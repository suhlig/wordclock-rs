//! A volatile log ring served over HTTP.
//!
//! The clock is on a wall with no console, and the status page is unreachable
//! while the network is down. This keeps the most recent lines in RAM and
//! serves them at `GET /log`, so a reconnect or a failed association can be
//! explained afterwards without USB. The ring does not survive a reset or a
//! power cut; that limitation is deliberate, because a flash-backed log would
//! cost a sector and wear for information the counters already summarise.
//!
//! Lines are captured through the `log` facade, so every existing `info!` and
//! `error!` reaches both the USB console and the ring. The ring itself lives in
//! `wordclock-core`, where its wrap and truncation rules are host-tested.

use core::cell::{Cell, RefCell};
use core::fmt::{self, Write as _};

use critical_section::Mutex;
use esp_hal::time::Instant;
use log::{LevelFilter, Log, Metadata, Record};
use wordclock_core::logring::Ring;

/// Bytes the ring can hold, and the size of the snapshot a reader needs.
pub use wordclock_core::logring::CAPACITY;

/// Longest line kept. Longer messages are truncated rather than dropped.
const LINE_MAX: usize = 192;

/// Set while the ring is borrowed. A nested call (an interrupt logging while
/// the main context holds it) is skipped instead of panicking on an
/// already-borrowed `RefCell`.
static BUSY: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));

static LOG: Mutex<RefCell<Ring>> = Mutex::new(RefCell::new(Ring::new()));

/// The logger to install. The safe `log::set_logger` is unavailable on this
/// target because the C3 has no atomic read-modify-write instructions, so the
/// binary installs this with `set_logger_racy` in one documented place.
pub static LOGGER: Logger = Logger;

pub struct Logger;

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        log::max_level() >= metadata.level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let uptime = Instant::now().duration_since_epoch().as_secs();
        esp_println::println!("{uptime:>6} {} {}", record.level(), record.args());

        let mut line = Line::new();
        let _ = write!(line, "{uptime:>6} {} {}", record.level(), record.args());
        line.finish();
        append(line.as_bytes());
    }

    fn flush(&self) {}
}

/// The maximum level, taken from `ESP_LOG`, which `esp-println` used before
/// the ring existed, so an existing build configuration keeps working.
pub fn level_filter() -> LevelFilter {
    match option_env!("ESP_LOG") {
        Some("off") | Some("none") => LevelFilter::Off,
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

/// Copies the ring into `out`, oldest line first, and returns the byte count.
/// `out` must be at least [`CAPACITY`] bytes; the caller sizes it from that.
pub fn snapshot(out: &mut [u8]) -> usize {
    with_ring(|ring| ring.snapshot(out)).unwrap_or(0)
}

fn append(line: &[u8]) {
    let _ = with_ring(|ring| ring.push(line));
}

/// Runs `f` with the ring, or skips it when another context already holds it.
/// The busy flag and the borrow are separate so a nested call returns instead
/// of panicking on `RefCell`.
fn with_ring<R>(f: impl FnOnce(&mut Ring) -> R) -> Option<R> {
    let acquired = critical_section::with(|cs| {
        let busy = BUSY.borrow(cs);
        if busy.get() {
            false
        } else {
            busy.set(true);
            true
        }
    });

    if !acquired {
        return None;
    }

    let result = critical_section::with(|cs| f(&mut LOG.borrow_ref_mut(cs)));
    critical_section::with(|cs| BUSY.borrow(cs).set(false));

    Some(result)
}

/// A fixed-size line buffer. Keeps one byte for the newline so a truncated
/// message still ends a line.
struct Line {
    bytes: [u8; LINE_MAX],
    len: usize,
}

impl Line {
    const fn new() -> Self {
        Self {
            bytes: [0; LINE_MAX],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn finish(&mut self) {
        if self.len < self.bytes.len() {
            self.bytes[self.len] = b'\n';
            self.len += 1;
        }
    }
}

impl fmt::Write for Line {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let room = self.bytes.len().saturating_sub(1).saturating_sub(self.len);
        let take = text.len().min(room);
        let take = (0..=take)
            .rev()
            .find(|&end| text.is_char_boundary(end))
            .unwrap_or(0);

        self.bytes[self.len..self.len + take].copy_from_slice(&text.as_bytes()[..take]);
        self.len += take;

        Ok(())
    }
}
