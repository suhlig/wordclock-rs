//! Host-side backends for the word clock.
//!
//! The terminal simulator and the CLI live here, as does the Pi fallback
//! display backend. The selected ESP32-C3 firmware uses `wordclock-core`
//! directly.

#![forbid(unsafe_code)]

pub mod config;
pub mod daemon;
pub mod display;
pub mod sim;
