# wordclock-rs

Rust replacement for the Fadecandy/OPC/Ruby word clock at [github.com/suhlig/wordclock](https://github.com/suhlig/wordclock).

Status: planning complete, crate scaffold in place, clock port not started. The plan is an OpenSpec change under `openspec/changes/replace-fadecandy-with-rust/`.

Start with [HANDOFF.md](HANDOFF.md), then read the proposal, design, specs, and tasks in that order.

The target is a single aarch64 Linux binary that drives 288 WS2812 pixels over SPI0 on a 64-bit Raspberry Pi 3B, preserving the legacy clock's visible behavior exactly. The legacy Ruby implementation remains the behavioral oracle until the port is proven.

The physical build is documented in [docs/hardware.md](docs/hardware.md): bill of materials, wiring, SPI enablement, test patterns, power and grounding, and the rollback procedure.

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

The default build is hardware-free and runs on macOS. The Pi build enables the hardware backend:

```bash
cross build --release --target aarch64-unknown-linux-gnu --features hardware
```

## Validate the plan

```bash
openspec validate replace-fadecandy-with-rust --strict
openspec status --change replace-fadecandy-with-rust
openspec instructions apply --change replace-fadecandy-with-rust
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
