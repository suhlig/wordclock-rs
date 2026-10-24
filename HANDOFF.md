# Wordclock Rust rewrite — handoff

Status: planning complete, implementation not started. OpenSpec 1.14.2 validates this change with `openspec validate replace-fadecandy-with-rust --strict`.

This repository is the planned replacement for `github.com/suhlig/wordclock`. It is not a git repository yet and nothing has been pushed. The legacy repository remains the behavioral oracle and must not be changed except to generate the fixture.

## Read in this order

1. `openspec/changes/replace-fadecandy-with-rust/proposal.md` — why and what.
2. `openspec/changes/replace-fadecandy-with-rust/design.md` — decisions, wiring, parity contract, risks.
3. `openspec/changes/replace-fadecandy-with-rust/tasks.md` — the 62-step checklist to execute.
4. `openspec/changes/replace-fadecandy-with-rust/specs/*/spec.md` — the behavior contract for each capability.

Then run `openspec instructions apply --change replace-fadecandy-with-rust` for the working instructions. Work tasks in order and check them off in `tasks.md`.

## Decisions already made

| Area | Decision |
| :--- | :--- |
| Language | Rust rewrite, phased. Ruby stays only as the legacy oracle. |
| Repository | New public `github.com/suhlig/wordclock-rs`, MIT OR Apache-2.0. Legacy repo unchanged. |
| Hardware | Keep the Raspberry Pi 3B; reflash a new SD with 64-bit Raspberry Pi OS Lite. Old SD is rollback. |
| LED output | Pi SPI0 GPIO10 at 3 MHz, 74AHCT125 or 74HCT245 level shifter, 470 Ω series resistor, common ground. |
| Runtime | Long-running systemd daemon, not cron. |
| v1 scope | Parity only, plus an explicit dim shutdown pattern on orderly stops: same words, colors, gamma, cadence, 24/7. No other new user-facing features. |
| Simulator | Terminal snapshot, fixed color/seed override, and full-day demo loop. No web UI in v1. |
| Wording | Preserve the current hybrid phrasing and quirks exactly. Fix them only in a later change. |
| Colors | New random color each minute on regular days; per-pixel on carnival, birthdays, and New Year party; grey on Ash Wednesday and New Year hangover. |
| Gamma | Fadecandy curve with gamma 2.5, whitepoint 1,1,1, linearCutoff 0. Dithering only if visual comparison requires it. |
| NTP | Do not show time after boot until `timedatectl` reports synchronized. |
| Shutdown | On SIGTERM, write a uniform dim fill (raw 32,32,32) across all logical pixels as the final frame; never leave a stale minute that reads as current. |
| Build | GitHub Actions cross-compiles aarch64; Ansible installs the pinned release and removes Fadecandy/Ruby/cron. |
| Rollback | Card swap plus Fadecandy reconnect; keep the old SD and board until two weeks of stable operation, then remove the board from the signal path and retire it. |

Hardware note (2026-10-24): the strip in use is WS2812, not WS2811B. Both use the same 800 kHz NZR protocol and the same 0.7 × VDD input threshold at 5 V, so the wiring, SPI encoding, and software are unchanged. The Adafruit BSS138 level shifter is not suitable for the data line: its 10 kΩ pull-ups produce roughly 1–2 µs edges, while a WS2812 bit is 1.25 µs and its high pulses are 0.4/0.8 µs. Use the 74AHCT125 or 74HCT245 from D3.

Host note (2026-10-24): the Raspberry Pi Zero 2 W at `rpizero.local` (now `wordclock-dev`) is suitable for development and staging, and is a production candidate after the SPI/strip test and a 24-hour soak. Keep the 3B and the old SD as the rollback path.

## Non-negotiables

- Do not change the wording while porting. The golden fixture encodes every current quirk.
- Do not resample colors on the 1 Hz refresh. Colors change once per minute, except per-pixel occasion frames which are also held for the minute.
- Do not trust identity pixel mapping until the walking-pixel and row test patterns have been run on the real strip.
- Do not power the Pi from the LED supply without a protected 5 V path.
- Do not add web, MQTT, Prometheus, sensors, animations, night blackout, or OPC in this change.
- Do not remove the old SD card or Fadecandy board until the rollback drill passes; after two weeks of stability, remove the board from the signal path and retire it. The board is a temporary rollback aid, not part of the target hardware.
- Do not use the dead GitHub Fadecandy URLs or `git submodule sync` in the reference clone.

## Parity oracle

The legacy clock's full-day fixture is the authority:

| Artifact | Value |
| :--- | :--- |
| `fixtures/minutes.json` | 1,440 entries, 503,999 bytes |
| SHA-256 | `33824892ca69303599c72cf6c7651a0f1d0bb9111ae312ef51dc943278df454e` |
| Stripe checksum | `aed1c38776d7be5a34e40eb376dce927d4a80efb6f7781a874a8eaa7adfe65b8` |
| Normalized XKCD list | 922 lines, SHA-256 `9ac5913dccc2200cd46e2b5c4f61d05a5779d8f934b99788c652b34b3ee75d0e` |

Generate the fixture from the legacy repository with the Ruby command in `design.md` section D10. Do not edit the fixture by hand.

Useful legacy facts:

- 288 logical pixels, 16 rows of 18.
- Maximum lit index is 268; indices 269–287 are never used by current phrases.
- A frame lights 11–39 pixels.
- The XKCD table has 922 colors, not 932.
- The live Pi is a 3B Rev 1.2 on Raspbian 13 trixie 32-bit, with Ruby 4.0.1, no Rust/Go, `fcserver` active, cron every minute, and SPI disabled.
- `timedatectl show -p NTPSynchronized --value` already returns `yes` on the current host; the new 64-bit image must be checked again.

## First actions for the implementing agent

1. Create the GitHub repository and copy this directory into it. Do not push the legacy repository.
2. Add the Rust workspace and the `hardware` feature split. Confirm `cargo test` passes on macOS without hardware.
3. Generate and verify the golden fixture before writing the clock renderer.
4. Port the clock renderer and color pipeline against the fixture; do not touch wording.
5. Build the terminal simulator and demo loop; use them for all development before touching the Pi.
6. Flash the new 64-bit SD, enable SPI, wire the level shifter, and run a minimal SPI blink on the real strip.
7. Implement and run the test pattern; record the physical order and the real pixel count.
8. Only then implement the daemon, systemd unit, release workflow, and Ansible role.
9. Compare fixed-color frames with Fadecandy, run the 24-hour soak, reboot test, network-loss test, and rollback drill.
10. Remove Fadecandy from the signal path only after the rollback path and two weeks of stability are confirmed.

## Human approvals still needed

- Verify the new SSH host-key fingerprint for `wordclock` before automation trusts it; the key changed since the last connection.
- Confirm the physical pixel order and whether the extra 12 LEDs beyond 288 are connected.
- Confirm the downtime window for the SD swap and the new repo creation.
- Confirm that no other host or tool depends on the old OPC port 7890; the plan drops OPC entirely.
- Confirm the level-shifter part on hand; if not, order the BOM from `design.md` section D3.

## OpenSpec commands

```bash
openspec validate replace-fadecandy-with-rust --strict
openspec status --change replace-fadecandy-with-rust
openspec instructions apply --change replace-fadecandy-with-rust
openspec view
```

After the change is implemented and every task is checked, archive it with `openspec archive replace-fadecandy-with-rust -y`. At that point the six capability specs move into `openspec/specs/` and become the source of truth for future changes.
