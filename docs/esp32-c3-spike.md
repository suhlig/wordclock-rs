# ESP32-C3 host spike

This is the working log for the host decision in design D16. The candidate is an ESP32-C3-DevKit-RUST-1 (single-core RISC-V, 400 KiB SRAM, typically 4 MiB flash, 2.4 GHz Wi-Fi, USB Serial/JTAG). The Pi 3B remains the incumbent and the fallback.

## 4.1 Core portability audit

The clock, color, and frame code must move into a `no_std` + `alloc` core crate if the ESP32-C3 wins. The audit after replacing `f64::powf` with `libm`:

| Module | Alloc | Std / OS dependency | Notes |
| :--- | :--- | :--- | :--- |
| `clock` | Yes (`Vec`, `String`) | None | Pure, fixture-tested |
| `color` | Yes (`Vec`, static table) | None after the `libm` change | `chrono::NaiveDate`/`Datelike` need the `alloc` feature only; `fastrand::Rng::with_seed` and `computus` are `no_std`; `libm` provides `pow` and `round`; the seedless `ColorPicker::new(None, ...)` arm is behind the `std` feature because `Rng::new` does not exist without it |
| `frame` | Yes (`Vec`) | None | Pure |
| `display` | Yes | `std::io` in the terminal backend | Host-only; the firmware supplies an SPI `Display` |
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
- `spike/esp32c3-blink` first drove that LED over RMT with `esp-hal-smartled` 0.18.0, `WS2812_TIMING`, and GRB order, cycling red, green, blue, white, off every 700 ms.
- Visual confirmation at that stage: the on-board LED cycled red, green, blue, white, off in that order, so the GRB byte order and RMT timing were correct for that LED.

Offcut through the level shifter (2026-10-31):

- A 35-pixel WS2812 offcut, after removing the dead pixels from the 41-pixel piece, runs from GPIO3 through the SN74AHCT125N at 5 V, with a 470 Ω series resistor and a common ground between the C3, the shifter and the strip.
- `spike/esp32c3-blink` now runs two patterns in a loop: all 35 pixels in red, green, blue, white and off at 700 ms each, then a single white pixel walking 0 to 34 at 120 ms per step. All 35 pixels light and hold colour, the primaries are correct, and the walking pixel runs end to end in order. That proves GRB order, index order and 5 V drive; it does not prove the edge quality or the voltage at `1Y`, which were not measured.
- GPIO3 replaces GPIO2 as the strip data pin. On this board only GPIO0, GPIO1 and GPIO3 have no strapping role, no JTAG duty, no internal-flash line and no onboard peripheral. GPIO4-7 carry JTAG plus the flash data lines, GPIO8 and GPIO10 are the I2C bus to the IMU and SHTC3, GPIO9 is the BOOT button, and GPIO2 is a strapping pin shared with the on-board WS2812. The on-board LED is therefore no longer driven by this firmware.
- `memsize` stays at 2, which is the maximum for channel 0. A 35-pixel frame is 842 RMT codes against 96 codes of channel RAM, so esp-hal refills the RAM in half-blocks while transmitting; no gap was measured and none is expected at this length.
- The 5 V side was verified by eye only. `1Y` has not been on the analyzer and the levels were not taken with a meter, so the edge-quality half of D16 still rests on the 4.7 capture, which was made at GPIO2 on the 3.3 V side.

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

Measured on 2026-10-25 with the combined gate firmware (`spike/esp32c3-wifi`): Wi-Fi, DHCP, SNTP, the real core and `Europe/Berlin` rule, and a 288-pixel RMT frame, with an OTA partition table for the size measurement. The frame timing was captured with `frame-loop`, which skips Wi-Fi, so the RMT rows below are the no-Wi-Fi control.

