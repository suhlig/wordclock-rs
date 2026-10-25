//! Core, hardware-independent logic for the word clock.
//!
//! The modules in this crate turn a local time into a frame of logical pixels.
//! The SPI output backend lives behind the `hardware` feature; everything else
//! builds and tests on any platform.

#![forbid(unsafe_code)]

// `core` modules use `alloc` types directly so the same files can be included
// by the `no_std` ESP32-C3 firmware without a `std` prelude.
extern crate alloc;

pub mod clock;
pub mod color;
pub mod config;
pub mod daemon;
pub mod display;
pub mod frame;
pub mod sim;
pub mod timezone;

/// Number of columns in the 18x16 word grid.
pub const COLS: usize = 18;

/// Number of rows in the 18x16 word grid.
pub const ROWS: usize = 16;

/// Number of logical word pixels driven by the clock.
pub const LOGICAL_PIXELS: usize = 288;

#[cfg(test)]
mod tests {
    use super::{COLS, LOGICAL_PIXELS, ROWS};

    #[test]
    fn grid_matches_logical_pixel_count() {
        assert_eq!(COLS * ROWS, LOGICAL_PIXELS);
    }
}
