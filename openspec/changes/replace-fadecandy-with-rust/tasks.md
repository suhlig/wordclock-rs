# Tasks

## 1. Repository and baseline

- [x] 1.1 Create the public `github.com/suhlig/wordclock-rs` repository with MIT OR Apache-2.0 licensing, README, `.gitignore`, and a single Rust crate (lib + bin).
- [x] 1.2 Copy this OpenSpec change into the new repo and commit it before writing code.
- [x] 1.3 Add `rust-toolchain.toml`, `Cross.toml`, and a `Cargo.toml` with default features that build without hardware.
- [x] 1.4 Add the `hardware` Cargo feature gating `rppal` and `ws2812-spi`; verify the default build and tests pass on macOS.
- [x] 1.5 Add a `HANDOFF.md` at the repo root that points to `proposal.md`, `design.md`, `tasks.md`, and the legacy repository.

## 2. Golden oracle and core port

- [x] 2.1 Generate `fixtures/minutes.json` in this repo from the legacy Ruby library with the documented command and verify it has 1,440 entries, 503,999 bytes, and SHA-256 `33824892ca69303599c72cf6c7651a0f1d0bb9111ae312ef51dc943278df454e`.
- [x] 2.2 Verify the legacy stripe checksum `aed1c38776d7be5a34e40eb376dce927d4a80efb6f7781a874a8eaa7adfe65b8` and the 922-entry normalized XKCD checksum `9ac5913dccc2200cd46e2b5c4f61d05a5779d8f934b99788c652b34b3ee75d0e` before porting.
- [x] 2.3 Port the 288-character stripe, `hour_words`, `minute_words`, sequential lookup, and `pixels`/`words` behavior exactly, keeping the clock module free of host and transport dependencies.
- [x] 2.4 Add a test that replays all 1,440 fixture entries and compares both the pixel list and the word string.
- [x] 2.5 Add property tests for non-empty output, strictly increasing indices, indices below 288, the known lit-count range 11–39, and at least one dark cell between lit runs on the same row. Cover the five known `HALB`/`SIEBEN` row-crossing cases (06:20, 06:25, 06:30, 06:35, 06:40) explicitly.
- [x] 2.6 Generate the 922-entry Rust XKCD table from the legacy Ruby file and add a table-integrity test.
- [x] 2.7 Port `ColorPicker`, all occasion arbiters, and the grey 64,64,64 cases; test regular, 11 November, Rosenmontag, Faschingsdienstag, Ash Wednesday, birthdays, New Year party, and New Year hangover.
- [x] 2.8 Implement the gamma 2.5 and whitepoint correction LUT and test the vectors 0→0, 32→1, 64→8, 96→22, 128→46, 160→80, 192→125, 224→184, 255→255.
- [x] 2.9 Add a test that the same frame written twice in one minute is byte-identical and does not resample colors.
- [x] 2.10 Do not change any wording or edge-case phrase in this change; record any desired cleanup in a separate OpenSpec proposal.

## 3. Simulator and demo

- [x] 3.1 Define `Frame`, `Rgb`, and the `Display` trait; implement a null backend for tests and a terminal backend for the simulator.
- [x] 3.2 Implement `wordclock sim [TIME]` with the legacy `DD.MM.YYYY HH:MM` format, an ISO-like format, and a default of now.
- [x] 3.3 Implement `--color RRGGBB` and `--seed N` so screenshots and visual comparisons are reproducible.
- [x] 3.4 Implement `wordclock demo` walking all 1,440 minutes at a configurable interval, with progress on stderr.
- [x] 3.5 Render 16 rows of 18 colored characters with dim unlit cells, matching the legacy terminal layout.
- [x] 3.6 Add a test that the simulator and the hardware frame pipeline produce the same indices and corrected colors for the same time and seed.

## 4. Host decision gate and ESP32-C3 spike

Run this gate after tasks 2 and 3 and before any hardware provisioning. The core and simulator must already be host-independent. The output is a recorded choice, not two maintained production paths.

