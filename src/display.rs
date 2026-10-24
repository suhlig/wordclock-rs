//! Output backends.
//!
//! The terminal and null backends always build. The SPI backend for
//! `/dev/spidev0.0` is compiled only with the `hardware` feature, which is
//! also what pulls in `rppal` and `ws2812-spi`.

/// Whether SPI hardware support was compiled into this build.
pub const HARDWARE_ENABLED: bool = cfg!(feature = "hardware");
