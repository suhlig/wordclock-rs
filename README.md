# wordclock-rs

> [!WARNING]
> **One 5 V supply, never two.** The finished clock is powered from a single 5 V supply that feeds the ESP32, the current sensor, and the LED strip. Power enters the ESP32 at the pin **labelled `5V` on the silkscreen**, header position 19 in the 28-pin numbering (other boards label the same pin `VIN`). That is a power pin with no GPIO number: do not confuse it with **GPIO19**, which is USB D+ on header position 25 and must never see 5 V.
>
> - **Never connect USB while the 5 V rail is powered.** VBUS and the `5V` pin are the same node, so you would be paralleling two supplies with no back-feed protection.
> - **Never leave USB connected with the 5 V rail switched off.**
> - **Firmware updates go over OTA**, so the built clock needs neither USB nor a serial console. See `spike/esp32c3-ota` for the prototype and the `curl` command.
> - **Never feed the `3V3` pin.** It is an output.
>
> First flash and recovery are the exceptions; for those, unplug the 5 V rail first, or use a charge-only cable with the VBUS wire cut. The reasoning is design decision D9 in `openspec/changes/add-power-budget-and-current-sensing/design.md`.

Rust replacement for the Fadecandy/OPC/Ruby word clock at [github.com/suhlig/wordclock](https://github.com/suhlig/wordclock).

Status: the Fadecandy replacement is implemented and accepted. The ESP32-C3-DevKit-RUST-1 is the selected host, with the Raspberry Pi 3B retained as the rollback path. Its OpenSpec change is archived at `openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/`, and its six capabilities are the current specs under `openspec/specs/`. Three changes are still planned: the web clock face, the power budget, and `finish-cutover-and-validation`, which carries the cutover items that were open at archive time.

Start with [HANDOFF.md](HANDOFF.md), then read the proposal, design, specs, and tasks in that order.

The target is a single Rust implementation that drives 288 WS2812 pixels, preserving the legacy clock's visible behavior exactly. The clock core, fixture, and simulator are host-independent. The host is the ESP32-C3-DevKit-RUST-1 over SPI2 with DMA (four SPI bits per LED bit) plus Wi-Fi/SNTP; the Raspberry Pi 3B over SPI0 remains the rollback path. See `docs/decisions.md` (WCR-D16–WCR-D19). The legacy Ruby implementation remains the behavioral oracle until the port is proven.

The selected ESP32-C3 build is documented in [docs/esp32-c3.md](docs/esp32-c3.md): wiring, BOM, credentials, flash/OTA, status page, and rollback. The shared hardware notes and the Pi fallback are in [docs/hardware.md](docs/hardware.md). The ESP32-C3 host evaluation and measurement log is in [docs/esp32-c3-spike.md](docs/esp32-c3-spike.md).

## Implemented and planned

OpenSpec keeps the two states in separate directories, and reading them as the two halves of the project is the quickest way to see where the clock stands: `openspec/specs/` is what is implemented and accepted, `openspec/changes/` is what is still planned.

```bash
openspec list --specs      # implemented capabilities and their requirement counts
openspec list              # active changes, with task progress
openspec list --all        # active and archived changes
```

Implemented: `clock-rendering`, `color-pipeline`, `led-output`, `display-simulator`, `clock-daemon`, and `deployment`, all archived with the Fadecandy replacement on 2026-11-03. Planned: `add-web-clock-face` and `add-power-budget-and-current-sensing`, neither of which has code yet, plus `finish-cutover-and-validation` for the bench checks and the cutover that the archived change left open.

Decisions are indexed in [docs/decisions.md](docs/decisions.md) with a stable ID and a status (`accepted`, `accepted (fallback)`, `superseded`, `planned`). Use those IDs, not a bare `D19`: OpenSpec numbers decisions per change, so every change numbers from its own `D1`.

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

The default build is hardware-free and runs on macOS. The selected ESP32-C3 firmware uses the `riscv32imc-unknown-none-elf` target and `espflash`; it is not part of the default build. The Pi fallback backend is built with `--features hardware`:

```bash
cross build --release --target aarch64-unknown-linux-gnu -p wordclock-host --features hardware
```

The ESP32-C3 firmware crate is `wordclock-esp32`; the working reference implementation remains in `spike/esp32c3-ota` until task 4.10 ports the transport.

## Simulator

```bash
cargo run -p wordclock-host -- sim "12.11.2017 17:23"
cargo run -p wordclock-host -- sim "12.11.2017 17:23" --color ff0000 --seed 42
cargo run -p wordclock-host -- sim "2026-01-01T00:30:00" --seed 42
cargo run -p wordclock-host -- demo --interval 0.2
```

## Validate the plan

```bash
openspec validate --all --strict
openspec status --change add-web-clock-face
openspec status --change add-power-budget-and-current-sensing
openspec status --change finish-cutover-and-validation
```

The archived change is plain files under `openspec/changes/archive/`; `openspec status` no longer addresses it, and its tasks and design are read directly from that directory.

## Ebook

```bash
./scripts/build-openspec-ebook.sh
```

Renders `dist/replace-fadecandy-with-rust.epub` and `.pdf` for reading on a tablet. It always produces **one** book.

The book opens with a cover built from `docs/images/wordclock-front.jpg`, then renders **every change under `openspec/changes/` that has a `proposal.md`**: the primary change first, with its proposal, design, tasks and capability specs under their plain headings, and each further change after it alphabetically with its headings prefixed by the change name so the table of contents cannot show two unrelated `Proposal` entries. A new change joins the book without editing the script.

There is deliberately no option to render a subset. An earlier version accepted a change name, which produced a second file with the same title, subtitle and cover but a different scope — impossible to tell apart on a tablet, and never what anyone wanted.

After the changes come the project documentation as reference material: this README, the hardware guide, the ESP32-C3 host audit, the OTA spike runbook, the strip colour fault handover, the runtime metrics contract, and the project handoff.

The output keeps the primary change's name, so the paths above stay valid. Mermaid diagrams are rendered with `mermaid-cli`, fetched through `npx` unless `mmdc` is already on `PATH`.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
