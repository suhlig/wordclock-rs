# ESP32-C3 host spike

This is the working log for the host decision in design D16. The candidate is an ESP32-C3-DevKit-RUST-1 (single-core RISC-V, 400 KiB SRAM, typically 4 MiB flash, 2.4 GHz Wi-Fi, USB Serial/JTAG). The Pi 3B remains the incumbent and the fallback.

## 4.1 Core portability audit

The clock, color, and frame code must move into a `no_std` + `alloc` core crate if the ESP32-C3 wins. The audit after replacing `f64::powf` with `libm`:

| Module | Alloc | Std / OS dependency | Notes |
| :--- | :--- | :--- | :--- |
| `clock` | Yes (`Vec`, `String`) | None | Pure, fixture-tested |
| `color` | Yes (`Vec`, static table) | None after the `libm` change | `chrono::NaiveDate`/`Datelike` need the `alloc` feature only; `fastrand::Rng::with_seed` and `computus` are `no_std`; `libm` provides `pow` and `round` |
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
- The sequence is visible in the USB Serial/JTAG log. Visual color-order confirmation is pending.
- The 288-pixel offcut strip test still waits for the 74AHCT125 and passives on Wednesday. That test will use a free header GPIO, the level shifter, a 470 Ω series resistor, and the Pico analyzer for pulse widths and inter-chunk gaps.

## 4.4 Wi-Fi and SNTP

- 2.4 GHz association, SNTP synchronization, and the neutral frame until the first sync.
- Reconnect after an access-point restart and resynchronize periodically.
- Keep rendering from the local clock after the initial sync if the network drops.

## 4.5 Timezone

- Implemented in `src/timezone.rs`: `berlin_local_from_utc` and `dst_bounds` use the last Sunday in March and October at 01:00 UTC.
- Verified against `chrono_tz::Europe::Berlin` for every hour from 2026-01-01 through 2030-12-31, plus explicit spring and autumn transition tests.
- The firmware can reuse the rule directly; it does not need the full timezone database.

## 4.6 Fixture frame

Render 06:30 on the C3 and compare its lit indices and corrected colors with the host simulator, including the split `HALB`/`SIEBEN` run.

## 4.7 Measurements

| Measurement | Result |
| :--- | :--- |
| Boot to first frame | Pending |
| Full 288-pixel frame time | Pending |
| RMT inter-chunk gap | Pending |
| Flash use with timezone + OTA partitions | Pending |
| RAM use | Pending |
| Wi-Fi reconnect time | Pending |

## 4.8 Decision

Pending the measurements above. If the spike is inconclusive, the Pi 3B wins by default.
