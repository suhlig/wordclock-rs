//! Prints a canonical frame dump for the host/ESP32-C3 parity check in task 4.6.
//!
//! The format is deliberately plain text so the same dump can be produced on
//! the board and compared with `diff`.
//!
//! Usage: `cargo run --example dump_frame -- "15.07.2026 06:30" [--color RRGGBB] [--seed N]`

use anyhow::{Context, Result, bail};
use wordclock::color::{ColorPicker, GammaLut, Rgb};
use wordclock::sim::{self, RenderOptions};

fn main() -> Result<()> {
    let mut time: Option<String> = None;
    let mut color: Option<Rgb> = None;
    let mut seed: Option<u64> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--color" => {
                let value = args.next().context("--color needs an RRGGBB value")?;
                color = Some(
                    Rgb::from_hex(&value)
                        .with_context(|| format!("invalid RRGGBB color {value:?}"))?,
                );
            }
            "--seed" => {
                seed = Some(
                    args.next()
                        .context("--seed needs a value")?
                        .parse()
                        .context("--seed must be a u64")?,
                );
            }
            other => {
                if time.is_some() {
                    bail!("unexpected argument {other:?}");
                }
                time = Some(other.to_string());
            }
        }
    }

    let time = time.context("usage: dump_frame <time> [--color RRGGBB] [--seed N]")?;
    let datetime = sim::parse_time(&time)?;

    let options = RenderOptions {
        color,
        seed,
        ..RenderOptions::default()
    };

    let mut picker = ColorPicker::new(options.seed, options.birthdays.clone());
    picker.begin_minute();
    let frame = sim::render(datetime, &mut picker, &options);
    let corrected = frame.corrected_logical(&GammaLut::new(options.whitepoint));

    let color_label = match options.color {
        Some(color) => format!("{:02x}{:02x}{:02x}", color.r, color.g, color.b),
        None => "none".to_string(),
    };
    let seed_label = options
        .seed
        .map_or_else(|| "none".to_string(), |seed| seed.to_string());

    println!(
        "FRAME time={} color={} seed={}",
        datetime.format("%Y-%m-%dT%H:%M"),
        color_label,
        seed_label
    );

    for (index, (&raw, &shown)) in frame.logical().iter().zip(corrected.iter()).enumerate() {
        println!(
            "{index} {:02x}{:02x}{:02x} {:02x}{:02x}{:02x}",
            raw.r, raw.g, raw.b, shown.r, shown.g, shown.b
        );
    }

    println!("END");

    Ok(())
}
