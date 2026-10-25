# ESP32-C3 host spike

This is the working log for the host decision in design D16. The candidate is an ESP32-C3-DevKit-RUST-1 (single-core RISC-V, 400 KiB SRAM, typically 4 MiB flash, 2.4 GHz Wi-Fi, USB Serial/JTAG). The Pi 3B remains the incumbent and the fallback.

## 4.1 Core portability audit

The clock, color, and frame code must move into a `no_std` + `alloc` core crate if the ESP32-C3 wins. The audit after replacing `f64::powf` with `libm`:

| Module | Alloc | Std / OS dependency | Notes |
| :--- | :--- | :--- | :--- |
| `clock` | Yes (`Vec`, `String`) | None | Pure, fixture-tested |
| `color` | Yes (`Vec`, static table) | None after the `libm` change | `chrono::NaiveDate`/`Datelike` need the `alloc` feature only; `fastrand::Rng::with_seed` and `computus` are `no_std`; `libm` provides `pow` and `round`; the seedless `ColorPicker::new(None, ...)` arm is behind the `std` feature because `Rng::new` does not exist without it |
| `frame` | Yes (`Vec`) | None | Pure |
| `display` | Yes | `std::io` in the terminal backend | Host-only; the firmware supplies an RMT `Display` |
| `sim` | Yes | `chrono::Local`, `std::io`, `std::thread` | Host-only |
| `config`, `daemon` | Later | Host-specific | Section 6 of the plan |

Boundary decisions for the split in task 4.9:

- `wordclock-core` uses `#![no_std]`, `extern crate alloc`, and imports `Vec`/`String` from `alloc`.
- `chrono` in the core uses `default-features = false, features = ["alloc"]`; the host crate enables `clock` and `std` for `Local::now`.
- `fastrand` in the core uses `default-features = false, features = ["alloc"]`; the firmware seeds `Rng::with_seed(hardware_random)` and constructs the picker with `ColorPicker::from_rng`.
- `libm` replaces `f64::powf` and `f64::round` in the gamma LUT.
- `display`, `sim`, `main`, `config`, and `daemon` stay in the host or firmware crate.

The seedless `ColorPicker::new(None, ...)` path uses `fastrand::Rng::new`, which needs std; it is fine for the host and is not used by the firmware.

## 4.2 Toolchain status

- Host: macOS, pinned Rust 1.99.1 for the main crate; the spike project uses stable (1.98.0) because `esp-hal` requires 1.95 or newer.
- Installed: `espflash` 4.6.0, `cargo-espflash`, `cargo-generate` 0.19.0, and `esp-generate` 1.4.0. The RISC-V target `riscv32imc-unknown-none-elf` is installed.
- Board detected on `/dev/cu.usbmodem1101`: ESP32-C3 revision v0.4, 4 MB flash, Wi-Fi/BLE, MAC `34:85:18:02:70:14`.
- Generated `spike/esp32c3-blink` with `esp-generate` (`esp32c3`, `log`, `esp-backtrace`, `alloc`). It builds, flashes with `espflash`, and streams `Hello world!` over USB Serial/JTAG.
- Crates: `esp-hal` 1.2.2, `esp-hal-smartled` 0.18.0, `smart-leds` 0.4.0.

## 4.3 RMT strip test

- The board's own README maps the on-board WS2812 to GPIO2 (not GPIO8, which is the generic DevKitM-1 pin).
- `spike/esp32c3-blink` now drives that LED over RMT with `esp-hal-smartled` 0.18.0, `WS2812_TIMING`, and GRB order. It cycles red, green, blue, white, off every 700 ms.
- Visual confirmation: the on-board LED cycles red, green, blue, white, off in that order, so the GRB byte order and RMT timing are correct for this LED.
- The 288-pixel offcut strip test still waits for the 74AHCT125 and passives on Wednesday. That test will use a free header GPIO, the level shifter, a 470 Ω series resistor, and the Pico analyzer for pulse widths and inter-chunk gaps.