- [x] 4.1 Confirm that `clock`, `color`, and `frame` contain no `std`-only, Pi, ESP32-C3, SPI, RMT, or operating-system dependency; document the alloc boundary needed for a `no_std` build.
- [x] 4.2 Install and verify the ESP32-C3 Rust toolchain: `espflash`, the `riscv32imc-unknown-none-elf` target, and a minimal `esp-hal` blink read over USB Serial/JTAG.
- [ ] 4.3 Build an RMT WS2812 smoke test on the C3; use the on-board addressable LED if present, then an offcut strip through the level shifter once it arrives; verify GRB order and 288-pixel timing with the Pico analyzer.
- [x] 4.4 Implement Wi-Fi association and SNTP synchronization on the C3; verify the boot gate, reconnect after network loss, and periodic resynchronization.
- [x] 4.5 Implement the `Europe/Berlin` local-time and DST rule for the C3; compare it against `chrono-tz` on the host across the fixture and future transition dates through 2030.
- [x] 4.6 Render the 06:30 fixture frame on the C3 and compare its indices and corrected colors with the host simulator.
- [ ] 4.7 Measure boot-to-first-frame time, frame time, RMT inter-chunk gaps, flash and RAM use with the timezone rule and OTA partitions, and Wi-Fi recovery time; record the numbers in `design.md` D16. Measured on 2026-10-25 except the worst-case inter-chunk gap, which needs the 4.3 analyzer capture.
- [ ] 4.8 Decide the host against the D16 criteria and record the choice and evidence in `design.md`; if the spike is inconclusive, keep the Pi.
- [ ] 4.9 If the ESP32-C3 is selected, split the workspace into `wordclock-core` (`no_std` + `alloc`), `wordclock-host` (simulator, CLI, Pi daemon), and `wordclock-esp32` (firmware); if the Pi is selected, keep the single crate and the `hardware` feature.

## 5. Hardware spike and wiring verification (selected host)

Execute only the path selected in task 4.8. Both paths require the 74AHCT125, 470 Ω series resistor, decoupling and bulk capacitors, and a common ground.

### 5A. Raspberry Pi path

- [ ] 5A.1 Acquire the SN74AHCT125N (DIL-14), a 14-pin IC socket, a 470 Ω resistor, a 47 nF ceramic, a 1000 µF 16 V electrolytic, a 5×7 cm perfboard, Dupont leads and a 1×40 header, a Raspberry Pi Pico H for the sigrok-pico analyzer, and a new 32 GB or larger microSD card, in the single BerryBase order listed in `design.md`.
- [ ] 5A.2 Flash a new SD card with Raspberry Pi OS Lite 64-bit (trixie); set hostname `wordclock`, user `suhlig`, SSH key, timezone Europe/Berlin, and German keyboard.
- [ ] 5A.3 Boot the new SD, enable SPI with `dtparam=spi=on`, reboot, and verify `/dev/spidev0.0` and the `spi`, `gpio`, and `i2c` groups.
- [ ] 5A.4 Wire the level shifter exactly as in `design.md`: GPIO10 to 1A, 1Y through 470 Ω to strip DIN, 1OE to GND, VCC to LED 5 V, common ground, decoupling and bulk capacitors.
- [ ] 5A.5 Build and run a minimal SPI blink program on the real strip before writing the full daemon; confirm pure red, green, blue, white, and a full-strip wipe.
- [ ] 5A.6 Implement `wordclock test-pattern` with walking-pixel, row-color, primary-color, and all-on modes; use it to identify the physical order, orientation, and whether the extra 12 LEDs are connected.
- [ ] 5A.7 Record the verified mapping in the configuration and hardware guide; keep identity as the default only if the test pattern proves it.
- [ ] 5A.8 Measure frame time at 2.4 MHz and 3 MHz and record the result; capture GPIO10 with the sigrok-pico analyzer to record the measured frame time and the 0.4/0.8 µs pulse widths, and keep the fastest reliable clock.
- [ ] 5A.9 If SPI is unreliable, try a lower clock or another SPI mode, then document the fallback to an RP2040/ESP32 controller before considering Pi PWM/DMA.

### 5B. ESP32-C3 path

