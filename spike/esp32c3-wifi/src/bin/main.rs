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

use alloc::vec;
use alloc::vec::Vec;

use chrono::{DateTime, Timelike};
use embassy_executor::Spawner;
use embassy_net::{
    Runner, Stack, StackResources,
    dns::DnsQueryType,
    udp::{PacketMetadata, UdpSocket},
};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    rmt::Rmt,
    rng::Rng,
    rtc_cntl::Rtc,
    time::{Instant, Rate},
    timer::timg::TimerGroup,
};
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use esp_println::println;
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config as WifiConfig, ControllerConfig, Interface, WifiController,
    sta::StationConfig,
};
use log::{error, info};
use smart_leds::{RGB8, SmartLedsWrite};
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};
use sntpc_net_embassy::UdpSocketWrapper;

use esp32c3_wifi::clock;
use esp32c3_wifi::color::{ColorPicker, DEFAULT_BIRTHDAYS, GammaLut};
use esp32c3_wifi::frame::Frame;
use esp32c3_wifi::timezone::berlin_local_from_utc;

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

const PIXELS: usize = esp32c3_wifi::LOGICAL_PIXELS;
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

/// The gate firmware's LED driver: 288 logical pixels, no padding.
type Leds<'a> =
    RmtSmartLeds<'a, { buffer_size::<RGB8>(PIXELS) }, esp_hal::Blocking, RGB8, color_order::Grb>;

fn now_us() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

/// Writes a full frame and returns how long the blocking call took.
fn write_frame(leds: &mut Leds<'_>, pixels: &[RGB8]) -> u64 {
    let start = now_us();
    leds.write(pixels.iter().copied())
        .expect("RMT write failed");
    now_us() - start
}

