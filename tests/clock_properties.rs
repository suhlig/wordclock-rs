use wordclock::clock;
use wordclock::{COLS, LOGICAL_PIXELS};

const MIN_LIT: usize = 11;
const MAX_LIT: usize = 39;

/// Returns the maximal contiguous runs of lit indices as `(start, end)` pairs.
fn runs(pixels: &[usize]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = pixels[0];
    let mut previous = pixels[0];

    for &pixel in &pixels[1..] {
        if pixel == previous + 1 {
            previous = pixel;
        } else {
            runs.push((start, previous));
            start = pixel;
            previous = pixel;
        }
    }

    runs.push((start, previous));
    runs
}

fn minutes() -> impl Iterator<Item = (u32, u32)> {
    (0..24).flat_map(|hour| (0..60).map(move |minute| (hour, minute)))
}

#[test]
fn every_minute_has_non_empty_ascending_indices_below_288() {
    for (hour, minute) in minutes() {
        let pixels = clock::pixels(hour, minute);
        let time = format!("{hour:02}:{minute:02}");

        assert!(!pixels.is_empty(), "{time} produced no lit pixels");
        assert!(
            pixels.windows(2).all(|pair| pair[0] < pair[1]),
            "{time} indices are not strictly increasing: {pixels:?}"
        );
        assert!(
            pixels.iter().all(|&pixel| pixel < LOGICAL_PIXELS),
            "{time} contains an index outside 0..288: {pixels:?}"
        );
    }
}

#[test]
fn every_minute_lights_between_11_and_39_pixels() {
    for (hour, minute) in minutes() {
        let lit = clock::pixels(hour, minute).len();
        assert!(
            (MIN_LIT..=MAX_LIT).contains(&lit),
            "{hour:02}:{minute:02} lit {lit} pixels, expected {MIN_LIT}..={MAX_LIT}"
        );
    }
}

#[test]
fn same_row_runs_have_at_least_one_dark_cell_between_them() {
    for (hour, minute) in minutes() {
        let pixels = clock::pixels(hour, minute);

        for pair in runs(&pixels).windows(2) {
            let (_, previous_end) = pair[0];
            let (next_start, _) = pair[1];

            if previous_end / COLS == next_start / COLS {
                assert!(
                    next_start > previous_end + 1,
                    "{hour:02}:{minute:02} has adjacent same-row runs ending at {previous_end} and starting at {next_start}"
                );
            }
        }
    }
}

#[test]
fn only_halb_sieben_crosses_a_row_boundary() {
    let mut crossings = Vec::new();

    for (hour, minute) in minutes() {
        let pixels = clock::pixels(hour, minute);

        for (start, end) in runs(&pixels) {
            if start / COLS != end / COLS {
                crossings.push((hour, minute, start, end));
            }
        }
    }

    assert_eq!(
        crossings,
        vec![
            (6, 20, 140, 149),
            (6, 25, 140, 149),
            (6, 30, 140, 149),
            (6, 35, 140, 149),
            (6, 40, 140, 149),
        ],
        "the only row-crossing run must be HALB/SIEBEN at 06:20 through 06:40"
    );
}

#[test]
fn halb_sieben_is_split_across_rows_7_and_8() {
    let stripe: Vec<char> = clock::STRIPE.chars().collect();

    for minute in [20, 25, 30, 35, 40] {
        let pixels = clock::pixels(6, minute);

        assert!(
            (140..=149).all(|index| pixels.contains(&index)),
            "06:{minute:02} is missing part of HALB/SIEBEN: {pixels:?}"
        );
        assert_eq!(
            runs(&pixels).into_iter().find(|&(start, _)| start == 140),
            Some((140, 149)),
            "06:{minute:02} must contain the split HALB/SIEBEN run"
        );

        let halb: String = (140..=143).map(|index| stripe[index]).collect();
        let sieben: String = (144..=149).map(|index| stripe[index]).collect();
        assert_eq!(halb, "HALB");
        assert_eq!(sieben, "SIEBEN");
        assert_eq!(140 / COLS, 7);
        assert_eq!(143 / COLS, 7);
        assert_eq!(144 / COLS, 8);
        assert_eq!(149 / COLS, 8);
        assert!(clock::words(&pixels).ends_with("HALBSIEBEN"));
    }
}