- [ ] 5B.1 Audit the BOM for the C3 path: ESP32-C3-DevKit-RUST-1, 74AHCT125, IC socket, 470 Ω, 47 nF, 1000 µF, perfboard, Dupont leads, 5 V supply, and the Pico H analyzer.
- [ ] 5B.2 Choose a free, non-strapping header GPIO from the board schematic; wire it to 1A, 1Y through 470 Ω to strip DIN, 1OE to GND, VCC to LED 5 V, common ground, decoupling and bulk capacitors.
- [ ] 5B.3 Flash the RMT test firmware; confirm pure red, green, blue, white, and a full-strip wipe on an offcut strip.
- [ ] 5B.4 Run walking-pixel, row-color, primary-color, and all-on patterns; identify the physical order, orientation, and whether the extra 12 LEDs are connected.
- [ ] 5B.5 Record the verified mapping in the configuration and hardware guide.
- [ ] 5B.6 Measure the RMT frame time and inter-chunk gaps with the Pico analyzer; confirm 288 pixels, GRB order, and margin below the WS2812 reset threshold.
- [ ] 5B.7 If RMT is unreliable, try another channel or GPIO, then an SPI fallback; document the result before rejecting the C3 path.

## 6. Daemon or firmware (selected host)

### 6A. Shared daemon logic

- [ ] 6A.1 Implement the minute scheduler and per-minute color cadence behind the clock and display traits; test them on the host with a fake time source.
- [ ] 6A.2 Implement the time-source gate behavior with a fake synchronized/unsynchronized source; test the neutral frame and the first synchronized frame.

### 6B. Raspberry Pi daemon

- [ ] 6B.1 Implement the TOML configuration with logical pixels, physical strip length, SPI device and clock, gamma, whitepoint, timezone, birthday list, refresh interval, and NTP gate.
- [ ] 6B.2 Implement the minute-boundary scheduler, including midnight rollover and daylight-saving transitions.
- [ ] 6B.3 Implement per-minute uniform color resampling and per-minute per-pixel sampling for occasion modes; never resample on the 1 Hz refresh.
- [ ] 6B.4 Implement the NTP gate using `timedatectl show -p NTPSynchronized --value`, with an off or neutral frame and a log every 30 seconds while waiting.
- [ ] 6B.5 Implement the 1 Hz refresh and the retry-on-write-failure behavior.
- [ ] 6B.6 Implement graceful SIGTERM handling that writes the uniform dim shutdown pattern (raw 32,32,32 across all logical pixels, padding black), logs the stop, and exits successfully.
- [ ] 6B.7 Implement single-instance protection so a second daemon cannot write the strip.
- [ ] 6B.8 Add the systemd unit with the user, groups, restart policy, time-sync ordering, `TimeoutStopSec=10`, and hardening flags from `design.md`.
- [ ] 6B.9 Add structured logging for startup, NTP waits, minute changes, color changes, and output errors; confirm journal visibility.
- [ ] 6B.10 Add a test that SIGTERM produces exactly one shutdown frame, that it contains no word from the previous minute, that every logical pixel is 32,32,32, and that padding pixels stay black.

### 6C. ESP32-C3 firmware

- [ ] 6C.1 Choose `esp-hal` or `esp-idf-svc` from the spike results and pin the toolchain and crate versions.
- [ ] 6C.2 Implement NVS/LittleFS configuration for strip length, GPIO, timezone, birthday list, refresh interval, and time-source gate; keep Wi-Fi credentials out of the repository.
- [ ] 6C.3 Implement Wi-Fi provisioning, association, reconnect, the SNTP startup gate, and periodic resynchronization.
- [ ] 6C.4 Implement minute scheduling, midnight and DST handling, per-minute color cadence, and the 1 Hz refresh with write retry.
- [ ] 6C.5 Implement the hardware watchdog and output-task restart.
- [ ] 6C.6 Implement OTA with an A/B partition and rollback, plus a readable firmware version.
- [ ] 6C.7 Implement serial and network logging, and document the power-loss limitation instead of a shutdown pattern.
- [ ] 6C.8 Run the fixture replay and daemon state-machine tests on the host; run the on-device smoke test on the C3.

## 7. Build and release (selected host)

### 7A. Raspberry Pi

- [ ] 7A.1 Add CI for `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` on default features.
- [ ] 7A.2 Add an aarch64 `cross build --release --target aarch64-unknown-linux-gnu --features hardware` job and run the resulting binary on the Pi.
- [ ] 7A.3 Add a tag-triggered release workflow that packages the binary, systemd unit, example config, licenses, and SHA-256.
- [ ] 7A.4 Add Dependabot for Cargo and GitHub Actions and pin the Rust toolchain.
- [ ] 7A.5 Verify the release binary starts on the Pi and reports its version without the Ruby, Fadecandy, or OPC stack.

