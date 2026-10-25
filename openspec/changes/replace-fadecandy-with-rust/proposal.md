# Proposal

## Why

The word clock has been stable for almost ten years, but its software chain is now the weak point: Fadecandy is unmaintained, the production server is a prebuilt 32-bit `fcserver-rpi` binary, and that binary is the only reason the Pi still runs a 32-bit OS. The clock itself is small, well tested, and well understood, so replacing the dead layer with a single supported Rust implementation is now cheaper and safer than preserving the current stack. The host is a separate question: the existing Pi 3B is the incumbent, but the 288-pixel, once-per-minute workload also fits a single ESP32-C3, which removes the SD card, the OS, and the Fadecandy-era deployment path entirely. The plan therefore keeps the clock core host-independent and decides the host on measured evidence rather than assumption.

## What Changes

- Replace `fcserver`, Open Pixel Control, and the `faderuby` client with an in-process Rust implementation that writes the 288-pixel frame directly to the WS2812 strip.
- Keep the clock renderer, color table, occasion rules, and gamma 2.5 correction independent of the host and its output transport, so the same fixture-tested logic runs on either candidate.
- Evaluate two hosts and select exactly one after a measured spike: the existing Raspberry Pi 3B with 64-bit Raspberry Pi OS Lite and SPI0, or an ESP32-C3 (ESP32-C3-DevKit-RUST-1) with the RMT peripheral and Wi-Fi/SNTP.
- Keep a hardware-free terminal simulator and demo loop so clock logic can be developed and tested without a Pi, an ESP32-C3, or LEDs.
- Replace the per-minute cron job with a long-running service on the selected host: a systemd daemon on the Pi, or a firmware loop with a watchdog and Wi-Fi reconnect on the ESP32-C3.
- Preserve the uniform dim shutdown pattern as the final frame on an orderly Pi stop. The ESP32-C3 cannot guarantee this on power loss; that limitation is documented and the post-boot NTP gate is retained.
- Add a 74AHCT125 (or 74HCT245) level shifter and series resistor between the selected host's data pin and the strip, replacing the level shifting and series termination that Fadecandy currently provides.
- **BREAKING**: remove the OPC server/client interface. Production only ever used a localhost OPC connection, and no external OPC client is part of the clock.
- **BREAKING**: retire the 32-bit Raspberry Pi OS and the Ruby/Ansible deployment path. The selected replacement is either an aarch64 Linux binary installed by Ansible, or an ESP32-C3 firmware image installed by flashing/OTA.
- **BREAKING**: change the physical data path from Fadecandy USB to the selected host's GPIO plus a level shifter. The LED wiring itself is not changed until a numbered test pattern confirms the physical order.

## Host Decision

The host is not assumed. The clock core and simulator are built first; then a time-boxed ESP32-C3 spike measures RMT output reliability, boot-to-first-frame time, Wi-Fi/SNTP behavior, flash and RAM use, and OTA/deployment effort. The Pi 3B path is measured against the same criteria. The selected host is recorded in `design.md` and only that host's hardware, daemon, release, and deployment tasks are executed.

## Capabilities

### New Capabilities

- `clock-rendering`: turns local time into the exact 18×16 German word pattern, including the current phrasing quirks and pixel indices, independently of the host and output transport.
- `color-pipeline`: selects the uniform or per-pixel color for the current time, applies the current occasion rules, and applies gamma 2.5/whitepoint correction.
- `led-output`: writes a complete RGB frame to the WS2812 strip over the selected host's transport (SPI0 on the Pi, RMT on the ESP32-C3), including physical pixel count, padding, and refresh behavior.
- `display-simulator`: renders the same frame to a terminal and runs the demo loop without hardware.
- `clock-daemon`: runs continuously on the selected host, schedules minute updates, gates startup on time synchronization, logs, writes the shutdown pattern where an orderly stop exists, and shuts down or restarts cleanly.
- `deployment`: builds and publishes the selected target's artifact and provisions either a 64-bit Pi with SPI and systemd or an ESP32-C3 with firmware, Wi-Fi credentials, and OTA.

### Modified Capabilities

None. This repository has no existing OpenSpec specs; the current Ruby implementation is the behavioral reference, not an OpenSpec capability.

## Impact

- New repository `github.com/suhlig/wordclock-rs`; `github.com/suhlig/wordclock` remains the legacy reference and the source of the golden test fixture.
- New Rust crates and a new CI/release pipeline; no Ruby, Bundler, Fadecandy, or OPC dependency in production.
- `uhlig-it/infrastructure` loses the `fadecandy` role, the Ruby role, the cron entries, and the `ansistrano` deployment for this host, and gains a `wordclock` role for the selected host.
- Physical hardware: one level shifter, one series resistor, decoupling capacitors, a short data lead from the selected host to the strip, and either a new Pi SD card or an ESP32-C3-DevKit-RUST-1. The Pi 3B and Fadecandy board remain available as rollback until the selected host has proven itself.
- Operational behavior is unchanged for the viewer: the clock shows the same words, the same color cadence, and the same gamma-corrected colors, 24/7, except for the explicit dim shutdown pattern during an orderly Pi stop and the documented ESP32-C3 power-loss limitation.
- No web UI, Prometheus, MQTT, sensors, animations, night blackout, or wording cleanups in this change; the architecture leaves room for them, but they are separate changes.
