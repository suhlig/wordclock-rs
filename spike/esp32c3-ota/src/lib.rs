//! Compiles the real host core modules for the frame check.
//!
//! The files are included from `src/` rather than copied, so this firmware
//! renders with exactly the code the host runs, which is what makes the
//! `GET /frame` dump comparable with `examples/dump_frame.rs`. Task 4.9
//! replaces this with a real `wordclock-core` crate.

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

#[path = "../../../src/strip.rs"]
pub mod strip;
