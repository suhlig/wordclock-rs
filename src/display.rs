//! Output backends.
//!
//! The terminal and null backends always build. The SPI backend for
//! `/dev/spidev0.0` is compiled only with the `hardware` feature, which is
//! also what pulls in `rppal` and `ws2812-spi`. The ESP32-C3 backend will use
//! RMT and lives in the firmware crate if that host is selected.

use std::io::{self, Write};

use crate::clock::STRIPE;
use crate::color::Rgb;
use crate::{COLS, LOGICAL_PIXELS, ROWS};

/// Whether SPI hardware support was compiled into this build.
pub const HARDWARE_ENABLED: bool = cfg!(feature = "hardware");

/// Receives corrected pixels in physical strip order.
pub trait Display {
    /// Writes one complete frame.
    fn write(&mut self, pixels: &[Rgb]) -> io::Result<()>;
}

/// A display that discards every frame; useful for tests and dry runs.
#[derive(Debug, Default)]
pub struct NullDisplay;

impl Display for NullDisplay {
    fn write(&mut self, _pixels: &[Rgb]) -> io::Result<()> {
        Ok(())
    }
}

/// Renders the word grid to an ANSI-colored terminal.
#[derive(Debug)]
pub struct TerminalDisplay<W: Write> {
    writer: W,
    clear: bool,
}

impl<W: Write> TerminalDisplay<W> {
    /// Creates a terminal display that appends each frame.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            clear: false,
        }
    }

    /// Creates a terminal display that clears the screen before each frame.
    pub fn with_clear(writer: W, clear: bool) -> Self {
        Self { writer, clear }
    }

    /// Flushes the underlying writer.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W: Write> Display for TerminalDisplay<W> {
    fn write(&mut self, pixels: &[Rgb]) -> io::Result<()> {
        if pixels.len() < LOGICAL_PIXELS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "expected at least {LOGICAL_PIXELS} pixels, got {}",
                    pixels.len()
                ),
            ));
        }

        if self.clear {
            write!(self.writer, "\x1b[2J\x1b[H")?;
        }

        writeln!(self.writer, "+{}+", "-".repeat(COLS * 2 + 1))?;

        let chars: Vec<char> = STRIPE.chars().collect();
        for row in 0..ROWS {
            write!(self.writer, "| ")?;

            for column in 0..COLS {
                if column > 0 {
                    write!(self.writer, " ")?;
                }

                let index = row * COLS + column;
                let color = pixels[index];
                let (red, green, blue) = if color == Rgb::BLACK {
                    (43, 43, 43)
                } else {
                    (color.r, color.g, color.b)
                };

                write!(
                    self.writer,
                    "\x1b[38;2;{red};{green};{blue}m{}\x1b[0m",
                    chars[index]
                )?;
            }

            writeln!(self.writer, " |")?;
        }

        writeln!(self.writer, "+{}+", "-".repeat(COLS * 2 + 1))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(input: &str) -> String {
        let mut output = String::new();
        let mut chars = input.chars();

        while let Some(character) = chars.next() {
            if character == '\x1b' {
                for character in chars.by_ref() {
                    if character.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                output.push(character);
            }
        }

        output
    }

    #[test]
    fn layout_has_16_rows_of_18_cells() {
        let mut output = Vec::new();
        let mut display = TerminalDisplay::new(&mut output);
        display
            .write(&vec![Rgb::BLACK; LOGICAL_PIXELS])
            .expect("write should succeed");

        let text = String::from_utf8(output).expect("terminal output is UTF-8");
        let plain = strip_ansi(&text);
        let lines: Vec<&str> = plain.lines().collect();

        assert_eq!(lines.len(), ROWS + 2);
        assert!(lines[0].starts_with('+'));
        assert!(lines[0].ends_with('+'));

        for line in &lines[1..=ROWS] {
            assert_eq!(
                line.chars()
                    .filter(|character| character.is_alphabetic())
                    .count(),
                COLS
            );
        }
    }

    #[test]
    fn unlit_cells_use_the_legacy_dim_gray() {
        let mut output = Vec::new();
        let mut display = TerminalDisplay::new(&mut output);
        display
            .write(&vec![Rgb::BLACK; LOGICAL_PIXELS])
            .expect("write should succeed");

        let text = String::from_utf8(output).expect("terminal output is UTF-8");
        assert!(text.contains("\x1b[38;2;43;43;43m"));
    }
}
