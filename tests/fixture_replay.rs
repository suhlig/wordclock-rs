use serde::Deserialize;
use wordclock::clock;

#[derive(Debug, Deserialize)]
struct FixtureEntry {
    time: String,
    pixels: Vec<usize>,
    words: String,
}

fn fixture() -> Vec<FixtureEntry> {
    serde_json::from_str(include_str!("../fixtures/minutes.json"))
        .expect("fixtures/minutes.json should be valid JSON")
}

fn parse_time(time: &str) -> (u32, u32) {
    let (hour, minute) = time.split_once(':').expect("fixture time should be HH:MM");
    let hour = hour.parse().expect("valid hour");
    let minute = minute.parse().expect("valid minute");
    assert!(hour < 24, "hour out of range in {time}");
    assert!(minute < 60, "minute out of range in {time}");
    (hour, minute)
}

#[test]
fn fixture_covers_every_minute_once() {
    let entries = fixture();
    assert_eq!(entries.len(), 1_440);

    let mut times: Vec<&str> = entries.iter().map(|entry| entry.time.as_str()).collect();
    times.sort_unstable();
    times.dedup();
    assert_eq!(times.len(), 1_440, "fixture times must be unique");
}

#[test]
fn renders_every_fixture_minute() {
    for entry in fixture() {
        let (hour, minute) = parse_time(&entry.time);
        let pixels = clock::pixels(hour, minute);

        assert_eq!(
            pixels, entry.pixels,
            "pixel list mismatch for {}",
            entry.time
        );
        assert_eq!(
            clock::words(&pixels),
            entry.words,
            "word string mismatch for {}",
            entry.time
        );
    }
}
