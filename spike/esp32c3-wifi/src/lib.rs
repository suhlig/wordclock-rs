//! Compiles the real host core modules for the ESP32-C3 gate firmware.
//!
//! The files are included from `src/` rather than copied, so this firmware
//! tests exactly the code the host runs. The crate-root constants below are
//! what the modules expect from `crate::` in the host crate. Task 4.9 replaces
//! this with a real `wordclock-core` crate.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

/// Number of columns in the 18x16 word grid.
pub const COLS: usize = 18;

/// Number of rows in the 18x16 word grid.
pub const ROWS: usize = 16;

/// Number of logical word pixels driven by the clock.
pub const LOGICAL_PIXELS: usize = 288;

#[path = "../../../src/clock.rs"]
pub mod clock;

#[path = "../../../src/color.rs"]
pub mod color;

#[path = "../../../src/frame.rs"]
pub mod frame;

#[path = "../../../src/timezone.rs"]
pub mod timezone;
