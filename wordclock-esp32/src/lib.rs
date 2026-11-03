//! ESP32-C3 firmware for the word clock.
//!
//! Task 4.9 creates this crate and shares `wordclock-core`. The SPI2+DMA
//! transport is in `display.rs`; the firmware loop is task 6C.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(target_arch = "riscv32")]
pub mod config;
#[cfg(target_arch = "riscv32")]
pub mod display;
#[cfg(target_arch = "riscv32")]
pub mod eventlog;
#[cfg(target_arch = "riscv32")]
pub mod provision;

pub use wordclock_core::*;
