use std::collections::HashSet;

use chrono::{NaiveDate, NaiveDateTime};
use wordclock::color::{ColorPicker, GammaLut, Rgb};
use wordclock::display::{Display, TerminalDisplay};
use wordclock::sim::{self, RenderOptions};

fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(year, month, day)
        .expect("valid date")
        .and_hms_opt(hour, minute, 0)
        .expect("valid time")
}

fn render_at(datetime: NaiveDateTime, options: &RenderOptions) -> wordclock::frame::Frame {
    let mut picker = ColorPicker::new(options.seed, options.birthdays.clone());
    picker.begin_minute();
    sim::render(datetime, &mut picker, options)
}

/// Extracts the RGB values from `ESC[38;2;r;g;bm` sequences.
fn ansi_colors(text: &str) -> Vec<Rgb> {
    let mut colors = Vec::new();
    let mut rest = text;

    while let Some(position) = rest.find("\x1b[38;2;") {
        rest = &rest[position + 7..];
        let end = rest
            .find('m')
            .expect("ANSI color sequence should end with m");
        let channels: Vec<u8> = rest[..end]
            .split(';')
            .map(|channel| channel.parse().expect("ANSI channel should be a number"))
            .collect();

        assert_eq!(channels.len(), 3, "expected three ANSI channels");
        colors.push(Rgb::new(channels[0], channels[1], channels[2]));
        rest = &rest[end + 1..];
    }

    colors
}

#[test]
fn terminal_and_frame_pipeline_agree() {
    let options = RenderOptions {
        color: Some(Rgb::new(128, 64, 32)),
        ..RenderOptions::default()
    };
    let frame = render_at(at(2026, 5, 5, 12, 0), &options);
    let lut = GammaLut::new(options.whitepoint);
    let corrected = frame.corrected_logical(&lut);
    let physical = frame.corrected_physical(&lut);

    assert_eq!(&physical[..corrected.len()], corrected.as_slice());

    let mut output = Vec::new();
    let mut display = TerminalDisplay::new(&mut output);
    display
        .write(&physical)
        .expect("terminal write should succeed");

    let text = String::from_utf8(output).expect("terminal output is UTF-8");
    let colors = ansi_colors(&text);
    assert_eq!(colors.len(), corrected.len());

    for (index, (&terminal, &expected)) in colors.iter().zip(&corrected).enumerate() {
        if expected == Rgb::BLACK {
            assert_eq!(
                terminal,
                Rgb::new(43, 43, 43),
                "unlit cell {index} should use the dim gray"
            );
        } else {
            assert_eq!(
                terminal, expected,
                "lit cell {index} should use the corrected color"
            );
        }
    }
}

#[test]
fn same_seed_produces_the_same_frame() {
    let options = RenderOptions {
        seed: Some(7),
        ..RenderOptions::default()
    };
    let datetime = at(2026, 4, 8, 12, 0);

    assert_eq!(
        render_at(datetime, &options),
        render_at(datetime, &options),
        "the same seed and minute must reproduce the frame"
    );
}

#[test]
fn ash_wednesday_is_grey() {
    let options = RenderOptions {
        seed: Some(3),
        ..RenderOptions::default()
    };
    let frame = render_at(at(2026, 2, 18, 12, 0), &options);
    let lit: Vec<Rgb> = frame
        .logical()
        .iter()
        .copied()
        .filter(|&color| color != Rgb::BLACK)
        .collect();

    assert!(!lit.is_empty());
    assert!(lit.iter().all(|&color| color == Rgb::new(64, 64, 64)));
}

#[test]
fn carnival_samples_per_pixel() {
    let options = RenderOptions {
        seed: Some(3),
        ..RenderOptions::default()
    };
    let frame = render_at(at(2026, 2, 16, 12, 0), &options);
    let distinct: HashSet<Rgb> = frame
        .logical()
        .iter()
        .copied()
        .filter(|&color| color != Rgb::BLACK)
        .collect();

    assert!(
        distinct.len() > 1,
        "Rosenmontag should sample independently per lit pixel"
    );
}

#[test]
fn birthday_samples_per_pixel() {
    let options = RenderOptions {
        seed: Some(3),
        ..RenderOptions::default()
    };
    let frame = render_at(at(2026, 4, 8, 12, 0), &options);
    let distinct: HashSet<Rgb> = frame
        .logical()
        .iter()
        .copied()
        .filter(|&color| color != Rgb::BLACK)
        .collect();

    assert!(
        distinct.len() > 1,
        "a default birthday should sample independently per lit pixel"
    );
}

#[test]
fn new_year_party_samples_per_pixel() {
    let options = RenderOptions {
        seed: Some(3),
        ..RenderOptions::default()
    };
    let frame = render_at(at(2026, 1, 1, 0, 30), &options);
    let distinct: HashSet<Rgb> = frame
        .logical()
        .iter()
        .copied()
        .filter(|&color| color != Rgb::BLACK)
        .collect();

    assert!(
        distinct.len() > 1,
        "the New Year party should sample independently per lit pixel"
    );
}
