//! SPI2+DMA transport for the WS2812 strip on the ESP32-C3.
//!
//! This is the production shape of the transport proven in
//! `spike/esp32c3-ota`: the CPU hands DMA a pre-encoded bitstream and is not
//! part of the timing path. The waveform itself lives in
//! `wordclock-core/src/strip.rs`.

use esp_hal::Blocking;
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config, Spi, SpiDma};
use esp_hal::time::Rate;
use wordclock_core::color::Rgb;
use wordclock_core::strip;

/// Errors the strip transport can report.
#[derive(Debug)]
pub enum Error {
    /// The SPI/DMA peripheral could not be initialized.
    Init(esp_hal::spi::master::ConfigError),
    /// The encoded frame does not fit the caller's buffer.
    BufferTooSmall,
    /// The DMA transfer failed.
    Write(esp_hal::spi::Error),
}

/// A WS2812 strip driven over SPI2 with DMA.
///
/// `N` is `strip::buffer_len(pixels)` for the configured strip length. The
/// buffer is owned here so a frame can be encoded and handed to DMA without an
/// allocation.
pub struct StripDisplay<const N: usize> {
    spi: SpiDma<'static, Blocking>,
    buffer: [u8; N],
}

impl<const N: usize> StripDisplay<N> {
    /// Initializes SPI2 with DMA on `mosi` at the waveform clock.
    pub fn new(
        spi2: esp_hal::peripherals::SPI2<'static>,
        dma: esp_hal::peripherals::DMA_CH0<'static>,
        mosi: esp_hal::peripherals::GPIO3<'static>,
    ) -> Result<Self, Error> {
        let spi = Spi::new(
            spi2,
            Config::default()
                .with_frequency(Rate::from_khz(strip::SPI_KHZ))
                .with_mode(Mode::_0),
        )
        .map_err(Error::Init)?
        .with_mosi(mosi)
        .with_dma(dma);

        Ok(Self {
            spi,
            buffer: [0; N],
        })
    }

    /// Encodes `frame` and transmits it in one DMA transfer.
    pub fn write(&mut self, frame: &[Rgb]) -> Result<(), Error> {
        let data = strip::encode(frame, &mut self.buffer).ok_or(Error::BufferTooSmall)?;
        self.spi.write(data).map_err(Error::Write)
    }
}
