# ESP32-C3 build guide (selected host)

This is the selected build. The Raspberry Pi 3B path in `docs/hardware.md` is the rollback only. The decision and its evidence are in `openspec/changes/replace-fadecandy-with-rust/design.md` D16, and the measurement log is `docs/esp32-c3-spike.md`.

> [!WARNING]
> **One 5 V supply, never two.** The finished clock is powered from a single 5 V supply that feeds the ESP32-C3, the current sensor, and the LED strip. Power enters the board at the pin **labelled `5V` on the silkscreen**, header position 19 in the 28-pin numbering (other boards label the same pin `VIN`). That is a power pin with no GPIO number: do not confuse it with **GPIO19**, which is USB D+ and must never see 5 V.
>
> - **Never connect USB while the 5 V rail is powered.** VBUS and the `5V` pin are the same node, so you would parallel two supplies with no back-feed protection.
> - **Never leave USB connected with the 5 V rail switched off.**
> - **Never feed the `3V3` pin.** It is an output.
> - Firmware updates go over OTA. First flash and recovery are the exceptions: unplug the 5 V rail first, or use a charge-only cable with the VBUS wire cut.

## Bill of materials

| Part | Quantity | Purpose |
| :--- | ---: | :--- |
| ESP32-C3-DevKit-RUST-1 | 1 | Controller |
| SN74AHCT125N, DIL-14 | 1 | 3.3 V to 5 V data level shift |
| IC socket, 14-pin, THT | 1 | Swap the buffer without desoldering |
| 470 Ω, 1/4 W, 1 % | 1 | Series termination at the first LED |
| 10 kΩ, 1/4 W, 1 % | 1 | Pull-down on the shifter input |
| 47 nF or 100 nF ceramic | 1 | Level-shifter decoupling |
| 1000 µF, 16 V radial | 1 | Strip input bulk capacitance |
| Perfboard, 5×7 cm | 1 | Level-shifter carrier |
| Dupont leads and a 1×40 header | 1 each | Board to carrier, data lead |
| MeanWell 5 V supply | 1 | One rail for controller, sensor, and strip |
| Raspberry Pi Pico H (optional) | 1 | sigrok-pico analyzer for the data line |

## Wiring

GPIO3 is one of only three exposed pins with no strapping role, no JTAG or internal-flash duty and no onboard peripheral, the other two being GPIO0 and GPIO1. Avoid GPIO2 (strapping, on-board WS2812), GPIO4–7 (JTAG plus flash data), GPIO8 and GPIO10 (the I²C bus to the IMU and SHTC3), GPIO9 (BOOT) and GPIO18/19 (USB).

| Connection | ESP32-C3 / part | Strip / supply |
| :--- | :--- | :--- |
| Data | GPIO3, header pin 8 | 74AHCT125 pin 2 (1A) |
| Shifted data | 74AHCT125 pin 3 (1Y) | 470 Ω, then strip DIN |
| Output enable | 74AHCT125 pin 1 (1OE) | GND |
| Logic supply | 74AHCT125 pin 14 (VCC) | LED 5 V |
| Ground | 74AHCT125 pin 7 (GND) | common ground |
| Pull-down | 10 kΩ from 1A to GND | keeps the input defined while the rail ramps |
| Decoupling | 47 nF across VCC/GND | at the shifter |
| Bulk | 1000 µF, 16 V | across strip 5 V/GND at the first LED |
| Unused inputs | pins 5, 9, 12 (2A, 3A, 4A) | GND |
| Unused enables | pins 4, 10, 13 (2OE, 3OE, 4OE) | GND |

Do not use the Adafruit BSS138 bidirectional shifter. Its 10 kΩ pull-ups produce roughly 1–2 µs edges, while a WS2812 bit is 1.25 µs and its high pulses are 0.4/0.8 µs.

## Configuration

The production firmware is `wordclock-esp32`. The settings are compiled in for v1:

- `STRIP_LENGTH` in `wordclock-esp32/src/main.rs`: **335** for the bench chain (35-pixel offcut + 300), **300** for the wall strip (288 logical + 12 padding).
- The timezone is the fixed `Europe/Berlin` EU rule in `wordclock-core/src/timezone.rs`.
- The birthday list is `DEFAULT_BIRTHDAYS` in `wordclock-core/src/color.rs`.
- The D20 blank bound is 24 h (`BLANK_AFTER_SECS`); the display is dark until the first SNTP reply and dark again once the last reply is older than that.

## Wi-Fi credentials

Credentials are never committed. On first boot the firmware looks for a stored pair in the flash record at `0x12000`, the free gap between `phy_init` and `ota_0`. If none is present it falls back to a compile-time `wifi.env` pair when the build has one, and otherwise prompts on the USB serial console:

```
wordclock setup
SSID: ...
password: ...
```

