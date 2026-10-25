//! The `Europe/Berlin` wall-clock rule for the ESP32-C3 firmware.
//!
//! The firmware receives UTC from SNTP and needs local wall-clock time without
//! pulling the full timezone database onto the device. The modern EU rule
//! (1996 onward) is enough for the clock: CEST starts on the last Sunday in
//! March at 01:00 UTC and ends on the last Sunday in October at 01:00 UTC.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};

/// Converts a UTC timestamp to `Europe/Berlin` local wall-clock time.
pub fn berlin_local_from_utc(utc: NaiveDateTime) -> NaiveDateTime {
    let (start, end) = dst_bounds(utc.year());
    let offset_hours = if utc >= start && utc < end { 2 } else { 1 };

    utc + Duration::hours(offset_hours)
}

/// The UTC instants at which CEST starts and ends in the given year.
pub fn dst_bounds(year: i32) -> (NaiveDateTime, NaiveDateTime) {
    let start = last_sunday(year, 3)
        .and_hms_opt(1, 0, 0)
        .expect("valid start time");
    let end = last_sunday(year, 10)
        .and_hms_opt(1, 0, 0)
        .expect("valid end time");

    (start, end)
}

fn last_sunday(year: i32, month: u32) -> NaiveDate {
    let mut date = NaiveDate::from_ymd_opt(year, month, 31).expect("month has 31 days");

    while date.weekday() != Weekday::Sun {
        date = date.pred_opt().expect("date stays in range");
    }

    date
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use chrono_tz::Europe::Berlin;

    use super::*;

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(hour, minute, 0)
            .expect("valid time")
    }

    #[test]
    fn spring_transition_2026() {
        // 29 March 2026, 01:00 UTC: 02:00 CET becomes 03:00 CEST.
        assert_eq!(
            berlin_local_from_utc(utc(2026, 3, 29, 0, 59)),
            utc(2026, 3, 29, 1, 59)
        );
        assert_eq!(
            berlin_local_from_utc(utc(2026, 3, 29, 1, 0)),
            utc(2026, 3, 29, 3, 0)
        );
    }

    #[test]
    fn autumn_transition_2026() {
        // 25 October 2026, 01:00 UTC: 03:00 CEST becomes 02:00 CET.
        assert_eq!(
            berlin_local_from_utc(utc(2026, 10, 25, 0, 59)),
            utc(2026, 10, 25, 2, 59)
        );
        assert_eq!(
            berlin_local_from_utc(utc(2026, 10, 25, 1, 0)),
            utc(2026, 10, 25, 2, 0)
        );
    }

    #[test]
    fn matches_chrono_tz_through_2030() {
        let mut current = utc(2026, 1, 1, 0, 0);
        let end = utc(2030, 12, 31, 23, 0);

        while current <= end {
            let expected = Berlin.from_utc_datetime(&current).naive_local();

            assert_eq!(
                berlin_local_from_utc(current),
                expected,
                "mismatch at {current}"
            );

            current += Duration::hours(1);
        }
    }
}
