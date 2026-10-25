//! Terminal simulator and demo loop.
//!
//! This module is host-only. The clock, color, and frame logic it drives stay
//! independent of the Pi and the ESP32-C3.

use std::time::Duration;

use anyhow::{Result, bail};
use chrono::{Local, NaiveDate, NaiveDateTime, Timelike};

use crate::clock;
use crate::color::{ColorPicker, DEFAULT_BIRTHDAYS, GammaLut, Rgb};
use crate::display::{Display, TerminalDisplay};
use crate::frame::Frame;

/// Options shared by `sim` and `demo`.
#[derive(Clone, Debug)]
pub struct RenderOptions {
    /// Fixed color override; bypasses random sampling.
    pub color: Option<Rgb>,
    /// Seed for deterministic random sampling.
    pub seed: Option<u64>,
    /// Birthday dates as `(month, day)`.
    pub birthdays: Vec<(u32, u32)>,
    /// Per-channel whitepoint.
    pub whitepoint: [f64; 3],
    /// Physical strip length, including padding.
    pub physical_len: usize,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            color: None,
            seed: None,
            birthdays: DEFAULT_BIRTHDAYS.to_vec(),
            whitepoint: [1.0, 1.0, 1.0],
            physical_len: crate::LOGICAL_PIXELS,
        }
    }
}

/// Parses the legacy `DD.MM.YYYY HH:MM` format and common ISO-like forms.
pub fn parse_time(input: &str) -> Result<NaiveDateTime> {
    const FORMATS: [&str; 6] = [
        "%d.%m.%Y %H:%M",
        "%d.%m.%Y %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ];

    for format in FORMATS {
        if let Ok(time) = NaiveDateTime::parse_from_str(input, format) {
            return Ok(time);
        }
    }

    bail!("invalid time {input:?}; use `DD.MM.YYYY HH:MM` or an ISO-like form")
}

/// The current local wall-clock time.
pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

/// Renders one frame at the given logical time.
pub fn render(datetime: NaiveDateTime, picker: &mut ColorPicker, options: &RenderOptions) -> Frame {
    let date = datetime.date();
    let hour = datetime.hour();
    let minute = datetime.minute();
    let mut frame = Frame::blank(options.physical_len);

    for index in clock::pixels(hour, minute) {
        let color = match options.color {
            Some(color) => color,
            None => picker.choose(date, hour, minute).rgb,
        };
        frame.set(index, color);
    }

    frame
}

/// Renders a single frame to the terminal.
pub fn run(datetime: NaiveDateTime, options: RenderOptions) -> Result<()> {
    let mut picker = ColorPicker::new(options.seed, options.birthdays.clone());
    picker.begin_minute();
    let frame = render(datetime, &mut picker, &options);
    let lut = GammaLut::new(options.whitepoint);

    let mut display = TerminalDisplay::new(std::io::stdout());
    display.write(&frame.corrected_physical(&lut))?;
    display.flush()?;

    Ok(())
}

/// Calls `f` for every minute of the day, in order.
///
/// The picker is shared across the whole run, so the random sequence advances
/// while the per-minute uniform color cache is reset for every frame.
pub fn for_each_demo_frame<F>(date: NaiveDate, options: &RenderOptions, mut f: F) -> Result<()>
where
    F: FnMut(u32, u32, &Frame) -> Result<()>,
{
    let mut picker = ColorPicker::new(options.seed, options.birthdays.clone());

    for hour in 0..24 {
        for minute in 0..60 {
            let datetime = date
                .and_hms_opt(hour, minute, 0)
                .expect("hour and minute are in range");
            picker.begin_minute();
            let frame = render(datetime, &mut picker, options);
            f(hour, minute, &frame)?;
        }
    }

    Ok(())
}

/// Runs the full-day terminal demo.
pub fn demo(options: RenderOptions, interval: f64) -> Result<()> {
    if !interval.is_finite() || interval < 0.0 {
        bail!("interval must be a finite, non-negative number of seconds");
    }

    let date = Local::now().date_naive();
    let lut = GammaLut::new(options.whitepoint);
    let mut display = TerminalDisplay::with_clear(std::io::stdout(), true);

    for_each_demo_frame(date, &options, |hour, minute, frame| {
        eprintln!("{hour:02}:{minute:02}");
        display.write(&frame.corrected_physical(&lut))?;
        display.flush()?;

        if interval > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(interval));
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::color::ColorPicker;

    #[test]
    fn parses_legacy_and_iso_times() {
        assert_eq!(
            parse_time("12.11.2017 17:23").expect("legacy format"),
            NaiveDate::from_ymd_opt(2017, 11, 12)
                .unwrap()
                .and_hms_opt(17, 23, 0)
                .unwrap()
        );
        assert_eq!(
            parse_time("2017-11-12T17:23:00").expect("ISO format"),
            NaiveDate::from_ymd_opt(2017, 11, 12)
                .unwrap()
                .and_hms_opt(17, 23, 0)
                .unwrap()
        );
    }

    #[test]
    fn demo_visits_every_minute_once() {
        let mut seen = Vec::new();
        for_each_demo_frame(
            NaiveDate::from_ymd_opt(2026, 5, 5).unwrap(),
            &RenderOptions::default(),
            |hour, minute, _frame| {
                seen.push((hour, minute));
                Ok(())
            },
        )
        .expect("demo walk");

        assert_eq!(seen.len(), 1_440);
        assert_eq!(seen.first(), Some(&(0, 0)));
        assert_eq!(seen.last(), Some(&(23, 59)));
    }

    #[test]
    fn fixed_color_bypasses_sampling() {
        let options = RenderOptions {
            color: Some(Rgb::new(255, 0, 0)),
            seed: Some(1),
            ..RenderOptions::default()
        };
        let mut picker = ColorPicker::new(options.seed, options.birthdays.clone());
        let frame = render(
            NaiveDate::from_ymd_opt(2026, 5, 5)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
            &mut picker,
            &options,
        );

        for &color in frame.logical() {
            assert!(color == Rgb::BLACK || color == Rgb::new(255, 0, 0));
        }
        assert_eq!(
            frame
                .logical()
                .iter()
                .filter(|&&c| c == Rgb::new(255, 0, 0))
                .count(),
            clock::pixels(12, 0).len()
        );
    }

    #[test]
    fn same_minute_is_stable_across_refreshes() {
        let options = RenderOptions::default();
        let mut picker = ColorPicker::new(options.seed, options.birthdays.clone());
        picker.begin_minute();
        let datetime = NaiveDate::from_ymd_opt(2026, 5, 5)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();

        let first = render(datetime, &mut picker, &options);
        let second = render(datetime, &mut picker, &options);

        assert_eq!(
            first, second,
            "a refresh within the same minute must not resample the regular color"
        );
    }
}
