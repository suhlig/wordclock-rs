use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use wordclock::color::Rgb;
use wordclock::sim::{self, RenderOptions};

/// Word clock for a 288-pixel WS2812 strip.
#[derive(Debug, Parser)]
#[command(name = "wordclock", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the clock daemon.
    Run {
        /// Path to the TOML configuration file.
        #[arg(long, default_value = "config.toml")]
        config: PathBuf,
    },

    /// Render a single frame to the terminal.
    Sim {
        /// Time to render as `DD.MM.YYYY HH:MM` or ISO 8601. Defaults to now.
        time: Option<String>,

        /// Fixed pixel color as RRGGBB, disabling random sampling.
        #[arg(long, value_name = "RRGGBB")]
        color: Option<String>,

        /// Seed for deterministic random color sampling.
        #[arg(long)]
        seed: Option<u64>,
    },

    /// Walk all 1,440 minutes of a day in the terminal.
    Demo {
        /// Seconds to wait between frames.
        #[arg(long, default_value_t = 0.2)]
        interval: f64,
    },

    /// Drive the hardware test patterns on the strip.
    TestPattern {
        /// Pattern to show: walking, rows, primaries, or all.
        #[arg(long, default_value = "walking")]
        pattern: String,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Run { .. } => not_implemented("run"),
        Command::Sim { time, color, seed } => {
            let options = RenderOptions {
                color: parse_color(color)?,
                seed,
                ..RenderOptions::default()
            };
            let datetime = match time {
                Some(time) => sim::parse_time(&time)?,
                None => sim::now(),
            };

            sim::run(datetime, options)
        }
        Command::Demo { interval } => sim::demo(RenderOptions::default(), interval),
        Command::TestPattern { .. } => {
            #[cfg(not(feature = "hardware"))]
            anyhow::bail!("`wordclock test-pattern` requires a build with the `hardware` feature");

            #[cfg(feature = "hardware")]
            not_implemented("test-pattern")
        }
    }
}

fn parse_color(value: Option<String>) -> Result<Option<Rgb>> {
    value
        .map(|value| {
            Rgb::from_hex(&value).with_context(|| format!("invalid RRGGBB color {value:?}"))
        })
        .transpose()
}

fn not_implemented(name: &str) -> Result<()> {
    anyhow::bail!("`wordclock {name}` is not implemented yet")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
