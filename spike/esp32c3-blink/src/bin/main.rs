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

    // The ESP32-C3-DevKit-RUST-1 has its on-board WS2812 on GPIO2.
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
        .expect("Failed to initialize the WS2812 driver");

    let delay = Delay::new();
    let colors = [
        ("red", RGB8::new(255, 0, 0)),
        ("green", RGB8::new(0, 255, 0)),
        ("blue", RGB8::new(0, 0, 255)),
        ("white", RGB8::new(255, 255, 255)),
        ("off", RGB8::new(0, 0, 0)),
    ];

    loop {
        for (name, color) in colors {
            info!("on-board WS2812: {name}");
            led.write([color]).expect("RMT write failed");
            delay.delay_millis(700);
        }
    }
}
