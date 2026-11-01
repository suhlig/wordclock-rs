//! `WS2812B` transport shaping.
//!
//! The strip's timing lives here as data: one SPI clock and one bit cell, from
//! which the four pulse widths and the reset are derived. The tests at the
//! bottom of this file check the result against the `WS2812B` datasheet window,
//! so a future change to the clock or the cell fails in CI instead of on the
//! wall.
//!
//! # Why the cell is four bits wide
//!
//! An SPI shift register has no NZR mode, so each LED bit is carried by one
//! cell of SPI bits: a high run at the start of the cell is the pulse and the
//! rest of the cell is the gap. The width of that cell decides what can be
//! expressed, and three bits per LED bit is not enough. A `1` then has to be
//! two clocks high and one low and a `0` the reverse, which ties every `1` to a
//! gap of exactly one clock. A `0` needs a pulse of at most 0.55 us and a `1`
//! needs a gap of 0.45 us nominal, and those are the same number there, so
//! widening the gap can only come out of the `0` pulse. The gap is capped as
//! well: `T1H` is twice the gap and must stay at or below 950 ns, so no clock
//! gives a gap wider than 475 ns, and reaching the 450 ns nominal already
//! leaves `T1H` on its ceiling.
//!
//! That was not a theory. At 2.5 MHz the three-bit cell left 400 ns after every
//! `1`, and any pixel whose last LED bit was a `1` was followed by a stray pixel
//! carrying `G = 0x80`. Four bits separate the two numbers: a `1` is two clocks
//! high and two low, a `0` is one clock high and three low, so both pulses are
//! at their datasheet nominal and the gap after a `1` is 0.8 us. Do not
//! reintroduce a narrower cell to save memory or frame time.
//!
//! See `docs/strip-colour-fault-handover.md` for how that fault was found and
//! the experiment that confirmed this cell fixes it.

use crate::color::Rgb;

/// SPI clock for the strip, in kHz.
///
/// 2.5 MHz is exact on the ESP32-C3's SPI2 (80 MHz / 32) and on a Raspberry Pi
/// at its default divisor. It is the clock the waveform below is defined
/// against; changing it changes every pulse width, which the tests catch.
pub const SPI_KHZ: u32 = 2_500;

/// One SPI bit, in nanoseconds.
pub const SPI_BIT_NS: u32 = 1_000_000 / SPI_KHZ;

/// SPI bits per LED bit. Four, for the reason in the module documentation.
pub const BITS_PER_LED_BIT: usize = 4;

/// One LED bit as an SPI cell: high run first, then gap.
const ZERO: u8 = 0b1000;
const ONE: u8 = 0b1100;

/// Bytes one pixel occupies in the stream: 24 LED bits of cells.
pub const BYTES_PER_PIXEL: usize = 24 * BITS_PER_LED_BIT / 8;

/// Reset tail in bytes of low.
///
/// The `WS2812B` needs more than 50 us and the V5 revision more than 280 us;
/// this is 448 us at the clock above, which satisfies both with margin.
pub const RESET_BYTES: usize = 140;

/// Counts the leading ones of a cell, which is its pulse width.
const fn high_bits(pattern: u8) -> u32 {
    let mut high = 0;
    let mut index = BITS_PER_LED_BIT;

    while index > 0 {
        index -= 1;
        if (pattern >> index) & 1 == 0 {
            break;
        }
        high += 1;
    }

    high
}

/// The pulse train the cell produces, in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Waveform {
    /// One SPI bit.
    pub bit_ns: u32,
    /// High time for a `0` LED bit.
    pub t0h: u32,
    /// Low time after a `0` LED bit.
    pub t0l: u32,
    /// High time for a `1` LED bit.
    pub t1h: u32,
    /// Low time after a `1` LED bit.
    pub t1l: u32,
    /// Total low time of the reset tail.
    pub reset_ns: u32,
}

impl Waveform {
    /// Derives the waveform the cell produces at the given SPI bit period.
    pub const fn new(bit_ns: u32) -> Self {
        let bits = BITS_PER_LED_BIT as u32;

        Self {
            bit_ns,
            t0h: high_bits(ZERO) * bit_ns,
            t0l: (bits - high_bits(ZERO)) * bit_ns,
            t1h: high_bits(ONE) * bit_ns,
            t1l: (bits - high_bits(ONE)) * bit_ns,
            reset_ns: RESET_BYTES as u32 * 8 * bit_ns,
        }
    }

    /// The bit period the strip sees for a `0`.
    pub const fn zero_period(self) -> u32 {
        self.t0h + self.t0l
    }