The pair is stored in flash and the board reboots into station mode. Later OTA images read it from there, so updating the code does not need the credentials. The release image is credential-free and uses exactly this path.

The build script reads a gitignored `wifi.env` in `wordclock-esp32/`, then `spike/esp32c3-wifi/wifi.env`; an explicit `WORDCLOCK_WIFI_SSID` / `WORDCLOCK_WIFI_PASSWORD` environment variable wins. The same file can optionally pin the address with `WORDCLOCK_OTA_IP` and `WORDCLOCK_OTA_GATEWAY`.

A password change later still needs the USB serial console; there is no setup-AP fallback yet. That is a documented limitation.

## Build and flash

From the repository root:

```bash
cargo build --release --target riscv32imc-unknown-none-elf -p wordclock-esp32
espflash save-image --chip esp32c3 \
  target/riscv32imc-unknown-none-elf/release/wordclock-esp32 \
  /tmp/wordclock.bin
```

First flash, with the 5 V rail **off**:

```bash
espflash flash --monitor --chip esp32c3 \
  --partition-table wordclock-esp32/partitions.csv \
  target/riscv32imc-unknown-none-elf/release/wordclock-esp32
```

The first boot asks for the SSID and password on the USB serial console, stores them in flash, and reboots into station mode. The release image works the same way; it just has no compile-time default.

After that, update over the air. The board serves the update endpoint on port 3232:

```bash
curl -T /tmp/wordclock.bin http://<board-ip>:3232/ota
```

Compare the image against the published SHA-256 before uploading; the endpoint also rejects a file whose first byte is not `0xE9`. The upload streams into the inactive A/B slot, switches the boot pointer, and resets. The status page reports the running slot and its state.

## Status page

```bash
curl http://<board-ip>:3232/
```

The fields that matter:

| Field | Meaning |
| :--- | :--- |
| `revision` | Firmware revision |
| `strip_check` | `on` when the SPI+DMA driver is driving the chain, `failed` otherwise |
| `strip_pixels` | Physical strip length baked into the image |
| `ip` | Current address |
| `nvs_credentials` | `true` when the Wi-Fi pair came from the flash record |
| `synced`, `sync_age_s` | Whether SNTP has ever answered, and how old the last reply is |
| `running`, `slot_state` | Active OTA slot and whether it is valid |

The status page is unreachable while the network is down; that is expected.

## Logging

The firmware keeps the most recent log lines in a 4 KiB ring in RAM and serves them on the same port:

```bash
curl http://<board-ip>:3232/log
```

Each line starts with the uptime in seconds, then the level and the message, so an event can be placed relative to a boot or an outage without a serial console. The ring captures the same lines that go to USB serial, including the Wi-Fi and network stack messages, because both paths go through the `log` facade. `ESP_LOG` still selects the level at build time.

A sample from revision 33:

```text
    13 INFO running from Ota1; OTA ready
    13 INFO Connecting to Wi-Fi
    14 INFO Wi-Fi connected: ConnectedInfo { ssid: "...", channel: 1, authmode: Wpa2Personal, aid: 0 }
    24 INFO SNTP synchronized: unix=1793730962
```

The ring drops the oldest whole lines when it fills, and it is volatile: a reset or a power cut clears it. That is deliberate, because a flash-backed log would cost a sector and wear for information the counters already summarise. Fetch `/log` before cutting power, and use `/` for the counters that survive a network drop.

## Test patterns and physical mapping

The diagnostic patterns live in the spike, `spike/esp32c3-ota`, not in the production firmware. Use them before trusting identity mapping:

- `--features strip-check` walks one dim pixel down the chain.
- `--features boundary-check` alternates the frame's own `0xee0b01` with black; a clean chain is one lit, one dark to the end.
- `--features identity-check` holds red, green, blue, yellow, white, black at the start of the chain.

Record the physical order, orientation, and whether the extra 12 LEDs beyond 288 are connected. If the order is not identity, a mapping is needed before the clock can be trusted.

## Power-loss behaviour

There is no orderly shutdown. On power loss the strip keeps its last frame until power returns, and the firmware cannot write a dim fill first. After boot the display is dark until the first SNTP reply, so a stale time is never shown. This is D20 and is a documented limitation, not a fault.

## Rollback

1. **Previous OTA image.** The A/B slots keep the other image; re-upload the last good `/tmp/wordclock.bin`, or use the bootloader's rollback.
2. **USB reflash.** Power the board from USB alone, with the 5 V rail off, and flash the known-good image.
3. **Legacy clock.** The Pi 3B and Fadecandy board stay available until the ESP32-C3 has run for the agreed stability window. Reconnect the strip to the Fadecandy channel 0 output and boot the old SD card.

Do not retire the Pi, the Fadecandy board, or the old SD card before that window has passed.
