# ESP32-C3 gate firmware

This is the measurement harness for the ESP32-C3 side of design D16. It combines Wi-Fi association, DHCP, SNTP, the real host core with the `Europe/Berlin` rule, and a 288-pixel RMT frame in one image, and prints the timings the gate asks for.

The library includes the real `src/clock.rs`, `src/color.rs`, `src/frame.rs`, and `src/timezone.rs` through `#[path]`, so the measurements exercise the same code the host runs rather than a copy.

- Board: ESP32-C3-DevKit-RUST-1
- LED data: GPIO2 over RMT, 288 logical pixels, `WS2812_TIMING`, GRB order
- Network: `esp-radio` 1.0.0-beta.1 station mode, `embassy-net` DHCP/DNS, `sntpc` over UDP
- Partitions: `partitions.csv` defines two OTA app slots of 1,966,080 bytes for the flash-budget measurement
- Measurements are replayed on every 60-second resync because the USB monitor usually attaches after the one-shot boot lines

## Credentials

Copy `wifi.env.example` to `wifi.env` and fill in your 2.4 GHz credentials. `wifi.env` is gitignored; the password is compiled into this image for the spike only.

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

The monitor never exits on its own. Capture 90 seconds or more to be sure of catching a replayed summary.

## GATE lines

| Line | Meaning |
| :--- | :--- |
| `GATE boot_us` | Systimer value after `esp_hal::init`, measured from chip reset |
| `GATE first_frame_us` | Reset to the completed first (neutral) frame |
| `GATE summary` | Boot, first frame, Wi-Fi up, first SNTP, first clock frame, and frame time |
| `GATE timing` | All-zero, all-white, and clock-frame write durations |
| `GATE heap` | Heap size, boot usage, peak usage, and the static RMT buffer size |
| `GATE connected_us`, `GATE disconnected_us` | Wi-Fi events for the reconnect measurement |

## Flash budget with OTA

```bash
espflash save-image --chip esp32c3 --partition-table partitions.csv \
  --target-app-partition ota_0 \
  target/riscv32imc-unknown-none-elf/release/esp32c3-wifi /tmp/gate_app.bin
```

The command reports `App/part. size` against the 1,966,080-byte OTA slot. Recorded result: 556,496 bytes, 28.30%.

## Reconnect measurement

```bash
cargo build --release --features reconnect-test
espflash flash --chip esp32c3 --port /dev/cu.usbmodem1101 target/riscv32imc-unknown-none-elf/release/esp32c3-wifi
espflash monitor --non-interactive --chip esp32c3 --port /dev/cu.usbmodem1101
```

With the feature, the firmware forces a disconnect 15 seconds after each association. The delta between `GATE disconnected_us` and the next `GATE connected_us` is the reconnect time; 5 seconds of it is the deliberate backoff.

## Logic-analyzer capture

The `frame-loop` feature skips Wi-Fi and writes the 06:30 frame back-to-back with a 2 ms gap, so a fixed-depth capture always contains a whole frame. Clip the Pico H to GPIO2 directly; it is a 3.3 V signal, so the level shifter is not needed for timing work.

Build the host client once with `scripts/setup-sigrok-pico.sh` and add `$HOME/opt/sigrok-pico/bin` to `PATH`. The released sigrok-cli and the Homebrew HEAD formula do not provide a working `raspberrypi-pico` driver.

```bash
cargo build --release --features frame-loop
espflash flash --chip esp32c3 --port /dev/cu.usbmodemXXXX target/riscv32imc-unknown-none-elf/release/esp32c3-wifi
sigrok-cli -d raspberrypi-pico:conn=/dev/cu.usbmodemYYYY:serialcomm=115200/flow=0 \
  --config samplerate=24000000 --channels D2 --samples 380000 -o /tmp/ws2812.sr
sigrok-cli -i /tmp/ws2812.sr -P rgb_led_ws281x -A rgb_led_ws281x=rgb-vals
```

`D2` is GPIO2 on the Pico. Do not use a trigger: the `raspberrypi-pico` driver switches to USB streaming for triggered captures and overflows at 24 MHz. Without a trigger it uses fixed-depth capture at full rate.

Recorded result: the longest low inside a frame is 0.708 µs, the decoded 288 pixels match `examples/dump_frame.rs` for the same time and seed, and the frame carries 7.311 ms of bitstream. The 50 µs WS2812 reset is never approached.

If the Pi host is selected, delete this directory. If the ESP32-C3 is selected, task 4.9 moves the shared code into a real `wordclock-core` crate and this harness becomes the basis of the firmware.
