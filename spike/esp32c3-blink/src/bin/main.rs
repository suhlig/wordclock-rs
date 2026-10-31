#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::main;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use log::info;
use smart_leds::{RGB8, SmartLedsWrite};

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    // GPIO3 is a free header pin on the ESP32-C3-DevKit-RUST-1: no strapping role,
    // no onboard peripheral, no JTAG and no internal-flash duty. GPIO4-7 carry JTAG
    // and the flash data lines, GPIO8/10 the I2C bus, GPIO9 is the BOOT button, and
    // GPIO2 is a strapping pin with the on-board WS2812 on it.
    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, frequency).expect("Failed to initialize RMT");

    const PIXELS: usize = 35; // offcut length after removing the dead pixels

    let mut led =
        RmtSmartLeds::<{ buffer_size::<RGB8>(PIXELS) }, _, RGB8, color_order::Grb>::new_with_memsize(
            esp_hal_smartled::WS2812_TIMING,
            rmt.channel0,
            peripherals.GPIO3,
            2,
            frequency,
        )
        .expect("Failed to initialize the WS2812 driver");

    let delay = Delay::new();
    let mut buffer = [RGB8::new(0, 0, 0); PIXELS];

    loop {
        // Primaries first: a swapped channel order shows up immediately.
        for (name, color) in [
            ("red", RGB8::new(255, 0, 0)),
            ("green", RGB8::new(0, 255, 0)),
            ("blue", RGB8::new(0, 0, 255)),
            ("white", RGB8::new(255, 255, 255)),
            ("off", RGB8::new(0, 0, 0)),
        ] {
            info!("all {PIXELS}: {name}");
            buffer.fill(color);
            led.write(buffer.iter().copied()).expect("RMT write failed");
            delay.delay_millis(700);
        }

        // Walking pixel: proves every index is addressed, in order, end to end.
        for index in 0..PIXELS {
            info!("walk {index}");
            buffer.fill(RGB8::new(0, 0, 0));
            buffer[index] = RGB8::new(255, 255, 255);
            led.write(buffer.iter().copied()).expect("RMT write failed");
            delay.delay_millis(120);
        }
    }
}