## 4.4 Wi-Fi and SNTP

- Implemented in `spike/esp32c3-wifi` with `esp-radio` 1.0.0-beta.1, `esp-rtos` 0.4.0, `embassy-net` 0.9.1, and `sntpc` 0.11.0. It builds cleanly for `riscv32imc-unknown-none-elf`.
- Credentials are read from a gitignored `wifi.env` by `build.rs`; `wifi.env.example` is the tracked template. The password is compiled into this spike only.
- The firmware associates in station mode, gets a DHCP lease, resolves `pool.ntp.org`, and requests SNTP over UDP every 60 seconds.
- The on-board WS2812 is off until the first SNTP response and green afterwards. It stays green if the network drops, while the Wi-Fi task reconnects.
- Status: verified on the board on `/dev/cu.usbmodem1101` against the local 2.4 GHz access point.
- Observed: a DHCP lease on the local subnet, an SNTP response (`unix=1792924384`), a 60 s resynchronization, and the boot gate (LED off until the first SNTP response, then green and staying green).
- Reconnect verified with `--features reconnect-test`: a forced disconnect drives `IPv4: DOWN`, the controller reassociates on the same BSSID and channel 1, the lease returns on the same IP, and `synced` stays true.
- Reconnect takes roughly 5 s, dominated by the deliberate backoff in the Wi-Fi task.
- The release image with Wi-Fi, SNTP, and RMT is 520,544 of 4,128,768 bytes (12.6%) on the default single-app partition layout, before the timezone rule and OTA partitions are added.

## 4.5 Timezone

- Implemented in `src/timezone.rs`: `berlin_local_from_utc` and `dst_bounds` use the last Sunday in March and October at 01:00 UTC.
- Verified against `chrono_tz::Europe::Berlin` for every hour from 2026-01-01 through 2030-12-31, plus explicit spring and autumn transition tests.
- The firmware can reuse the rule directly; it does not need the full timezone database.

## 4.6 Fixture frame

- `spike/esp32c3-frame` includes the real `src/clock.rs`, `src/color.rs`, and `src/frame.rs` through `#[path]`, so it runs the host code rather than a copy. It also proves that the core compiles for `riscv32imc-unknown-none-elf` with `no_std` + `alloc`.
- Three scenarios at 06:30: fixed color `c04020` on 2026-07-15, seed 42 on the same regular day, and seed 42 on Rosenmontag 2026-02-16 for per-pixel sampling.
- The device dump and `examples/dump_frame.rs` are byte-identical after stripping the serial CRLF: 870 lines, empty `diff`. That covers the 288 logical pixels, the raw colors, and the gamma-corrected colors, including the split `HALB`/`SIEBEN` run.
- Image size: 102,800 of 4,128,768 bytes (2.49%) without Wi-Fi, RMT, or the timezone rule.
- Two portability bugs were found and fixed in the core: `sample_xkcd` used `rng.usize`, which is 32-bit on the C3 and 64-bit on the host, so the same seed gave different colors; it now draws a `u64`. And `ColorPicker::new(None, ...)` needs `fastrand::Rng::new`, which is `std`-only, so the seedless arm is behind the new default `std` feature and the firmware uses `ColorPicker::from_rng`.

## 4.7 Measurements

| Measurement | Result |
| :--- | :--- |
| Boot to first frame | Pending |
| Boot to first SNTP response | Not instrumented yet; early boot lines are lost because the monitor attaches after flash. Add an explicit boot timestamp before recording. |
| Full 288-pixel frame time | Pending |
| RMT inter-chunk gap | Pending |
| Flash use with timezone + OTA partitions | Pending |
| RAM use | Pending |
| Wi-Fi reconnect time | About 5 s after a forced disconnect, including the backoff |

## 4.8 Decision

Pending the measurements above. If the spike is inconclusive, the Pi 3B wins by default.
