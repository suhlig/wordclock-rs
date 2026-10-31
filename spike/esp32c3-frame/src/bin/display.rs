//! Shows the real color pipeline on a 35-pixel offcut.
//!
//! This binary includes the same host core modules as `esp32c3-frame` and renders
//! genuine frames, then writes the first 35 corrected pixels to the strip.
//!
//! Each test color is shown twice, raw then gamma-corrected, so the effect of the
//! 2.5 curve is visible side by side on the same LEDs. Two word slices follow, so
//! the real frame can be compared with the host simulator:
//!
//! `cargo run --example dump_frame -- "15.07.2026 06:30" --seed 42`

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use chrono::NaiveDate;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::main;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal_smartled::{RmtSmartLeds, buffer_size, color_order};
use esp_println::println;
use smart_leds::{RGB8, SmartLedsWrite};

use esp32c3_frame::clock;
use esp32c3_frame::color::{ColorPicker, DEFAULT_BIRTHDAYS, GammaLut, Rgb};
use esp32c3_frame::frame::Frame;

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
esp_bootloader_esp_idf::esp_app_desc!();

/// Offcut length.
const PIXELS: usize = 35;
const PHYSICAL_LEN: usize = esp32c3_frame::LOGICAL_PIXELS;
const WHITEPOINT: [f64; 3] = [1.0, 1.0, 1.0];
const HOLD_MS: u32 = 2_500;

/// Copies the first `PIXELS` corrected pixels into a fixed buffer.
fn take_first(corrected: &[Rgb]) -> [RGB8; PIXELS] {
    let mut out = [RGB8::new(0, 0, 0); PIXELS];
    for (slot, rgb) in out.iter_mut().zip(corrected.iter()) {
        *slot = RGB8::new(rgb.r, rgb.g, rgb.b);
    }
    out
}

/// Lights every offcut pixel with the uncorrected color.
fn raw(raw: Rgb) -> [RGB8; PIXELS] {
    [RGB8::new(raw.r, raw.g, raw.b); PIXELS]
}

/// Lights every logical pixel with one raw color, then corrects it, so the
/// offcut shows the corrected color itself rather than the scattered word pixels.
fn swatch(raw: Rgb) -> [RGB8; PIXELS] {
    let mut frame = Frame::blank(PHYSICAL_LEN);
    for index in 0..PHYSICAL_LEN {
        frame.set(index, raw);
    }
    take_first(&frame.corrected_logical(&GammaLut::new(WHITEPOINT)))
}

/// Renders the real word frame for the given time and seed.
fn words(date: NaiveDate, hour: u32, minute: u32, seed: u64) -> [RGB8; PIXELS] {
    let mut picker =
        ColorPicker::from_rng(fastrand::Rng::with_seed(seed), DEFAULT_BIRTHDAYS.to_vec());
    picker.begin_minute();

    let mut frame = Frame::blank(PHYSICAL_LEN);
    for index in clock::pixels(hour, minute) {
        frame.set(index, picker.choose(date, hour, minute).rgb);
    }
    take_first(&frame.corrected_logical(&GammaLut::new(WHITEPOINT)))
}

/// Returns the raw color the picker chooses for this minute.
fn seeded(date: NaiveDate, hour: u32, minute: u32, seed: u64) -> Rgb {
    let mut picker =
        ColorPicker::from_rng(fastrand::Rng::with_seed(seed), DEFAULT_BIRTHDAYS.to_vec());
    picker.begin_minute();
    picker.choose(date, hour, minute).rgb
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, frequency).expect("Failed to initialize RMT");
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

    // 2026-07-15 is a regular Wednesday; 2026-02-16 is Rosenmontag, which samples
    // a color per pixel.
    let regular = NaiveDate::from_ymd_opt(2026, 7, 15).expect("valid date");
    let carnival = NaiveDate::from_ymd_opt(2026, 2, 16).expect("valid date");

    let fixed = Rgb::new(0xc0, 0x40, 0x20);
    let seed_42 = seeded(regular, 6, 30, 42);
    let seed_7 = seeded(regular, 6, 30, 7);

    println!(
        "raw colors: fixed c04020, seed 42 {:02x}{:02x}{:02x}, seed 7 {:02x}{:02x}{:02x}",
        seed_42.r, seed_42.g, seed_42.b, seed_7.r, seed_7.g, seed_7.b
    );

    let steps: [(&str, [RGB8; PIXELS]); 8] = [
        ("raw c04020", raw(fixed)),
        ("gamma c04020", swatch(fixed)),
        ("raw seed 42", raw(seed_42)),
        ("gamma seed 42", swatch(seed_42)),
        ("raw seed 7", raw(seed_7)),
        ("gamma seed 7", swatch(seed_7)),
        ("word slice 06:30 seed 42", words(regular, 6, 30, 42)),
        ("word slice Rosenmontag seed 42", words(carnival, 6, 30, 42)),
    ];

    loop {
        for (label, pixels) in steps.iter() {
            println!("showing {label}");
            led.write(pixels.iter().copied()).expect("RMT write failed");
            delay.delay_millis(HOLD_MS);
        }

        println!("showing off");
        let blank = [RGB8::new(0, 0, 0); PIXELS];
        led.write(blank.iter().copied()).expect("RMT write failed");
        delay.delay_millis(1_000);
    }
}
