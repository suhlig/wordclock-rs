# ESP32-C3 frame parity spike

This is the gate artifact for task 4.6: it renders the 06:30 frame on the ESP32-C3 and prints the same dump the host simulator prints, so the two can be compared with `diff`.

The library does not copy the core code. It includes the real `src/clock.rs`, `src/color.rs`, and `src/frame.rs` from the repository root through `#[path]`, with the crate-root constants and `extern crate alloc` that those modules expect. That makes the spike a compile check of the `no_std` + `alloc` boundary described in `docs/esp32-c3-spike.md` section 4.1 as well as a behavioral check.

- Board: ESP32-C3-DevKit-RUST-1
- No Wi-Fi, no RMT: this spike only computes and prints
- Scenarios: fixed color on a regular day, seeded XKCD color on a regular day, and per-pixel seeded colors on Rosenmontag

## Build, flash, monitor

```bash
cargo build --release
espflash flash --chip esp32c3 --port /dev/cu.usbmodem1101 target/riscv32imc-unknown-none-elf/release/esp32c3-frame
espflash monitor --non-interactive --chip esp32c3 --port /dev/cu.usbmodem1101 > /tmp/device_raw.txt
```

The firmware waits 8 seconds after boot so the monitor can attach, prints the three frames, and repeats the set every 30 seconds. The monitor command never exits on its own; stop it after one set has been captured.

## Compare

Generate the host dumps and concatenate them in the same order:

```bash
cargo run --example dump_frame -- "15.07.2026 06:30" --color c04020 > /tmp/host_a.txt
cargo run --example dump_frame -- "15.07.2026 06:30" --seed 42 > /tmp/host_b.txt
cargo run --example dump_frame -- "16.02.2026 06:30" --seed 42 > /tmp/host_c.txt
cat /tmp/host_a.txt /tmp/host_b.txt /tmp/host_c.txt > /tmp/host_all.txt
```

The serial output uses CRLF and ends the set with `END OF SET`, so normalize before diffing:

```bash
tr -d '\r' < /tmp/device_raw.txt > /tmp/device_clean.txt
awk '/^FRAME /{n++} /^END OF SET/{exit} n>=1 && n<=3 {print}' /tmp/device_clean.txt > /tmp/device_set.txt
diff /tmp/host_all.txt /tmp/device_set.txt
```

An empty diff means the 288 logical pixels, their raw colors, and their gamma-corrected colors are identical. Recorded result: 870 lines, empty diff.

## Findings

Two host/device divergences surfaced here and were fixed in the core rather than in this spike:

- `sample_xkcd` sampled with `rng.usize`, which `fastrand` implements with 32 bits on a 32-bit target and 64 bits on a 64-bit target. The same seed would have produced different colors on the C3 and the host/Pi. It now draws a `u64` explicitly.
- `ColorPicker::new(None, ...)` calls `fastrand::Rng::new`, which only exists with `std`. The seedless arm is now behind the crate's `std` feature, and the firmware seeds the picker with `ColorPicker::from_rng`.

If the Pi host is selected, delete this directory. If the ESP32-C3 is selected, task 4.9 replaces the `#[path]` includes with a real `wordclock-core` crate.
