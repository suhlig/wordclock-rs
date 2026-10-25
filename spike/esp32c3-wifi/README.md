# ESP32-C3 Wi-Fi + SNTP gate spike

This is a gate artifact for design D16, not the production firmware. It combines the on-board WS2812 with Wi-Fi, DHCP, DNS, and SNTP.

- Board: ESP32-C3-DevKit-RUST-1
- On-board WS2812: GPIO2, RMT via `esp-hal-smartled`
- Wi-Fi: `esp-radio` 1.0.0-beta.1 station mode, 2.4 GHz only
- Network: `embassy-net` DHCP and DNS
- Time: `sntpc` + `sntpc-net-embassy`, resynchronized every 60 seconds
- Boot gate: the on-board LED stays off until the first SNTP response, then turns green and stays green if the network later drops

## Credentials

Copy `wifi.env.example` to `wifi.env` and fill in your 2.4 GHz credentials. `wifi.env` is gitignored; the password is compiled into the image for this spike only.

```bash
cp wifi.env.example wifi.env
$EDITOR wifi.env
```

## Build, flash, monitor

```bash
cargo build --release
espflash flash --chip esp32c3 --port /dev/cu.usbmodem1101 target/riscv32imc-unknown-none-elf/release/esp32c3-wifi
espflash monitor --non-interactive --chip esp32c3 --port /dev/cu.usbmodem1101
```

Expected log:

```text
Starting Wi-Fi
Waiting for DHCP
Got IP: ...
SNTP synchronized: unix=...
```

The on-board LED should be off before the SNTP line and green afterwards. If the access point is restarted, the firmware reconnects and resynchronizes; the clock behavior after a later network loss is implemented in the next step.

If the Pi host is selected, delete this directory. If the ESP32-C3 is selected, move the working code into the firmware crate during the workspace split in task 4.9.
