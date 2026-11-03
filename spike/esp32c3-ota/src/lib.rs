//! Re-exports the shared core for the ESP32-C3 spike.
//!
//! The spike and the production firmware now use the same `wordclock-core`
//! crate, so the frame dump and the transport see identical types. The
//! `esp32c3_ota::clock`-style paths keep working through this re-export.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub use wordclock_core::*;
