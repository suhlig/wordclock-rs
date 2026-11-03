# Wordclock Rust rewrite — handoff

Status: planning complete, crate scaffold and golden fixture in place, clock core, color pipeline, and simulator complete; host selected. The ESP32-C3-DevKit-RUST-1 is the selected host as of 2026-11-03; the Raspberry Pi 3B is retained only as the rollback path. OpenSpec 1.14.2 validates this change with `openspec validate replace-fadecandy-with-rust --strict`.

Progress: tasks 1.1–1.5, 2.1–2.10, 3.1–3.6, 4.1–4.7, 4.8, 4.9, and 4.10 are complete. The ESP32-C3 is the selected host and the workspace is split into `wordclock-core`, `wordclock-host`, and `wordclock-esp32`. The SPI2+DMA strip transport is in `wordclock-esp32/src/display.rs` and was verified on the 335-pixel chain on 2026-11-03 (revision 26: clean boundary pattern, 2.122 A). The on-board WS2812 RMT test was confirmed on GPIO2; the offcut strip now runs on GPIO3 over SPI2 with DMA. Wi-Fi + SNTP is implemented in `spike/esp32c3-wifi` and verified on the board: DHCP, SNTP, the boot gate, and reconnect after a forced disconnect all pass. `spike/esp32c3-frame` compiles the real core and renders the 06:30 frame on the C3; its dump is byte-identical to the host simulator for a fixed color and two seeded scenarios. Task 4.7 is measured: boot to first frame 246 ms, 288-pixel RMT frame 8.1–10.5 ms, OTA image 28.3% of a slot, 54 KiB heap peak, reconnect 5.1 s, and no RMT inter-chunk gap above 0.708 µs in the `frame-loop` capture (Pico analyzer at 24 MHz, all 288 pixels decoded identically to the host). That capture is the no-Wi-Fi control only: at 335 pixels with Wi-Fi active, RMT missed its 30 µs refill deadline and shifted the frame, so the C3 transport is SPI2 with DMA (D19). At the four-bit cell the SPI wire time is about 11.5 ms for 288 pixels and 13.3 ms for 335. Task 4.3 is complete: the 35-pixel offcut runs through the shifter on GPIO3, and the primaries plus the walking pixel prove GRB order and index order. The 5 V side was verified by eye, not measured, so the edge quality at `1Y` is still unmeasured; the timing evidence remains the 4.7 capture at 3.3 V. The working log is `docs/esp32-c3-spike.md`.

This repository is the planned replacement for `github.com/suhlig/wordclock`. It is a public git repository and the current branch is pushed. The legacy repository remains the behavioral oracle and must not be changed except to generate the fixture.

## Read in this order

1. `openspec/changes/replace-fadecandy-with-rust/proposal.md` — why and what.
2. `openspec/changes/replace-fadecandy-with-rust/design.md` — decisions, the host decision D16–D19, wiring, parity contract, risks.
3. `openspec/changes/replace-fadecandy-with-rust/tasks.md` — the task list to execute.
4. `openspec/changes/replace-fadecandy-with-rust/specs/*/spec.md` — the behavior contract for each capability.

Then run `openspec instructions apply --change replace-fadecandy-with-rust` for the working instructions. Work tasks in order and check them off in `tasks.md`.

## Decisions already made

| Area | Decision |
| :--- | :--- |
| Language | Rust rewrite, phased. Ruby stays only as the legacy oracle. |
| Repository | New public `github.com/suhlig/wordclock-rs`, MIT OR Apache-2.0. Legacy repo unchanged. |
| Hardware | ESP32-C3-DevKit-RUST-1, selected 2026-11-03. The Raspberry Pi 3B is the rollback path only and is not maintained. |
| LED output | ESP32-C3 (selected): SPI2 with DMA, GPIO3 as MOSI, 2.5 MHz with four SPI bits per LED bit. Pi (fallback): SPI0 GPIO10 at 2.5 MHz, same four-bit cell. Both use the 74AHCT125 or 74HCT245 level shifter, a 470 Ω series resistor, and common ground. |
| Runtime | ESP32-C3 (selected): firmware loop with watchdog, Wi-Fi reconnect, SNTP gate, and OTA. Pi (fallback): long-running systemd daemon. |
| v1 scope | Parity only: same words, colors, gamma, cadence, 24/7. No other new user-facing features. The dim shutdown pattern applies only to the Pi fallback; the ESP32-C3 cannot do it on power loss. |
| Simulator | Terminal snapshot, fixed color/seed override, and full-day demo loop. No web UI in v1. |
| Wording | Preserve the current hybrid phrasing and quirks exactly. Fix them only in a later change. |
| Colors | New random color each minute on regular days; per-pixel on carnival, birthdays, and New Year party; grey on Ash Wednesday and New Year hangover. |
| Gamma | Fadecandy curve with gamma 2.5, whitepoint 1,1,1, linearCutoff 0. Dithering only if visual comparison requires it. |
| NTP | Do not show time after boot until the time source reports synchronized: SNTP on the selected ESP32-C3, `timedatectl` on the Pi fallback. |
| Shutdown | Pi fallback only: on an orderly stop, write a uniform dim fill (raw 32,32,32) across all logical pixels as the final frame. The selected ESP32-C3 cannot do this on power loss; document the limitation and keep the neutral boot gate. |
| Build | GitHub Actions builds the selected target: an `espflash` image plus OTA metadata for the ESP32-C3. The Pi aarch64/Ansible path is fallback only. |
| Rollback | Pi: card swap plus Fadecandy reconnect. C3: previous OTA image or USB reflash, with the legacy Pi clock as the ultimate fallback. Keep the old SD and board until two weeks of stable operation. |