/// Renders a local time through the real core and returns the corrected pixels.
fn clock_pixels(local: chrono::NaiveDateTime, seed: u64) -> Vec<RGB8> {
    let mut picker =
        ColorPicker::from_rng(fastrand::Rng::with_seed(seed), DEFAULT_BIRTHDAYS.to_vec());
    picker.begin_minute();

    let mut frame = Frame::blank(PIXELS);
    for index in clock::pixels(local.hour(), local.minute()) {
        frame.set(
            index,
            picker
                .choose(local.date(), local.hour(), local.minute())
                .rgb,
        );
    }

    frame
        .corrected_physical(&GammaLut::new([1.0, 1.0, 1.0]))
        .into_iter()
        .map(|color| RGB8::new(color.r, color.g, color.b))
        .collect()
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

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let boot_us = now_us();
    println!("GATE boot_us={boot_us}");

    let rtc = Rtc::new(peripherals.RTC_TIMER);

    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, frequency).expect("Failed to initialize RMT");
    let mut leds = RmtSmartLeds::<{ buffer_size::<RGB8>(PIXELS) }, _, RGB8, color_order::Grb>::new_with_memsize(
        esp_hal_smartled::WS2812_TIMING,
        rmt.channel0,
        peripherals.GPIO2,
        2,
        frequency,
    )
    .expect("Failed to initialize the WS2812 driver");

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);
    esp_alloc::heap_allocator!(size: 36 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    // First frame: neutral, before any time source is trusted.
    let neutral = vec![RGB8::new(0, 0, 0); PIXELS];
    let neutral_write_us = write_frame(&mut leds, &neutral);
    let first_frame_us = now_us();
    println!("GATE first_frame_us={first_frame_us} neutral_write_us={neutral_write_us}");

    // All-white is the slowest possible frame: every bit is a long pulse. The
    // host derives the aggregate RMT refill overhead from this and the neutral
    // frame against the documented 1.05/1.40 us bit periods.
    let white = vec![RGB8::new(255, 255, 255); PIXELS];
    let white_write_us = write_frame(&mut leds, &white);
    println!("GATE white_write_us={white_write_us}");

    let stats = esp_alloc::HEAP.stats();
    let boot_heap_size = stats.size;
    let boot_heap_used = stats.current_usage;
    let boot_heap_max = stats.max_usage;
    println!(
        "GATE heap size={} used={} max={} rmt_buffer_bytes={}",
        boot_heap_size,
        boot_heap_used,
        boot_heap_max,
        buffer_size::<RGB8>(PIXELS) * core::mem::size_of::<u32>()
    );

    if SSID.is_empty() || PASSWORD.is_empty() {
        panic!(
            "set WORDCLOCK_WIFI_SSID and WORDCLOCK_WIFI_PASSWORD in spike/esp32c3-wifi/wifi.env"
        );
    }

    let station_config = WifiConfig::Station(
        StationConfig::default()
            .with_ssid(SSID.try_into().expect("SSID is too long"))
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(
                PASSWORD.try_into().expect("password is too long"),
            )),
    );

    let wifi_interface = Interface::station();
    let controller = WifiController::new(
        peripherals.WIFI,
        ControllerConfig::default().with_initial_config(station_config),
    )
    .expect("Failed to initialize the Wi-Fi controller");

    let net_config = embassy_net::Config::dhcpv4(Default::default());
    let rng = Rng::new();
    let net_seed = (rng.random() as u64) << 32 | rng.random() as u64;
    let color_seed = ((rng.random() as u64) << 32 | rng.random() as u64) ^ net_seed;

    let (stack, runner) = embassy_net::new(
        wifi_interface,
        net_config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        net_seed,
    );

    spawner.spawn(heartbeat(stack).expect("failed to spawn the heartbeat"));
    spawner.spawn(connection(controller).expect("failed to spawn the Wi-Fi task"));
    spawner.spawn(net_task(runner).expect("failed to spawn the network task"));

    stack.wait_config_up().await;
    let wifi_up_us = now_us();
    println!("GATE wifi_up_us={wifi_up_us}");

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

    /// The first-sync measurements, replayed on every resync because the USB
    /// monitor usually attaches after the one-shot boot lines are gone.
    #[derive(Clone, Copy)]
    struct Report {
        boot_us: u64,
        first_frame_us: u64,
        neutral_write_us: u64,
        wifi_up_us: u64,
        sntp_us: u64,
        clock_frame_us: u64,
        frame_min_us: u64,
        frame_avg_us: u64,
        white_write_us: u64,
        boot_heap_size: usize,
        boot_heap_used: usize,
        boot_heap_max: usize,
        heap_used: usize,
        heap_max: usize,
    }

    impl Report {
        fn print(&self) {
            println!(
                "GATE summary boot_us={} first_frame_us={} neutral_write_us={} wifi_up_us={} sntp_us={} clock_frame_us={} frame_min_us={} frame_avg_us={}",
                self.boot_us,
                self.first_frame_us,
                self.neutral_write_us,
                self.wifi_up_us,
                self.sntp_us,
                self.clock_frame_us,
                self.frame_min_us,
                self.frame_avg_us
            );
            println!(
                "GATE timing white_write_us={} zero_write_us={} frame_write_us={}",
                self.white_write_us, self.neutral_write_us, self.frame_avg_us
            );
            println!(
                "GATE heap size={} boot_used={} boot_max={} wifi_used={} wifi_max={} rmt_buffer_bytes={}",
                self.boot_heap_size,
                self.boot_heap_used,
                self.boot_heap_max,
                self.heap_used,
                self.heap_max,
                buffer_size::<RGB8>(PIXELS) * core::mem::size_of::<u32>()
            );
        }
    }

    let mut report: Option<Report> = None;

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
                SYNCED.store(true, Ordering::Relaxed);
                let sntp_us = now_us();

                let utc = DateTime::from_timestamp(time.sec() as i64, 0)
                    .expect("valid SNTP time")
                    .naive_utc();
                let local = berlin_local_from_utc(utc);
                let pixels = clock_pixels(local, color_seed);

                if report.is_none() {
                    println!("GATE first_sntp_us={sntp_us} utc={utc} local={local}");

                    let clock_write_us = write_frame(&mut leds, &pixels);
                    let clock_frame_us = now_us();
                    println!(
                        "GATE first_clock_frame_us={clock_frame_us} write_us={clock_write_us}"
                    );

                    let mut min = u64::MAX;
                    let mut sum = 0u64;
                    for _ in 0..5 {
                        let elapsed = write_frame(&mut leds, &pixels);
                        min = min.min(elapsed);
                        sum += elapsed;
                    }
                    println!("GATE frame_us min={min} avg={}", sum / 5);

                    let stats = esp_alloc::HEAP.stats();
                    println!(
                        "GATE heap_after_wifi size={} used={} max={}",
                        stats.size, stats.current_usage, stats.max_usage
                    );

                    let stored = Report {
                        boot_us,
                        first_frame_us,
                        neutral_write_us,
                        wifi_up_us,
                        sntp_us,
                        clock_frame_us,
                        frame_min_us: min,
                        frame_avg_us: sum / 5,
                        white_write_us,
                        boot_heap_size,
                        boot_heap_used,
                        boot_heap_max,
                        heap_used: stats.current_usage,
                        heap_max: stats.max_usage,
                    };
                    stored.print();
                    report = Some(stored);
                    info!("gate measurements complete");
                } else {
                    info!("SNTP resynchronized: unix={} local={local}", time.sec());
                    let elapsed = write_frame(&mut leds, &pixels);
                    info!("frame written in {elapsed} us");
                    if let Some(report) = &report {
                        report.print();
                    }
                }
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
                println!("GATE connected_us={}", now_us());
                info!("Wi-Fi connected: {info:?}");

                #[cfg(feature = "reconnect-test")]
                {
                    Timer::after(Duration::from_secs(15)).await;
                    println!("GATE disconnect_requested_us={}", now_us());
                    let _ = controller.disconnect_async().await;
                    println!("GATE disconnected_us={}", now_us());
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
