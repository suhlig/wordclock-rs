//! The 288-character stripe and the exact legacy word lookup.
//!
//! The wording and the lookup quirks are frozen by the golden fixture. This
//! module deliberately has no host or output-transport dependency, so the same
//! renderer can run in the simulator, on the Pi, or on the ESP32-C3.
//!
//! Word separation is geometric: `words` concatenates only the lit stripe
//! characters, and dark cells (or a row break) are the spaces. Do not add space
//! characters or "space pixels".

use crate::COLS;

/// The legacy 16×18 word stripe, exactly as published by the word clock.
///
/// Each line is one row. The string contains 288 characters, including the
/// German umlauts; pixel indices are character indices, not byte offsets.
pub const STRIPE: &str = concat!(
    "ESAISTOVIERTELEINS",
    "DREINERSECHSIEBENE",
    "ELFÜNFNEUNVIERACHT",
    "NULLZWEINZWÖLFZEHN",
    "UNDOZWANZIGVIERZIG",
    "DREISSIGFÜNFZIGUHR",
    "MINUTENIVORUNDNACH",
    "EINDREIVIERTELHALB",
    "SIEBENEUNULLZWEINE",
    "FÜNFSECHSNACHTVIER",
    "DREINSUNDAELFEZEHN",
    "ZWANZIGGRADREISSIG",
    "VIERZIGZWÖLFÜNFZIG",
    "MINUTENUHREFRÜHVOR",
    "ABENDSMITTERNACHTS",
    "MORGENSWARMMITTAGS",
);

const MINUTES: [&[&str]; 60] = [
    &[],
    &["EINS"],
    &["ZWEI"],
    &["DREI"],
    &["VIER"],
    &["FÜNF"],
    &["SECHS"],
    &["SIEBEN"],
    &["ACHT"],
    &["NEUN"],
    &["ZEHN"],
    &["ELF"],
    &["ZWÖLF"],
    &["DREI", "ZEHN"],
    &["VIER", "ZEHN"],
    &["FÜNF", "ZEHN"],
    &["SECH", "ZEHN"],
    &["SIEB", "ZEHN"],
    &["ACHT", "ZEHN"],
    &["NEUN", "ZEHN"],
    &["ZWANZIG"],
    &["EIN", "UND", "ZWANZIG"],
    &["ZWEI", "UND", "ZWANZIG"],
    &["DREI", "UND", "ZWANZIG"],
    &["VIER", "UND", "ZWANZIG"],
    &["FÜNF", "UND", "ZWANZIG"],
    &["SECHS", "UND", "ZWANZIG"],
    &["SIEBEN", "UND", "ZWANZIG"],
    &["ACHT", "UND", "ZWANZIG"],
    &["NEUN", "UND", "ZWANZIG"],
    &["DREISSIG"],
    &["EIN", "UND", "DREISSIG"],
    &["ZWEI", "UND", "DREISSIG"],
    &["DREI", "UND", "DREISSIG"],
    &["VIER", "UND", "DREISSIG"],
    &["FÜNF", "UND", "DREISSIG"],
    &["SECHS", "UND", "DREISSIG"],
    &["SIEBEN", "UND", "DREISSIG"],
    &["ACHT", "UND", "DREISSIG"],
    &["NEUN", "UND", "DREISSIG"],
    &["VIERZIG"],
    &["EIN", "UND", "VIERZIG"],
    &["ZWEI", "UND", "VIERZIG"],
    &["DREI", "UND", "VIERZIG"],
    &["VIER", "UND", "VIERZIG"],
    &["FÜNF", "UND", "VIERZIG"],
    &["SECHS", "UND", "VIERZIG"],
    &["SIEBEN", "UND", "VIERZIG"],
    &["ACHT", "UND", "VIERZIG"],
    &["NEUN", "UND", "VIERZIG"],
    &["FÜNFZIG"],
    &["EIN", "UND", "FÜNFZIG"],
    &["ZWEI", "UND", "FÜNFZIG"],
    &["DREI", "UND", "FÜNFZIG"],
    &["VIER", "UND", "FÜNFZIG"],
    &["FÜNF", "UND", "FÜNFZIG"],
    &["SECHS", "UND", "FÜNFZIG"],
    &["SIEBEN", "UND", "FÜNFZIG"],
    &["ACHT", "UND", "FÜNFZIG"],
    &["NEUN", "UND", "FÜNFZIG"],
];

