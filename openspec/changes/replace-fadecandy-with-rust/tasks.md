# Tasks

## 1. Repository and baseline

- [ ] 1.1 Create the public `github.com/suhlig/wordclock-rs` repository with MIT OR Apache-2.0 licensing, README, `.gitignore`, and a Rust workspace.
- [ ] 1.2 Copy this OpenSpec change into the new repo and commit it before writing code.
- [ ] 1.3 Add `rust-toolchain.toml`, `Cross.toml`, and a `Cargo.toml` with default features that build without hardware.
- [ ] 1.4 Add the `hardware` Cargo feature gating `rppal` and `ws2812-spi`; verify the default build and tests pass on macOS.
- [ ] 1.5 Add a `HANDOFF.md` at the repo root that points to `proposal.md`, `design.md`, `tasks.md`, and the legacy repository.

## 2. Golden oracle and core port

- [ ] 2.1 In the legacy repo, generate `fixtures/minutes.json` with the documented Ruby command and verify it has 1,440 entries, 503,999 bytes, and SHA-256 `33824892ca69303599c72cf6c7651a0f1d0bb9111ae312ef51dc943278df454e`.
- [ ] 2.2 Verify the legacy stripe checksum `aed1c38776d7be5a34e40eb376dce927d4a80efb6f7781a874a8eaa7adfe65b8` and the 922-entry normalized XKCD checksum `9ac5913dccc2200cd46e2b5c4f61d05a5779d8f934b99788c652b34b3ee75d0e` before porting.
- [ ] 2.3 Port the 288-character stripe, `hour_words`, `minute_words`, sequential lookup, and `pixels`/`words` behavior exactly.
- [ ] 2.4 Add a test that replays all 1,440 fixture entries and compares both the pixel list and the word string.
- [ ] 2.5 Add property tests for non-empty output, strictly increasing indices, indices below 288, and the known lit-count range 11–39.
- [ ] 2.6 Generate the 922-entry Rust XKCD table from the legacy Ruby file and add a table-integrity test.
- [ ] 2.7 Port `ColorPicker`, all occasion arbiters, and the grey 64,64,64 cases; test regular, 11 November, Rosenmontag, Faschingsdienstag, Ash Wednesday, birthdays, New Year party, and New Year hangover.
- [ ] 2.8 Implement the gamma 2.5 and whitepoint correction LUT and test the vectors 0→0, 32→1, 64→8, 96→22, 128→46, 160→80, 192→125, 224→184, 255→255.
- [ ] 2.9 Add a test that the same frame written twice in one minute is byte-identical and does not resample colors.
- [ ] 2.10 Do not change any wording or edge-case phrase in this change; record any desired cleanup in a separate OpenSpec proposal.

## 3. Simulator and demo

- [ ] 3.1 Define `Frame`, `Rgb`, and the `Display` trait; implement a null backend for tests and a terminal backend for the simulator.
- [ ] 3.2 Implement `wordclock sim [TIME]` with the legacy `DD.MM.YYYY HH:MM` format, an ISO-like format, and a default of now.
- [ ] 3.3 Implement `--color RRGGBB` and `--seed N` so screenshots and visual comparisons are reproducible.
- [ ] 3.4 Implement `wordclock demo` walking all 1,440 minutes at a configurable interval, with progress on stderr.
- [ ] 3.5 Render 16 rows of 18 colored characters with dim unlit cells, matching the legacy terminal layout.
- [ ] 3.6 Add a test that the simulator and the hardware frame pipeline produce the same indices and corrected colors for the same time and seed.

## 4. Hardware spike and wiring verification

- [ ] 4.1 Acquire the SN74AHCT125N (DIL-14), a 14-pin IC socket, a 470 Ω resistor, a 47 nF ceramic, a 1000 µF 16 V electrolytic, a 5×7 cm perfboard, Dupont leads and a 1×40 header, a Raspberry Pi Pico H for the sigrok-pico analyzer, and a new 32 GB or larger microSD card, in the single BerryBase order listed in `design.md`.
- [ ] 4.2 Flash a new SD card with Raspberry Pi OS Lite 64-bit (trixie); set hostname `wordclock`, user `suhlig`, SSH key, timezone Europe/Berlin, and German keyboard.
- [ ] 4.3 Boot the new SD, enable SPI with `dtparam=spi=on`, reboot, and verify `/dev/spidev0.0` and the `spi`, `gpio`, and `i2c` groups.
- [ ] 4.4 Wire the level shifter exactly as in `design.md`: GPIO10 to 1A, 1Y through 470 Ω to strip DIN, 1OE to GND, VCC to LED 5 V, common ground, decoupling and bulk capacitors.
- [ ] 4.5 Build and run a minimal SPI blink program on the real strip before writing the full daemon; confirm pure red, green, blue, white, and a full-strip wipe.
- [ ] 4.6 Implement `wordclock test-pattern` with walking-pixel, row-color, primary-color, and all-on modes; use it to identify the physical order, orientation, and whether the extra 12 LEDs are connected.
- [ ] 4.7 Record the verified mapping in the configuration and hardware guide; keep identity as the default only if the test pattern proves it.
- [ ] 4.8 Measure frame time at 2.4 MHz and 3 MHz and record the result; capture GPIO10 with the sigrok-pico analyzer to record the measured frame time and the 0.4/0.8 µs pulse widths, and keep the fastest reliable clock.
- [ ] 4.9 If SPI is unreliable, try a lower clock or another SPI mode, then document the fallback to an RP2040/ESP32 controller before considering Pi PWM/DMA.