| Measurement | Result |
| :--- | :--- |
| Boot to first frame | 245.9 ms from chip reset to the completed neutral write (236.8 ms bootloader plus HAL init, 8.1 ms frame write) |
| Boot to first SNTP response | 2.30 s when the AP has no stale session for this MAC; 11.29 s and 11.33 s after an abrupt reset while still associated |
| Boot to first clock frame | 2.39 s clean, 11.42 s with a stale session |
| Full 288-pixel RMT frame time (no-Wi-Fi control) | 8.10–8.18 ms for the clock frame; 8.15 ms all-zero; 10.45 ms all-white |
| Full 288-pixel SPI frame time (four-bit cell, 2.5 MHz) | 11.5 ms wire time, derived from 12 bytes/pixel |
| Full 335-pixel SPI frame time (four-bit cell, 2.5 MHz) | 13.3 ms wire time, derived |
| RMT inter-chunk gap (no-Wi-Fi control) | None measurable: the longest low inside a frame is 0.708 µs (the normal zero bit) in two captures covering 9,697 and 10,335 bits at 24 MHz; far below the 50 µs WS2812 reset |
| Flash use with timezone + OTA partitions | 556,496 of 1,966,080 bytes (28.30%) in one OTA slot; 1,409,584 bytes free |
| Flash use with the default single-app layout | 556,496 of 4,128,768 bytes (13.48%) |
| RAM use | Heap 103,184 bytes: 1,728 used after boot, 50,308 after Wi-Fi, 54,432 peak (52.8%); plus a 27,656-byte static RMT buffer, which the SPI build replaces with about 4 KiB |
| Wi-Fi reconnect time | 5.10 s and 5.07 s after a forced disconnect, of which 5 s is the deliberate backoff; association itself is about 0.1 s |

The consistent 2.30 s versus 11.3 s difference is the DHCP lease reacquisition after an abrupt reset: the access point still holds the previous association for the same MAC. A power loss looks like that case, so plan for up to about 12 s of neutral frame after boot rather than the clean 2.4 s.

The software `write()` time includes the 80 µs reset, buffer conversion, and channel hand-over, so it overstates the line time. The analyzer capture is authoritative: one frame carries 7.311 ms of bitstream and no low interval above 0.708 µs. The setup was the Pico H with `sigrok-pico` and the `raspberrypi-pico` driver, clipped directly to GPIO2 (3.3 V, so no level shifter was needed), one channel at 24 MHz, 380,000 samples. The `frame-loop` feature streams the 06:30 frame back-to-back for the capture, and `rgb_led_ws281x` decoded all 288 pixels identically to `examples/dump_frame.rs` for the same time and seed. That capture is the no-Wi-Fi control: `frame-loop` skips Wi-Fi, and the same RMT transport failed at 335 pixels with Wi-Fi active (4.8 and D19).

## 4.8 OTA update path

The single 5 V decision (design D9 of `add-power-budget-and-current-sensing`) removes USB from the finished build, so firmware updates need a path that does not touch VBUS. `spike/esp32c3-ota` implements HTTP on port 3232 and was verified on 2026-10-31.

- Layout: two OTA slots of 1,966,080 bytes plus `otadata`; the spike image is 522,848 bytes, 26.6 % of one slot.
- `PUT /ota` with `curl -T` streams the body into the inactive slot, verifies the `0xE9` magic byte, switches the boot pointer, marks the image `New`, and resets.
- Four consecutive uploads alternated ota_0 → ota_1 → ota_0 and delivered a changed revision. The bootloader line `Loaded app from partition at offset 0x200000` and the status field `running=Ota1` are the proof, both from the device rather than from the client.
- Upload time: 10.8 s for 522,848 bytes while USB-powered, about 48 KB/s. The floor is the flash path (128 pages of 4 KiB, each erasing a sector with interrupts masked), not Wi-Fi.
- Delivered with **USB unplugged and the board powered from the `5V` pin** on 2026-10-31: revision 2 → 3 in 35.6 s (`running=Ota1`), then revision 3 again in 39.4 s (`running=Ota0`). After adding **1000 µF + 100 nF** at the board, revision 3 → 4 took 15.5 s, so the bulk capacitance appears to buy back most of the difference. Single samples, so this is a strong hint rather than a result: the rail without it was consistently three times slower, which points at supply noise on the radio rather than at retransmission timing.
- Gotcha: reflashing a board that previously used a different partition layout leaves stale bytes at `0x10000`, which is the second `otadata` slot in this layout. The library rejects the entry, the whole partition becomes unusable (`Error::Invalid`), and the first update cannot start. The spike repairs this at boot by scrubbing `otadata` and selecting the slot the MMU says is running; `espflash erase-flash` does the same from outside.
- `GET /` reports the revision, uptime, heap, the running slot (from the MMU mapping), the boot selection, and the slot state, and the on-board WS2812 blinks the revision at boot and then shows ready, progress and error colours.
- The offcut ran on the same rail as the controller: revision 4, built with `--features strip-check`, drove five pixels on GPIO3 through the SN74AHCT125N with USB unplugged, which closed the question of whether one 5 V rail can carry both.
- Driving only five of the strip's 35 pixels made the untransmitted thirty come up solid red on roughly every third power-up. A WS2812 holds its power-on latch until it receives data, so a short frame leaves the tail at whatever the rail ramp left there. Four rules came out of chasing it, all now in `docs/hardware.md`:
  - write the first full frame as soon as the display driver exists, not after status blinking — the 2.4 s delay was the thing on display;
  - cover every pixel in every frame;
  - send the first frame two or three times, because the far end of a chain can still be finishing its own power-on reset (three of ten power-ups left the last pixel dark with a single early frame);
  - keep the 10 kΩ pull-down on the shifter's input, on principle: with it the flash shrank from the whole strip to the first 19–20 pixels, and after the timing fix ten of ten power-ups were clean. The combination of an early first frame *without* it was never measured.
