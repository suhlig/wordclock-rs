//! ESP32-C3 firmware for the word clock.
//!
//! Task 4.9 creates this crate and shares `wordclock-core`. Task 4.10 adds the
//! SPI2+DMA strip transport and the firmware loop from `spike/esp32c3-ota`.

#![no_std]
#![forbid(unsafe_code)]

pub use wordclock_core::*;