## 5. Daemon

- [ ] 5.1 Implement the TOML configuration with logical pixels, physical strip length, SPI device and clock, gamma, whitepoint, timezone, birthday list, refresh interval, and NTP gate.
- [ ] 5.2 Implement the minute-boundary scheduler, including midnight rollover and daylight-saving transitions.
- [ ] 5.3 Implement per-minute uniform color resampling and per-minute per-pixel sampling for occasion modes; never resample on the 1 Hz refresh.
- [ ] 5.4 Implement the NTP gate using `timedatectl show -p NTPSynchronized --value`, with an off or neutral frame and a log every 30 seconds while waiting.
- [ ] 5.5 Implement the 1 Hz refresh and the retry-on-write-failure behavior.
- [ ] 5.6 Implement graceful SIGTERM handling that writes the uniform dim shutdown pattern (raw 32,32,32 across all logical pixels, padding black), logs the stop, and exits successfully.
- [ ] 5.7 Implement single-instance protection so a second daemon cannot write the strip.
- [ ] 5.8 Add the systemd unit with the user, groups, restart policy, time-sync ordering, `TimeoutStopSec=10`, and hardening flags from `design.md`.
- [ ] 5.9 Add structured logging for startup, NTP waits, minute changes, color changes, and output errors; confirm journal visibility.
- [ ] 5.10 Add a test that SIGTERM produces exactly one shutdown frame, that it contains no word from the previous minute, that every logical pixel is 32,32,32, and that padding pixels stay black.

## 6. Build and release

- [ ] 6.1 Add CI for `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` on default features.
- [ ] 6.2 Add an aarch64 `cross build --release --target aarch64-unknown-linux-gnu --features hardware` job and run the resulting binary on the Pi.
- [ ] 6.3 Add a tag-triggered release workflow that packages the binary, systemd unit, example config, licenses, and SHA-256.
- [ ] 6.4 Add Dependabot for Cargo and GitHub Actions and pin the Rust toolchain.
- [ ] 6.5 Verify the release binary starts on the Pi and reports its version without the Ruby, Fadecandy, or OPC stack.

## 7. Deployment and legacy removal

- [ ] 7.1 Add a `wordclock` Ansible role that installs the pinned release, verifies the checksum, creates the user and groups, installs the config and unit, and enables SPI.
- [ ] 7.2 Remove the `fadecandy` role, the `nerab.ruby` role, the `ansistrano` deployment, and the wordclock cron entries from `uhlig-it/infrastructure`.
- [ ] 7.3 Provision a freshly flashed 64-bit SD card with one Ansible run and confirm the service is active and the SPI device exists.
- [ ] 7.4 Confirm `fcserver` is absent or disabled, no Ruby cron remains, and the host no longer needs Ruby.
- [ ] 7.5 Add a rollback section to the infrastructure docs that describes the card swap and Fadecandy reconnection.

## 8. Cutover and validation

- [ ] 8.1 Capture fixed-color reference photos of the legacy clock at 00:00, 00:06, 01:05, 11:15, 12:15, 13:15, and 23:55.
- [ ] 8.2 Render the same times and colors with the Rust build and compare the words, pixel positions, and gamma-corrected colors side by side.
- [ ] 8.3 Run the new daemon for 24 hours and confirm every minute updates, no minute is skipped, and the color changes once per minute.
- [ ] 8.4 Reboot the Pi and confirm the shutdown pattern on the way down, then the NTP gate, service start, and correct first frame.
- [ ] 8.5 Disconnect the network after startup and confirm the clock keeps rendering from the local clock.
- [ ] 8.6 Stop and start the service and confirm the strip shows the shutdown pattern, then recovers to the current time.
- [ ] 8.7 Run the rollback drill: swap the old SD card back in, reconnect Fadecandy, and confirm the legacy clock works.
- [ ] 8.8 After at least two weeks of stable operation, remove Fadecandy from the signal path and retire the board; keep the old SD card as the last-resort image.

## 9. Documentation and follow-up

- [ ] 9.1 Write the new repo README with build, simulator, configuration, deployment, and troubleshooting instructions.
- [x] 9.2 Write the hardware guide with the wiring diagram, BOM, SPI enablement, test-pattern procedure, and power/ground notes.
- [ ] 9.3 Link the new repo to the legacy repo and the Fadecandy handoff; state that the legacy repo is the behavioral oracle.
- [ ] 9.4 Record the verified physical mapping and pixel counts in the hardware guide.
- [ ] 9.5 Create separate OpenSpec proposals for the deferred Reminders ideas, starting with the highest-value ones: smooth fade, night dimming, web/Prometheus/MQTT observability, and sensors.
