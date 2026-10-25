# ESP32-C3 gate spike

This is a throwaway artifact for the host decision in design D16. It is not the production firmware.

- Board: ESP32-C3-DevKit-RUST-1
- On-board WS2812: GPIO2
- RMT driver: `esp-hal-smartled` 0.18.0, `WS2812_TIMING`, GRB order
- Behavior: cycles the on-board LED through red, green, blue, white, and off every 700 ms, and logs each step over USB Serial/JTAG.

```bash
cargo build --release
espflash flash --chip esp32c3 --port /dev/cu.usbmodem1101 target/riscv32imc-unknown-none-elf/release/esp32c3-blink
espflash monitor --non-interactive --chip esp32c3 --port /dev/cu.usbmodem1101
```

If the Pi host is selected, delete this directory. If the ESP32-C3 is selected, move the working code into the firmware crate during the workspace split in task 4.9.