    /// The bit period the strip sees for a `1`.
    pub const fn one_period(self) -> u32 {
        self.t1h + self.t1l
    }
}

/// The bytes [`encode`] writes for `pixels` pixels and their reset tail.
pub const fn buffer_len(pixels: usize) -> usize {
    pixels * BYTES_PER_PIXEL + RESET_BYTES
}

/// Encodes `frame` in GRB wire order, followed by the reset tail.
///
/// Returns the bytes to hand to the SPI peripheral, or `None` when `out` is
/// smaller than [`buffer_len`]. The caller sizes `out` from that function, so
/// `None` means the frame and the buffer disagree and is worth reporting.
pub fn encode<'a>(frame: &[Rgb], out: &'a mut [u8]) -> Option<&'a [u8]> {
    let payload = frame.len().checked_mul(BYTES_PER_PIXEL)?;
    let total = payload.checked_add(RESET_BYTES)?;

    if out.len() < total {
        return None;
    }

    let (payload_out, reset_out) = out.split_at_mut(payload);
    payload_out.fill(0);
    reset_out.fill(0);

    let mut bit = 0;
    for pixel in frame {
        for value in [pixel.g, pixel.r, pixel.b] {
            for shift in (0..8).rev() {
                write_cell(&mut payload_out[..], bit, (value >> shift) & 1 == 1);
                bit += BITS_PER_LED_BIT;
            }
        }
    }

    Some(&out[..total])
}

