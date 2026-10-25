# wordclock-rs

Rust replacement for the Fadecandy/OPC/Ruby word clock at [github.com/suhlig/wordclock](https://github.com/suhlig/wordclock).

Status: planning complete, crate scaffold and golden fixture in place, clock core, color pipeline, and simulator complete; host decision gate next. The host is an open decision between the Raspberry Pi 3B and an ESP32-C3; the plan is an OpenSpec change under `openspec/changes/replace-fadecandy-with-rust/`.

Start with [HANDOFF.md](HANDOFF.md), then read the proposal, design, specs, and tasks in that order.

The target is a single Rust implementation that drives 288 WS2812 pixels, preserving the legacy clock's visible behavior exactly. The clock core, fixture, and simulator are host-independent. The host is selected after a measured spike: either a 64-bit Raspberry Pi 3B over SPI0, or an ESP32-C3-DevKit-RUST-1 over RMT with Wi-Fi/SNTP. See design decisions D16–D18. The legacy Ruby implementation remains the behavioral oracle until the port is proven.

The physical build is documented in [docs/hardware.md](docs/hardware.md): bill of materials, wiring, SPI enablement, test patterns, power and grounding, and the rollback procedure. The ESP32-C3 host evaluation and measurement log is in [docs/esp32-c3-spike.md](docs/esp32-c3-spike.md).

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

The default build is hardware-free and runs on macOS. Until the host decision, the Pi backend is built with `--features hardware`:

```bash
cross build --release --target aarch64-unknown-linux-gnu --features hardware
```

The ESP32-C3 spike uses the `riscv32imc-unknown-none-elf` target and `espflash`; it is not part of the default build.

## Simulator

```bash
cargo run -- sim "12.11.2017 17:23"
cargo run -- sim "12.11.2017 17:23" --color ff0000 --seed 42
cargo run -- sim "2026-01-01T00:30:00" --seed 42
cargo run -- demo --interval 0.2
```

## Validate the plan

```bash
openspec validate replace-fadecandy-with-rust --strict
openspec status --change replace-fadecandy-with-rust
openspec instructions apply --change replace-fadecandy-with-rust
```

## Ebook

```bash
./scripts/build-openspec-ebook.sh
```

Renders `dist/replace-fadecandy-with-rust.epub` and `.pdf` for reading on a tablet. The book opens with a cover built from `docs/images/wordclock-front.jpg`, followed by the OpenSpec proposal, design, tasks, and capability specs, then the project documentation: this README, the hardware guide, the ESP32-C3 spike, and the handoff. Mermaid diagrams are rendered with `mermaid-cli`, fetched through `npx` unless `mmdc` is already on `PATH`.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
