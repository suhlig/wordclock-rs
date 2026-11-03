//! A fixed-size ring of whole lines.
//!
//! The firmware uses this to keep the most recent log lines in RAM and serve
//! them over HTTP. It is plain data with no dependency on `log` or the ESP32, so
//! the wrap and truncation rules are tested on the host.

/// Bytes the ring can hold.
pub const CAPACITY: usize = 4096;

/// A byte ring that stores complete, newline-terminated lines and drops the
/// oldest line when it runs out of room.
///
/// Lines are only ever removed at a newline, so the contents always start at a
/// line boundary and remain valid UTF-8 when the caller writes UTF-8 lines.
pub struct Ring {
    bytes: [u8; CAPACITY],
    head: usize,
    len: usize,
}

impl Ring {
    /// An empty ring.
    pub const fn new() -> Self {
        Self {
            bytes: [0; CAPACITY],
            head: 0,
            len: 0,
        }
    }

    /// Appends one complete line, dropping whole lines from the front until it
    /// fits.
    pub fn push(&mut self, line: &[u8]) {
        let line = if line.len() > CAPACITY {
            &line[line.len() - CAPACITY..]
        } else {
            line
        };

        while self.len + line.len() > CAPACITY {
            self.drop_line();
        }

        let mut at = (self.head + self.len) % CAPACITY;
        for &byte in line {
            self.bytes[at] = byte;
            at = (at + 1) % CAPACITY;
        }
        self.len += line.len();
    }

    /// Copies the contents into `out`, oldest first, and returns the byte count.
    ///
    /// When `out` is smaller than the ring, the oldest bytes are skipped up to
    /// the next line start, so the result still begins at a whole line.
    pub fn snapshot(&self, out: &mut [u8]) -> usize {
        let mut start = self.head;
        let mut count = self.len;

        if count > out.len() {
            let skip = count - out.len();
            start = (start + skip) % CAPACITY;
            count -= skip;

            while count > 0 && self.bytes[start] != b'\n' {
                start = (start + 1) % CAPACITY;
                count -= 1;
            }
            if count > 0 {
                start = (start + 1) % CAPACITY;
                count -= 1;
            }
        }

        for (index, slot) in out.iter_mut().enumerate().take(count) {
            *slot = self.bytes[(start + index) % CAPACITY];
        }

        count
    }

    fn drop_line(&mut self) {
        while self.len > 0 {
            let byte = self.bytes[self.head];
            self.head = (self.head + 1) % CAPACITY;
            self.len -= 1;
            if byte == b'\n' {
                return;
            }
        }
    }
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::vec;

    use super::*;

    #[test]
    fn ring_keeps_the_newest_whole_lines() {
        let mut ring = Ring::new();
        for index in 0..CAPACITY {
            ring.push(format!("line {index}\n").as_bytes());
        }

        let mut out = vec![0; CAPACITY];
        let count = ring.snapshot(&mut out);
        let text = core::str::from_utf8(&out[..count]).expect("whole lines are UTF-8");

        assert!(text.starts_with("line "));
        assert!(text.ends_with('\n'));
        assert!(!text.contains('\0'));
    }

    #[test]
    fn a_line_longer_than_the_ring_is_truncated() {
        let mut ring = Ring::new();
        let line = vec![b'x'; CAPACITY * 2];

        ring.push(&line);

        let mut out = vec![0; CAPACITY];
        let count = ring.snapshot(&mut out);

        assert_eq!(count, CAPACITY);
        assert!(out[..count].iter().all(|byte| *byte == b'x'));
    }

    #[test]
    fn a_small_snapshot_starts_at_a_whole_line() {
        let mut ring = Ring::new();
        ring.push(b"aaaaaaaaaa\n");
        ring.push(b"bbbbbbbbbb\n");
        ring.push(b"cccccccccc\n");

        let mut out = vec![0; 12];
        let count = ring.snapshot(&mut out);

        assert_eq!(&out[..count], b"cccccccccc\n");
    }
}