### 7B. ESP32-C3

- [ ] 7B.1 Add CI for `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` on default features.
- [ ] 7B.2 Build the `riscv32imc-unknown-none-elf` firmware with `espflash` and publish the image with its checksum.
- [ ] 7B.3 Publish the partition/OTA metadata and the flashing instructions with the release.
- [ ] 7B.4 Verify the release image boots on the C3, joins Wi-Fi, synchronizes, reports its version, and renders the fixture frame.
- [ ] 7B.5 Pin the ESP toolchain and crate versions and add Dependabot coverage.

## 8. Deployment and legacy removal (selected host)

### 8A. Raspberry Pi

- [ ] 8A.1 Add a `wordclock` Ansible role that installs the pinned release, verifies the checksum, creates the user and groups, installs the config and unit, and enables SPI.
- [ ] 8A.2 Remove the `fadecandy` role, the `nerab.ruby` role, the `ansistrano` deployment, and the wordclock cron entries from `uhlig-it/infrastructure`.
- [ ] 8A.3 Provision a freshly flashed 64-bit SD card with one Ansible run and confirm the service is active and the SPI device exists.
- [ ] 8A.4 Confirm `fcserver` is absent or disabled, no Ruby cron remains, and the host no longer needs Ruby.
- [ ] 8A.5 Add a rollback section to the infrastructure docs that describes the card swap and Fadecandy reconnection.

### 8B. ESP32-C3

- [ ] 8B.1 Document Wi-Fi credential provisioning and secrets handling; nothing secret goes into git.
- [ ] 8B.2 Document the flash and OTA procedure, including checksum verification and the rollback image.
- [ ] 8B.3 Provision a blank C3 with one documented procedure and confirm it renders without a debugger attached.
- [ ] 8B.4 Keep the legacy Pi and Fadecandy clock as the rollback path, and retire it only after the stability window.
- [ ] 8B.5 Document the OTA rollback and USB reflash procedures.

## 9. Cutover and validation

- [ ] 9.1 Capture fixed-color reference photos of the legacy clock at 00:00, 00:06, 01:05, 11:15, 12:15, 13:15, and 23:55.
- [ ] 9.2 Render the same times and colors with the selected host and compare the words, pixel positions, and gamma-corrected colors side by side.
- [ ] 9.3 Run the selected host for 24 hours and confirm every minute updates, no minute is skipped, and the color changes once per minute.
- [ ] 9.4 Reboot the selected host and confirm its boot behavior: shutdown pattern on an orderly Pi stop, neutral/NTP gate on the C3, then service start and correct first frame.
- [ ] 9.5 Disconnect the network after startup and confirm the clock keeps rendering from the local clock.
- [ ] 9.6 Stop and start the service on the Pi, or power-cycle the C3; confirm the shutdown pattern on the Pi or the neutral gate on the C3, then recovery to the current time.
- [ ] 9.7 Run the selected host's rollback drill: old SD card and Fadecandy for the Pi, or previous OTA image/USB reflash plus the legacy Pi clock for the C3.
- [ ] 9.8 After at least two weeks of stable operation, remove Fadecandy from the signal path and retire the board; keep the old SD card or the previous firmware as the last-resort image.

## 10. Documentation and follow-up

- [ ] 10.1 Write the new repo README with build, simulator, configuration, deployment, and troubleshooting instructions.
- [x] 10.2 Write the hardware guide with the wiring diagram, BOM, output enablement, test-pattern procedure, and power/ground notes.
- [ ] 10.3 Link the new repo to the legacy repo and the Fadecandy handoff; state that the legacy repo is the behavioral oracle.
- [ ] 10.4 Record the verified physical mapping and pixel counts in the hardware guide.
- [ ] 10.5 After the host decision, add the selected host's wiring, RMT or SPI setup, and OTA or Ansible procedure to the hardware guide; mark the other path as non-normative or remove it.
- [ ] 10.6 Create separate OpenSpec proposals for the deferred Reminders ideas, starting with the highest-value ones: smooth fade, night dimming, web/Prometheus/MQTT observability, and sensors.