const HOURS: [&[&str]; 24] = [
    &["NULL"],
    &["EIN"],
    &["ZWEI"],
    &["DREI"],
    &["VIER"],
    &["FÜNF"],
    &["SECHS"],
    &["SIEBEN"],
    &["ACHT"],
    &["NEUN"],
    &["ZEHN"],
    &["ELF"],
    &["ZWÖLF"],
    &["DREI", "ZEHN"],
    &["VIER", "ZEHN"],
    &["FÜNF", "ZEHN"],
    &["SECH", "ZEHN"],
    &["SIEB", "ZEHN"],
    &["ACHT", "ZEHN"],
    &["NEUN", "ZEHN"],
    &["ZWANZIG"],
    &["EIN", "UND", "ZWANZIG"],
    &["ZWEI", "UND", "ZWANZIG"],
    &["DREI", "UND", "ZWANZIG"],
];

macro_rules! phrase {
    ($($part:expr),+ $(,)?) => {{
        let mut words: Vec<&str> = Vec::new();
        $(words.extend_from_slice($part);)+
        words
    }};
}

/// Returns the lit pixel indices for a local time, in ascending order.
///
/// This is a direct port of `WordClock::Clock24#pixels`; the branches and their
/// order are intentional and are pinned by the golden fixture.
pub fn pixels(hour: u32, minute: u32) -> Vec<usize> {
    assert!(hour < 24, "hour must be 0..=23, got {hour}");
    assert!(minute < 60, "minute must be 0..=59, got {minute}");

    if hour == 0 {
        if minute == 0 {
            return lookup(&phrase!(&["ES"], &["IST"], &["MITTERNACHT"]));
        }
        if minute == 1 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["EINS"],
                &["NACH"],
                &["MITTERNACHT"]
            ));
        }
        if (2..=5).contains(&minute) || minute == 10 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                minute_words(minute),
                &["NACH"],
                &["MITTERNACHT"]
            ));
        }
        if minute == 15 {
            return lookup(&phrase!(&["ES"], &["IST"], &["VIERTEL"], &["EINS"]));
        }
        if minute == 20 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["ZEHN"],
                &["VOR"],
                &["HALB"],
                &["EINS"]
            ));
        }
        if minute == 25 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["FÜNF"],
                &["VOR"],
                &["HALB"],
                &["EINS"]
            ));
        }
        if minute == 30 {
            return lookup(&phrase!(&["ES"], &["IST"], &["HALB"], &["EINS"]));
        }
        if minute == 35 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["FÜNF"],
                &["NACH"],
                &["HALB"],
                &["EINS"]
            ));
        }
        if minute == 40 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["ZEHN"],
                &["NACH"],
                &["HALB"],
                &["EINS"]
            ));
        }
        if minute == 45 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["DREI"],
                &["VIERTEL"],
                &["EINS"]
            ));
        }
        if minute == 50 {
            return lookup(&phrase!(&["ES"], &["IST"], &["ZEHN"], &["VOR"], &["EINS"]));
        }
        if minute == 55 {
            return lookup(&phrase!(&["ES"], &["IST"], &["FÜNF"], &["VOR"], &["EINS"]));
        }
        if (56..=59).contains(&minute) {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                minute_words(60 - minute),
                &["VOR"],
                &["EINS"]
            ));
        }
    }

    if hour == 1 {
        if minute == 5 {
            return lookup(&phrase!(&["ES"], &["IST"], &["FÜNF"], &["NACH"], &["EINS"]));
        }
        if minute == 10 {
            return lookup(&phrase!(&["ES"], &["IST"], &["ZEHN"], &["NACH"], &["EINS"]));
        }
    }

    if hour < 12 {
        if minute == 5 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["FÜNF"],
                &["NACH"],
                hour_words(hour)
            ));
        }
        if minute == 10 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["ZEHN"],
                &["NACH"],
                hour_words(hour)
            ));
        }
        if minute == 15 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["VIERTEL"],
                hour_words(hour + 1)
            ));
        }
        if minute == 20 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["ZEHN"],
                &["VOR"],
                &["HALB"],
                hour_words(hour + 1)
            ));
        }
        if minute == 25 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["FÜNF"],
                &["VOR"],
                &["HALB"],
                hour_words(hour + 1)
            ));
        }
        if minute == 30 {
            return lookup(&phrase!(&["ES"], &["IST"], &["HALB"], hour_words(hour + 1)));
        }
        if minute == 35 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["FÜNF"],
                &["NACH"],
                &["HALB"],
                hour_words(hour + 1)
            ));
        }
        if minute == 40 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["ZEHN"],
                &["NACH"],
                &["HALB"],
                hour_words(hour + 1)
            ));
        }
        if minute == 45 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["DREI"],
                &["VIERTEL"],
                hour_words(hour + 1)
            ));
        }
        if minute == 50 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["ZEHN"],
                &["VOR"],
                hour_words(hour + 1)
            ));
        }
        if minute == 55 {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                &["FÜNF"],
                &["VOR"],
                hour_words(hour + 1)
            ));
        }
        if (56..=59).contains(&minute) {
            return lookup(&phrase!(
                &["ES"],
                &["IST"],
                minute_words(60 - minute),
                &["VOR"],
                hour_words(hour + 1)
            ));
        }
    }

    if hour == 23 && (55..=59).contains(&minute) {
        return lookup(&phrase!(
            &["ES"],
            &["IST"],
            minute_words(60 - minute),
            &["VOR"],
            &["MITTERNACHT"]
        ));
    }

    lookup(&phrase!(
        &["ES"],
        &["IST"],
        hour_words(hour),
        &["UHR"],
        minute_words(minute)
    ))
}

