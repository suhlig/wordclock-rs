//! Core, hardware-independent logic for the word clock.
//!
//! These modules turn a local time into a frame of logical pixels and encode
//! that frame as a WS2812 bitstream. They build with `no_std` and `alloc`, so
//! the same files run on the host and on the ESP32-C3 firmware.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod clock;
pub mod color;
pub mod frame;
pub mod logring;
pub mod strip;
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
