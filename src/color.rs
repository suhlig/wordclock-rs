//! Color selection, occasion rules, and gamma/whitepoint correction.
//!
//! The legacy `ColorPicker` samples once per minute on regular days and once
//! per lit pixel on carnival, birthdays, and the New Year party. The gamma
//! curve is Fadecandy's: whitepoint scaling, then pow(2.5), then rounding to
//! 8 bits.

mod xkcd;

use chrono::{Datelike, NaiveDate};
use fastrand::Rng;

/// An 8-bit RGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Black, used for unlit pixels and padding.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    /// Creates a color from its channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses an `RRGGBB` hex string, case-insensitively.
    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }

        let channel = |start: usize| u8::from_str_radix(&hex[start..start + 2], 16).ok();
        Some(Self::new(channel(0)?, channel(2)?, channel(4)?))
    }
}

/// A color with its legacy XKCD name, retained for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NamedColor {
    pub rgb: Rgb,
    pub name: &'static str,
}

/// The legacy default birthday list.
pub const DEFAULT_BIRTHDAYS: [(u32, u32); 5] = [(1, 13), (3, 10), (4, 8), (7, 18), (12, 31)];

/// The fixed grey used on Ash Wednesday and the New Year hangover.
pub const GREY: NamedColor = NamedColor {
    rgb: Rgb::new(64, 64, 64),
    name: "grey",
};

/// Iterates over the 922 legacy XKCD colors in insertion order.
pub fn all_xkcd() -> impl Iterator<Item = NamedColor> {
    xkcd::COLORS.iter().map(|&(r, g, b, name)| NamedColor {
        rgb: Rgb::new(r, g, b),
        name,
    })
}

/// Samples one legacy XKCD color with replacement.
pub fn sample_xkcd(rng: &mut Rng) -> NamedColor {
    let (r, g, b, name) = xkcd::COLORS[rng.usize(..xkcd::COLORS.len())];
    NamedColor {
        rgb: Rgb::new(r, g, b),
        name,
    }
}

/// The 256-entry per-channel gamma and whitepoint lookup table.
#[derive(Clone, Debug)]
pub struct GammaLut {
    channels: [[u8; 256]; 3],
}

impl GammaLut {
    /// Builds the LUT for the configured whitepoint.
    pub fn new(whitepoint: [f64; 3]) -> Self {
        let mut channels = [[0u8; 256]; 3];

        for channel in 0..3 {
            for value in 0..=255u8 {
                let input = f64::from(value) / 255.0 * whitepoint[channel];
                let corrected = libm::round(libm::pow(input, 2.5) * 255.0);
                channels[channel][value as usize] = corrected.clamp(0.0, 255.0) as u8;
            }
        }

        Self { channels }
    }

    /// Applies whitepoint and gamma correction to a color.
    pub fn correct(&self, color: Rgb) -> Rgb {
        Rgb::new(
            self.channels[0][color.r as usize],
            self.channels[1][color.g as usize],
            self.channels[2][color.b as usize],
        )
    }

    /// Applies correction to one channel, for tests and diagnostics.
    pub fn correct_channel(&self, channel: usize, value: u8) -> u8 {
        self.channels[channel][value as usize]
    }
}

/// Chooses the color for the current minute.
///
/// A fresh picker is used per minute, matching the legacy process lifetime.
/// Call [`ColorPicker::begin_minute`] before rendering a new minute; the RNG
/// advances across minutes, the cached uniform color does not.
pub struct ColorPicker {
    rng: Rng,
    birthdays: Vec<(u32, u32)>,
    cached: Option<NamedColor>,
}

impl ColorPicker {
    /// Creates a picker from an explicit RNG and birthday list.
    ///
    /// This is the no-std-friendly constructor: the firmware seeds its own
    /// RNG, while the host can use [`ColorPicker::new`] with `None` for
    /// entropy from the operating system.
    pub fn from_rng(rng: Rng, birthdays: Vec<(u32, u32)>) -> Self {
        Self {
            rng,
            birthdays,
            cached: None,
        }
    }

    /// Creates a picker with an explicit birthday list.
    pub fn new(seed: Option<u64>, birthdays: Vec<(u32, u32)>) -> Self {
        let rng = match seed {
            Some(seed) => Rng::with_seed(seed),
            None => Rng::new(),
        };

        Self::from_rng(rng, birthdays)
    }

    /// Creates a picker with the legacy default birthday list.
    pub fn with_default_birthdays(seed: Option<u64>) -> Self {
        Self::new(seed, DEFAULT_BIRTHDAYS.to_vec())
    }

    /// Clears the per-minute uniform color cache.
    pub fn begin_minute(&mut self) {
        self.cached = None;
    }

    /// Chooses a color for one pixel at the given local date and minute.
    ///
    /// The occasion order matches the legacy `ColorPicker`: carnival, Ash
    /// Wednesday, birthdays, New Year party, New Year hangover, regular day.
    pub fn choose(&mut self, date: NaiveDate, hour: u32, minute: u32) -> NamedColor {
        if is_carnival(date, hour, minute) {
            return sample_xkcd(&mut self.rng);
        }
        if is_ash_wednesday(date) {
            return GREY;
        }
        if self.is_birthday(date) {
            return sample_xkcd(&mut self.rng);
        }
        if is_new_year_party(date, hour) {
            return sample_xkcd(&mut self.rng);
        }
        if is_new_year_hangover(date, hour) {
            return GREY;
        }

        if let Some(cached) = self.cached {
            return cached;
        }

        let color = sample_xkcd(&mut self.rng);
        self.cached = Some(color);
        color
    }

