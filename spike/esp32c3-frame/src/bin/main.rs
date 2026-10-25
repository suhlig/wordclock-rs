#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use alloc::format;
use alloc::string::String;

use chrono::NaiveDate;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::main;
use esp_println::println;

use esp32c3_frame::clock;
use esp32c3_frame::color::{ColorPicker, DEFAULT_BIRTHDAYS, GammaLut, Rgb};
use esp32c3_frame::frame::Frame;

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

const PHYSICAL_LEN: usize = esp32c3_frame::LOGICAL_PIXELS;
const WHITEPOINT: [f64; 3] = [1.0, 1.0, 1.0];

/// Renders one frame and prints the same dump as `examples/dump_frame.rs`.
#[allow(
    clippy::large_stack_frames,
    reason = "the gamma LUT and frame buffers are bounded and short-lived"
)]
fn dump(date: NaiveDate, hour: u32, minute: u32, color: Option<Rgb>, seed: Option<u64>) {
    let mut picker = ColorPicker::from_rng(
        fastrand::Rng::with_seed(seed.unwrap_or(0)),
        DEFAULT_BIRTHDAYS.to_vec(),
    );
    picker.begin_minute();

    let mut frame = Frame::blank(PHYSICAL_LEN);
    for index in clock::pixels(hour, minute) {
        let chosen = match color {
            Some(color) => color,
            None => picker.choose(date, hour, minute).rgb,
        };
        frame.set(index, chosen);
    }

    let corrected = frame.corrected_logical(&GammaLut::new(WHITEPOINT));

    let color_label = match color {
        Some(color) => format!("{:02x}{:02x}{:02x}", color.r, color.g, color.b),
        None => String::from("none"),
    };
    let seed_label = match seed {
        Some(seed) => format!("{seed}"),
        None => String::from("none"),
    };

    println!(
        "FRAME time={}T{:02}:{:02} color={} seed={}",
        date, hour, minute, color_label, seed_label
    );

    for (index, (&raw, &shown)) in frame.logical().iter().zip(corrected.iter()).enumerate() {
        println!(
            "{index} {:02x}{:02x}{:02x} {:02x}{:02x}{:02x}",
            raw.r, raw.g, raw.b, shown.r, shown.g, shown.b
        );
    }

    println!("END");
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let _peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let delay = Delay::new();

    // Give the USB Serial/JTAG monitor time to attach after the reset.
    delay.delay_millis(8_000);

    let regular = NaiveDate::from_ymd_opt(2026, 7, 15).expect("valid date");
    let carnival = NaiveDate::from_ymd_opt(2026, 2, 16).expect("valid date");

    loop {
        println!("wordclock-rs ESP32-C3 frame dump");

        // Fixed color: exercises the indices and the gamma vector.
        dump(regular, 6, 30, Some(Rgb::new(0xc0, 0x40, 0x20)), None);
        // Seeded uniform color on a regular day: exercises XKCD sampling.
        dump(regular, 6, 30, None, Some(42));
        // Rosenmontag: per-pixel sampling through the occasion arbiter.
        dump(carnival, 6, 30, None, Some(42));

        println!("END OF SET");
        delay.delay_millis(30_000);
    }
}