- The 335-pixel chain exposed a platform limit that belongs in D16's column: the C3 has 48 RMT codes per channel, so a full frame of 8,042 codes is sent by refilling that block from a busy-poll loop every 30 µs. If an interrupt holds the CPU past that deadline the hardware wraps the block and transmits it again, shifting the rest of the frame by two pixels per wrap. Measured on a walk: one step's lit pixel appeared about 36 pixels further down the chain while the rest of the frame was correct — 18 wraps, roughly 1.1 ms of interrupt time. Writing each frame twice turned that into a brief flicker rather than a cure.
- The cure is to leave the CPU out of the timing path: the strip runs on **SPI2 with DMA at 2.5 MHz, four SPI bits per LED bit** (`0` → `1000`, `1` → `1100`), with GPIO3 as MOSI and DMA streaming a 4 KiB bitstream. A full 335-pixel walk runs with **no glitches at all**, where RMT produced a few per sweep. This retires the earlier clean RMT result: that capture ran with `frame-loop`, i.e. without Wi-Fi. The firmware for the clock must use SPI with DMA, not RMT, and the Pi's SPI path lands in the same place.
- The first SPI build used **three** SPI bits per LED bit (`0` → `100`, `1` → `110`) at 2.5 MHz, which ties the gap after a `1` to exactly one clock, 400 ns. That gap is the only place the strip misbehaved: a pixel whose last LED bit was a `1` was followed by a stray pixel carrying `G = 0x80`. Revision 20 moved to the four-bit cell, which leaves 800 ns there while keeping both data pulses at their nominal widths. The timing is now data in `src/strip.rs` and is asserted by that module's tests instead of being argued about in a comment. See `docs/strip-colour-fault-handover.md`.
- Not verified: update behaviour during a Wi-Fi outage; and whether the 3× slower upload on the un-decoupled 5 V rail is supply noise, given that one sample with 1000 µF came back near the USB figure.

## 4.9 Decision

Pending the measurements above. If the spike is inconclusive, the Pi 3B wins by default.

## 4.10 Outage check (D20)

Ran on 2026-11-03 on the bench with `spike/esp32c3-ota` built `--features outage-check`: revision 25, the 35-pixel offcut, `strip_pixels=35`. The build drops its own Wi-Fi link for 300 s after every 120 s up, so D20 can be observed without taking the household access point down. It was delivered by OTA from revision 24 in 12.0 s (553,280 bytes) and came up as `running=Ota1`.

The first cycle, with the board booted at ~12:16:46:

| Event | Computed | Observed |
| :--- | :--- | :--- |
| Link drops | uptime 120 s, ~12:18:47 | status page unreachable from 12:18:59 |
| Amber, age > 90 s | ~12:19:39 | yes |
| Dark, age > 180 s | ~12:21:09 | yes |
| Reconnect and resync | ~12:23:47–12:24:10 | yes, `sync_age_s=5` |

Recovered status at 12:24:10, uptime 443 s:

```
revision=25 strip_check=on strip_pixels=35
synced=true sync_age_s=5 last_sync_unix=1793705045
clock_unix=1793705050 max_sync_age_s=356 outages=1
```

- The bar advanced while the link was down, so the display time base is the crystal and not the network.
- `max_sync_age_s=356` is the gap from the last pre-drop reply to the first post-recovery reply. It exceeds the 300 s down window by up to one 60 s resync interval, which is expected.
- `clock_unix=1793705050` at 12:24:10 against host `unix=1793705072` at 12:24:32 is the same instant across the 22 s elapsed.
- `outages=1` for one observed cycle.

All four rows are now confirmed by eye: green → amber → dark → green, with the bar advancing throughout. The counters agree, so this run validates the D20 approach; the production firmware still has to implement task 4.11.
