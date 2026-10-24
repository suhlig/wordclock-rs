# Proposal

## Why

The word clock has been stable for almost ten years, but its software chain is now the weak point: Fadecandy is unmaintained, the production server is a prebuilt 32-bit `fcserver-rpi` binary, and that binary is the only reason the Pi still runs a 32-bit OS. The clock itself is small, well tested, and well understood, so replacing the dead layer with a single supported Rust daemon is now cheaper and safer than preserving the current stack.

## What Changes

- Replace `fcserver`, Open Pixel Control, and the `faderuby` client with an in-process Rust daemon that writes the 288-pixel frame directly to the WS2811B strip over SPI.
- Port the existing clock logic, XKCD color table, occasion rules, and Fadecandy gamma 2.5 color correction to Rust, preserving the current visible output exactly.
- Keep a hardware-free terminal simulator and demo loop so clock logic can be developed and tested without a Pi or LEDs.
- Replace the per-minute cron job with a long-running systemd service that owns the display, resamples the uniform color once per minute, and gates the first render on NTP synchronization.
- Reflash the existing Raspberry Pi 3B with 64-bit Raspberry Pi OS Lite on a new SD card; keep the old SD card and Fadecandy board as a rollback path.
- Add a 74AHCT125 (or 74HCT245) level shifter and series resistor between the Pi's SPI0 MOSI pin and the strip, replacing the level shifting and series termination that Fadecandy currently provides.
- **BREAKING**: remove the OPC server/client interface. Production only ever used a localhost OPC connection, and no external OPC client is part of the clock.
- **BREAKING**: retire the 32-bit Raspberry Pi OS and the Ruby/Ansible deployment path. The replacement is an aarch64 static-style binary built in CI and installed by Ansible.
- **BREAKING**: change the physical data path from Fadecandy USB to Pi SPI0 plus a level shifter. The LED wiring itself is not changed until a numbered test pattern confirms the physical order.

## Capabilities

### New Capabilities

- `clock-rendering`: turns local time into the exact 18×16 German word pattern, including the current phrasing quirks and pixel indices.
- `color-pipeline`: selects the uniform or per-pixel color for the current time, applies the current occasion rules, and applies gamma 2.5/whitepoint correction.
- `led-output`: writes a complete RGB frame to the WS2811B strip over SPI, including physical pixel count, padding, and refresh behavior.
- `display-simulator`: renders the same frame to a terminal and runs the demo loop without hardware.
- `clock-daemon`: runs continuously, schedules minute updates, gates startup on NTP, logs, and shuts down cleanly under systemd.
- `deployment`: builds and publishes the aarch64 binary and provisions the 64-bit Pi, SPI, service, and configuration reproducibly.

### Modified Capabilities

None. This repository has no existing OpenSpec specs; the current Ruby implementation is the behavioral reference, not an OpenSpec capability.

## Impact

- New repository `github.com/suhlig/wordclock-rs`; `github.com/suhlig/wordclock` remains the legacy reference and the source of the golden test fixture.
- New Rust crates and a new CI/release pipeline; no Ruby, Bundler, Fadecandy, or OPC dependency in production.
- `uhlig-it/infrastructure` loses the `fadecandy` role, the Ruby role, the cron entries, and the `ansistrano` deployment for this host, and gains a `wordclock` binary/systemd role.
- Physical hardware: one level shifter, one series resistor, decoupling capacitors, a new SD card, and a short data lead from the Pi to the strip.
- Operational behavior is unchanged for the viewer: the clock shows the same words, the same color cadence, and the same gamma-corrected colors, 24/7.
- No web UI, Prometheus, MQTT, sensors, animations, night blackout, or wording cleanups in this change; the architecture leaves room for them, but they are separate changes.