/// Returns the concatenated stripe characters for the given indices.
///
/// The result intentionally contains no spaces; the grid's dark cells and row
/// breaks carry the word separation.
pub fn words(pixels: &[usize]) -> String {
    let chars: Vec<char> = STRIPE.chars().collect();
    pixels.iter().map(|&index| chars[index]).collect()
}

fn hour_words(hour: u32) -> &'static [&'static str] {
    HOURS[hour as usize]
}

fn minute_words(minute: u32) -> &'static [&'static str] {
    MINUTES[minute as usize]
}

fn lookup(words: &[&str]) -> Vec<usize> {
    let chars: Vec<char> = STRIPE.chars().collect();
    let mut index = 0usize;
    let mut lit = Vec::new();

    for word in words {
        let needle: Vec<char> = word.chars().collect();
        let start = find_word(&chars, &needle, index)
            .unwrap_or_else(|| panic!("word {word:?} not found at or after index {index}"));
        let end = start + needle.len();

        lit.extend(start..end);
        index = end;
        if !index.is_multiple_of(COLS) {
            index += 1;
        }
    }

    lit
}

fn find_word(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from);
    }
    if from + needle.len() > haystack.len() {
        return None;
    }

    (from..=haystack.len() - needle.len()).find(|&start| {
        haystack[start..start + needle.len()]
            .iter()
            .eq(needle.iter())
    })
}

#[cfg(test)]
mod tests {
    use super::{STRIPE, pixels, words};
    use crate::{COLS, ROWS};

    #[test]
    fn stripe_has_288_characters_in_16_rows_of_18() {
        assert_eq!(STRIPE.chars().count(), 288);
        assert_eq!(STRIPE.chars().count(), COLS * ROWS);
    }

    #[test]
    fn midnight_and_six_thirty_match_the_legacy_phrases() {
        assert_eq!(words(&pixels(0, 0)), "ESISTMITTERNACHT");
        assert_eq!(words(&pixels(6, 30)), "ESISTHALBSIEBEN");
    }

    #[test]
    fn row_boundary_word_is_split_by_the_grid() {
        let lit = pixels(6, 30);
        let halb = &lit[lit.iter().position(|&i| i == 140).expect("HALB start")..];
        assert_eq!(halb, &[140, 141, 142, 143, 144, 145, 146, 147, 148, 149]);
    }
}