    fn is_birthday(&self, date: NaiveDate) -> bool {
        let month = date.month();
        let day = date.day();
        self.birthdays
            .iter()
            .any(|&(birthday_month, birthday_day)| birthday_month == month && birthday_day == day)
    }
}

/// Carnival: 11 November from 11:11, Rosenmontag, and Faschingsdienstag.
pub fn is_carnival(date: NaiveDate, hour: u32, minute: u32) -> bool {
    let eleven_eleven =
        date.month() == 11 && date.day() == 11 && (hour > 11 || (hour == 11 && minute >= 11));
    if eleven_eleven {
        return true;
    }

    let easter = easter_sunday(date.year());
    date == days_before(easter, 48) || date == days_before(easter, 47)
}

/// Ash Wednesday: Easter Sunday minus 46 days.
pub fn is_ash_wednesday(date: NaiveDate) -> bool {
    let easter = easter_sunday(date.year());
    date == days_before(easter, 46)
}

/// New Year party: 1 January from 00:00 through 03:59.
pub fn is_new_year_party(date: NaiveDate, hour: u32) -> bool {
    date.month() == 1 && date.day() == 1 && hour < 4
}

/// New Year hangover: 1 January from 04:00 through 23:59.
pub fn is_new_year_hangover(date: NaiveDate, hour: u32) -> bool {
    date.month() == 1 && date.day() == 1 && hour >= 4
}

fn easter_sunday(year: i32) -> NaiveDate {
    let date = computus::gregorian(year).expect("computus supports years 1583 through 9999");
    NaiveDate::from_ymd_opt(date.year, date.month, date.day).expect("valid computus date")
}

fn days_before(date: NaiveDate, days: u64) -> NaiveDate {
    date.checked_sub_days(chrono::Days::new(days))
        .expect("occasion date is in range")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::NaiveDate;
    use sha2::{Digest, Sha256};

    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid test date")
    }

    #[test]
    fn gamma_vectors_match_fadecandy() {
        let lut = GammaLut::new([1.0, 1.0, 1.0]);

        for (input, expected) in [
            (0, 0),
            (32, 1),
            (64, 8),
            (96, 22),
            (128, 46),
            (160, 80),
            (192, 125),
            (224, 184),
            (255, 255),
        ] {
            assert_eq!(
                lut.correct(Rgb::new(input, input, input)),
                Rgb::new(expected, expected, expected),
                "gamma mismatch for {input}"
            );
        }
    }

    #[test]
    fn xkcd_table_has_922_unique_hexes_and_names() {
        let colors: Vec<NamedColor> = all_xkcd().collect();
        assert_eq!(colors.len(), 922);

        let hexes: HashSet<(u8, u8, u8)> = colors
            .iter()
            .map(|color| (color.rgb.r, color.rgb.g, color.rgb.b))
            .collect();
        let names: HashSet<&str> = colors.iter().map(|color| color.name).collect();

        assert_eq!(hexes.len(), 922);
        assert_eq!(names.len(), 922);
    }

    #[test]
    fn xkcd_checksum_matches_legacy_normalization() {
        let mut normalized = String::new();
        for color in all_xkcd() {
            normalized.push_str(&format!(
                "{:02X}{:02X}{:02X} {}\n",
                color.rgb.r, color.rgb.g, color.rgb.b, color.name
            ));
        }

        let digest = Sha256::digest(normalized.as_bytes());
        assert_eq!(
            format!("{digest:x}"),
            "9ac5913dccc2200cd46e2b5c4f61d05a5779d8f934b99788c652b34b3ee75d0e"
        );
    }

    #[test]
    fn carnival_and_ash_wednesday_match_2026() {
        // Easter 2026 is 5 April: Rosenmontag 16 February,
        // Faschingsdienstag 17 February, Ash Wednesday 18 February.
        assert!(is_carnival(date(2026, 2, 16), 12, 0));
        assert!(is_carnival(date(2026, 2, 17), 12, 0));
        assert!(!is_carnival(date(2026, 2, 18), 12, 0));
        assert!(is_ash_wednesday(date(2026, 2, 18)));
        assert!(!is_ash_wednesday(date(2026, 2, 17)));

        assert!(is_carnival(date(2026, 11, 11), 11, 11));
        assert!(!is_carnival(date(2026, 11, 11), 11, 10));
    }

    #[test]
    fn new_year_windows_split_at_four() {
        let new_year = date(2026, 1, 1);
        assert!(is_new_year_party(new_year, 3));
        assert!(!is_new_year_party(new_year, 4));
        assert!(is_new_year_hangover(new_year, 4));
        assert!(!is_new_year_hangover(new_year, 3));
    }

    #[test]
    fn regular_day_caches_one_color_per_minute() {
        let mut picker = ColorPicker::with_default_birthdays(Some(42));
        let day = date(2026, 5, 5);

        let first = picker.choose(day, 12, 0);
        let second = picker.choose(day, 12, 0);
        assert_eq!(
            first, second,
            "the uniform color must be cached in a minute"
        );
    }

    #[test]
    fn grey_days_ignore_the_table() {
        let mut picker = ColorPicker::with_default_birthdays(Some(42));

        assert_eq!(picker.choose(date(2026, 2, 18), 12, 0), GREY);
        assert_eq!(picker.choose(date(2026, 1, 1), 12, 0), GREY);
    }

    #[test]
    fn birthday_list_is_configurable() {
        let default = ColorPicker::with_default_birthdays(Some(1));
        assert!(default.is_birthday(date(2026, 4, 8)));

        let custom = ColorPicker::new(Some(1), vec![(6, 1)]);
        assert!(custom.is_birthday(date(2026, 6, 1)));
        assert!(!custom.is_birthday(date(2026, 4, 8)));
    }
}
