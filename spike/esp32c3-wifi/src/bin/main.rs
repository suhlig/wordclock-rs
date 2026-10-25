#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use core::net::{IpAddr, SocketAddr};
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_net::{
    Runner, Stack, StackResources,
    dns::DnsQueryType,
    udp::{PacketMetadata, UdpSocket},
};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock, rmt::Rmt, rng::Rng, rtc_cntl::Rtc, time::Rate, timer::timg::TimerGroup,
};
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config as WifiConfig, ControllerConfig, Interface, WifiController,
    sta::StationConfig,
};
use log::{error, info};
use smart_leds::{RGB8, SmartLedsWrite};
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};
use sntpc_net_embassy::UdpSocketWrapper;

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

const SSID: &str = env!("WORDCLOCK_WIFI_SSID");
const PASSWORD: &str = env!("WORDCLOCK_WIFI_PASSWORD");
const NTP_SERVER: &str = "pool.ntp.org";
static SYNCED: AtomicBool = AtomicBool::new(false);

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[derive(Clone, Copy)]
struct Timestamp<'a> {
    rtc: &'a Rtc<'a>,
    current_time_us: u64,
}

impl NtpTimestampGenerator for Timestamp<'_> {
    fn init(&mut self) {
        self.current_time_us = self.rtc.current_time_us();
    }

    fn timestamp_sec(&self) -> u64 {
        self.current_time_us / 1_000_000
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.current_time_us % 1_000_000) as u32
    }
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    info!("Booting ESP32-C3 Wi-Fi spike");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    info!("HAL initialized");
    let rtc = Rtc::new(peripherals.RTC_TIMER);

    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, frequency).expect("Failed to initialize RMT");
    let mut led =
        RmtSmartLeds::<{ buffer_size::<RGB8>(1) }, _, RGB8, color_order::Grb>::new_with_memsize(
            esp_hal_smartled::WS2812_TIMING,
            rmt.channel0,
            peripherals.GPIO2,
            2,
            frequency,
        )
        .expect("Failed to initialize the on-board WS2812");
    led.write([RGB8::new(0, 0, 0)])
        .expect("Failed to clear the on-board WS2812");
    info!("On-board WS2812 initialized");

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    esp_alloc::heap_allocator!(size: 36 * 1024);
    info!("Heap initialized");

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);
    info!("esp-rtos started");

    if SSID.is_empty() || PASSWORD.is_empty() {
        panic!(
            "set WORDCLOCK_WIFI_SSID and WORDCLOCK_WIFI_PASSWORD in spike/esp32c3-wifi/wifi.env"
        );
    }
    info!("Wi-Fi credentials present");

    let station_config = WifiConfig::Station(
        StationConfig::default()
            .with_ssid(SSID.try_into().expect("SSID is too long"))
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(
                PASSWORD.try_into().expect("password is too long"),
            )),
    );

    info!("Starting Wi-Fi");
    let wifi_interface = Interface::station();
    let controller = WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config(station_config),
    )
    .expect("Failed to initialize the Wi-Fi controller");
    info!("Wi-Fi controller initialized");

    let net_config = embassy_net::Config::dhcpv4(Default::default());
    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        net_config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );

    spawner.spawn(heartbeat(stack).expect("failed to spawn the heartbeat"));
    spawner.spawn(connection(controller).expect("failed to spawn the Wi-Fi task"));
    spawner.spawn(net_task(runner).expect("failed to spawn the network task"));

    info!("Waiting for DHCP");
    stack.wait_config_up().await;
    if let Some(config) = stack.config_v4() {
        info!("Got IP: {}", config.address);
    }

    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 4096];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 4096];
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(123).expect("failed to bind the SNTP socket");
    let socket = UdpSocketWrapper::new(socket);

    loop {
        let ntp_addrs = match stack.dns_query(NTP_SERVER, DnsQueryType::A).await {
            Ok(addrs) if !addrs.is_empty() => addrs,
            Ok(_) => {
                error!("DNS returned no addresses for {NTP_SERVER}");
                Timer::after(Duration::from_secs(10)).await;
                continue;
            }
            Err(error) => {
                error!("DNS error: {error:?}");
                Timer::after(Duration::from_secs(10)).await;
                continue;
            }
        };

        let addr: IpAddr = ntp_addrs[0].into();
        match get_time(
            SocketAddr::from((addr, 123)),
            &socket,
            NtpContext::new(Timestamp {
                rtc: &rtc,
                current_time_us: 0,
            }),
        )
        .await
        {
            Ok(time) => {
                info!("SNTP synchronized: unix={}", time.sec());
                SYNCED.store(true, Ordering::Relaxed);
                led.write([RGB8::new(0, 32, 0)])
                    .expect("Failed to set the sync indicator");
            }
            Err(error) => error!("SNTP error: {error:?}"),
        }

        Timer::after(Duration::from_secs(60)).await;
    }
}

#[embassy_executor::task]
async fn heartbeat(stack: Stack<'static>) {
    loop {
        match stack.config_v4() {
            Some(config) => info!(
                "heartbeat: ip={} synced={}",
                config.address,
                SYNCED.load(Ordering::Relaxed)
            ),
            None => info!(
                "heartbeat: waiting for DHCP synced={}",
                SYNCED.load(Ordering::Relaxed)
            ),
        }
        Timer::after(Duration::from_secs(5)).await;
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    loop {
        info!("Connecting to Wi-Fi");
        match controller.connect_async().await {
            Ok(info) => {
                info!("Wi-Fi connected: {info:?}");

                #[cfg(feature = "reconnect-test")]
                {
                    Timer::after(Duration::from_secs(15)).await;
                    info!("reconnect test: forcing a disconnect");
                    let _ = controller.disconnect_async().await;
                }

                #[cfg(not(feature = "reconnect-test"))]
                {
                    let info = controller.wait_for_disconnect_async().await.ok();
                    info!("Wi-Fi disconnected: {info:?}");
                }
            }
            Err(error) => error!("Wi-Fi connection failed: {error:?}"),
        }

        Timer::after(Duration::from_secs(5)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await
}