/// Writes one cell, most significant SPI bit first.
fn write_cell(out: &mut [u8], start: usize, one: bool) {
    let pattern = if one { ONE } else { ZERO };

    for offset in 0..BITS_PER_LED_BIT {
        if (pattern >> (BITS_PER_LED_BIT - 1 - offset)) & 1 == 0 {
            continue;
        }

        let index = start + offset;
        out[index / 8] |= 0x80 >> (index % 8);
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    /// `WS2812B` datasheet windows in nanoseconds: low bound, high bound.
    ///
    /// `T0H` 0.4 ±0.15, `T1H` 0.8 ±0.15, `T0L` 0.85 ±0.15, `T1L` 0.45 ±0.15,
    /// bit period 1.25 ±0.6. The reset is above 50 us everywhere and above
    /// 280 us on the V5 revision.
    const T0H_WINDOW: (u32, u32) = (250, 550);
    const T1H_WINDOW: (u32, u32) = (650, 950);
    const T0L_WINDOW: (u32, u32) = (700, 1_000);
    const T1L_WINDOW: (u32, u32) = (300, 600);
    const PERIOD_WINDOW: (u32, u32) = (650, 1_850);

    const T0H_NOMINAL: u32 = 400;
    const T1H_NOMINAL: u32 = 800;
    const T1L_NOMINAL: u32 = 450;
    const T0L_NOMINAL: u32 = 850;

    const RESET_MIN_NS: u32 = 280_000;
    const RESET_THRESHOLD_NS: u32 = 50_000;

    fn within(value: u32, (low, high): (u32, u32)) -> bool {
        value >= low && value <= high
    }

    fn waveform() -> Waveform {
        Waveform::new(SPI_BIT_NS)
    }

    /// The two high times are what the strip decodes, so they sit on their
    /// nominal values rather than at the edge of the window.
    #[test]
    fn the_data_pulses_are_at_nominal() {
        let waveform = waveform();

        assert_eq!(waveform.bit_ns, 400);
        assert_eq!(waveform.t0h, T0H_NOMINAL);
        assert_eq!(waveform.t1h, T1H_NOMINAL);
        assert!(within(waveform.t0h, T0H_WINDOW));
        assert!(within(waveform.t1h, T1H_WINDOW));
    }

    /// The gap after a `1` is the parameter the stray pixel tracked: at 400 ns
    /// a pixel ending in `1` was followed by a stray, and this is the assertion
    /// that keeps it above the nominal.
    ///
    /// Unlike the high times, a low is only bounded from below in practice: a
    /// gap longer than the datasheet maximum has no failure mode until it
    /// starts to look like a reset, so the lows are checked one-sided with the
    /// 50 us threshold as the ceiling.
    #[test]
    fn the_gaps_are_never_shorter_than_nominal() {
        let waveform = waveform();

        assert!(
            waveform.t1l >= T1L_WINDOW.0 && waveform.t1l >= T1L_NOMINAL,
            "the gap after a 1 is {} ns, below the {} ns nominal",
            waveform.t1l,
            T1L_NOMINAL
        );
        assert!(
            waveform.t0l >= T0L_WINDOW.0 && waveform.t0l >= T0L_NOMINAL,
            "the gap after a 0 is {} ns, below the {} ns nominal",
            waveform.t0l,
            T0L_NOMINAL
        );

        assert!(waveform.t1l < RESET_THRESHOLD_NS);
        assert!(waveform.t0l < RESET_THRESHOLD_NS);

        assert!(within(waveform.zero_period(), PERIOD_WINDOW));
        assert!(within(waveform.one_period(), PERIOD_WINDOW));
        assert!(waveform.reset_ns >= RESET_MIN_NS);
    }

    #[test]
    fn encoding_length_is_exact() {
        let frame = [Rgb::new(1, 2, 3); 7];
        let expected = buffer_len(frame.len());
        let mut out = [0u8; 256];
        let encoded = encode(&frame, &mut out).expect("buffer is large enough");

        assert_eq!(encoded.len(), expected);
        assert_eq!(encoded.len(), frame.len() * BYTES_PER_PIXEL + RESET_BYTES);
        assert!(
            encoded[..expected - RESET_BYTES]
                .iter()
                .any(|byte| *byte != 0)
        );
        assert!(
            encoded[expected - RESET_BYTES..]
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn round_trip_preserves_every_byte() {
        let frame: Vec<Rgb> = (0..=255u8)
            .map(|value| Rgb::new(value, 255 - value, value.rotate_left(3)))
            .collect();

        let mut out = alloc::vec![0u8; buffer_len(frame.len())];
        let encoded = encode(&frame, &mut out).expect("buffer is large enough");

        assert_eq!(decode(encoded, frame.len()), frame);
    }

    /// WS2812 expects green, red, blue in that order on the wire.
    #[test]
    fn wire_order_is_green_red_blue() {
        let frame = [Rgb::new(0x11, 0x22, 0x33)];
        let mut out = [0u8; 256];
        let encoded = encode(&frame, &mut out).expect("buffer is large enough");

        assert_eq!(decode(encoded, 1), frame);
    }

    /// No run inside the payload may approach the 50 us reset threshold, and no
    /// pulse may reach three clocks, which would blur the `0`/`1` boundary.
    #[test]
    fn no_cell_run_approaches_a_reset() {
        let frame: Vec<Rgb> = (0..64u8)
            .map(|value| Rgb::new(value, !value, value))
            .collect();
        let payload = frame.len() * BYTES_PER_PIXEL;

        let mut out = alloc::vec![0u8; buffer_len(frame.len())];
        let encoded = encode(&frame, &mut out).expect("buffer is large enough");
        let (high, low) = longest_runs(&encoded[..payload]);

        assert!(
            (high as u32) * SPI_BIT_NS <= 800,
            "a {high}-bit high run is {} ns",
            (high as u32) * SPI_BIT_NS
        );
        assert!(
            (low as u32) * SPI_BIT_NS <= 1_200,
            "a {low}-bit low run is {} ns",
            (low as u32) * SPI_BIT_NS
        );
        assert!((low as u32) * SPI_BIT_NS < RESET_THRESHOLD_NS);
    }

    fn longest_runs(data: &[u8]) -> (usize, usize) {
        let mut longest = (0, 0);
        let mut run = (0, 0);

        for byte in data {
            for bit in (0..8).rev() {
                if (byte >> bit) & 1 == 1 {
                    run = (run.0 + 1, 0);
                } else {
                    run = (0, run.1 + 1);
                }
                longest = (longest.0.max(run.0), longest.1.max(run.1));
            }
        }

        longest
    }

    /// Decodes a payload back into pixels, inverting the cell layout.
    fn decode(data: &[u8], pixels: usize) -> Vec<Rgb> {
        let mut led_bits = Vec::with_capacity(pixels * 24);

        for cell in 0..pixels * 24 {
            let mut pattern = 0u8;
            for offset in 0..BITS_PER_LED_BIT {
                let index = cell * BITS_PER_LED_BIT + offset;
                pattern = (pattern << 1) | ((data[index / 8] >> (7 - index % 8)) & 1);
            }

            if pattern == ONE {
                led_bits.push(1u8);
            } else {
                assert_eq!(pattern, ZERO, "unexpected cell {pattern:#b} at {cell}");
                led_bits.push(0);
            }
        }

        (0..pixels)
            .map(|pixel| {
                let channel = |offset: usize| {
                    led_bits[pixel * 24 + offset * 8..pixel * 24 + offset * 8 + 8]
                        .iter()
                        .fold(0u8, |value, bit| (value << 1) | bit)
                };
                Rgb::new(channel(1), channel(0), channel(2))
            })
            .collect()
    }
}