Hardware note (2026-10-24): the strip in use is WS2812, not WS2811B. Both use the same 800 kHz NZR protocol and the same 0.7 × VDD input threshold at 5 V, so the wiring, SPI encoding, and software are unchanged. The Adafruit BSS138 level shifter is not suitable for the data line: its 10 kΩ pull-ups produce roughly 1–2 µs edges, while a WS2812 bit is 1.25 µs and its high pulses are 0.4/0.8 µs. Use the 74AHCT125 or 74HCT245 from D3. The ESP32-C3 has the same 3.3 V logic and needs the same shifter; it drives the strip over SPI2 with DMA, not RMT (D19).

Host note (2026-10-24, decision 2026-11-03): the ESP32-C3-DevKit-RUST-1 is the selected host. The Raspberry Pi Zero 2 W at `rpizero.local` (now `wordclock-dev`) and the Pi 3B remain available as development and rollback hardware. Keep the 3B and the old SD as the rollback path.

## Non-negotiables

- Do not change the wording while porting. The golden fixture encodes every current quirk.
- Do not resample colors on the 1 Hz refresh. Colors change once per minute, except per-pixel occasion frames which are also held for the minute.
- Do not trust identity pixel mapping until the walking-pixel and row test patterns have been run on the real strip.
- Do not power the Pi or the ESP32-C3 from the LED supply without a protected 5 V path.
- Do not maintain both host paths in production. Select exactly one in task 4.8 and execute only that path.
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
2. Add the Rust crate and the `hardware` feature split. Confirm `cargo test` passes on macOS without hardware.
3. Generate and verify the golden fixture before writing the clock renderer.
4. Port the clock renderer and color pipeline against the fixture; do not touch wording.
5. Build the terminal simulator and demo loop; use them for all development before touching hardware.
6. The ESP32-C3 spike is complete and the host decision is recorded in `design.md` D16; do not provision the Pi.
7. Flash or wire only the selected host: the ESP32-C3 firmware over SPI-with-DMA. Run a minimal blink and the test pattern on the real strip.
8. Record the physical order and the real pixel count, then implement the daemon or firmware, systemd/OTA release, and deployment for the selected host.
9. Compare fixed-color frames with Fadecandy, run the 24-hour soak, reboot test, network-loss test, and rollback drill.
10. Remove Fadecandy from the signal path only after the rollback path and two weeks of stability are confirmed.

## Human approvals still needed

- Verify the new SSH host-key fingerprint for `wordclock` before automation trusts it; the key changed since the last connection.
- Confirm the physical pixel order and whether the extra 12 LEDs beyond 288 are connected.
- Confirm the downtime window for the SD swap and the new repo creation.
- Confirm that no other host or tool depends on the old OPC port 7890; the plan drops OPC entirely.
- Confirm the ESP32-C3-DevKit-RUST-1 board variant and that its Wi-Fi reaches the clock's location.
- Confirm the level-shifter part on hand; if not, order the BOM from `design.md` section D3.

## OpenSpec commands

```bash
openspec validate replace-fadecandy-with-rust --strict
openspec status --change replace-fadecandy-with-rust
openspec instructions apply --change replace-fadecandy-with-rust
openspec view
```

After the change is implemented and every task is checked, archive it with `openspec archive replace-fadecandy-with-rust -y`. At that point the six capability specs move into `openspec/specs/` and become the source of truth for future changes.
