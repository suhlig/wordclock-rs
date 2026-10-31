# ESP32-C3 OTA spike

Answers the question "single 5 V supply, so how do I update the firmware?" with a working prototype. The device joins Wi-Fi and serves one tiny HTTP endpoint on port 3232 that writes a new application image into the inactive OTA slot; `curl` is the uploader.

The point is to prove the workflow on the bench before the real firmware depends on it: once this works, the clock never needs USB in place, only the 5 V rail.

## Power

- 5 V goes into the pin **labelled `5V`** on the board's silkscreen, header position 19 in the 28-pin numbering. It is the VBUS net and the only supported 5 V input; boards with other silkscreens call the same pin `VIN`.
- That pin has **no GPIO number**. `GPIO19` is a different pin — USB D+, header position 25 — and must never see 5 V. Never feed `3V3` either; it is an output.
- 100 µF plus 100 nF sit at the board. This build was run with **1000 µF + 100 nF**, which is also fine; the over-the-air upload came back at 15.5 s against 35–39 s without any bulk capacitance (see the timing section).
- **Never connect USB while the 5 V rail is powered**, and never leave USB connected with the rail off. VBUS and `5V` are the same node.
- 100 µF plus 100 nF from `5V` to `GND` at the board, and tap the rail near the supply. The C3's Wi-Fi transmit peak is about 335 mA and should not have to travel through the LED distribution.

See design decision D9 in `openspec/changes/add-power-budget-and-current-sensing/design.md`.

## First flash, over USB

Unplug the 5 V rail for this. The partition table must have two OTA slots, so pass `partitions.csv` explicitly:

```bash
cargo build --release
espflash flash --chip esp32c3 --partition-table partitions.csv \
  --port /dev/cu.usbmodem101 \
  target/riscv32imc-unknown-none-elf/release/esp32c3-ota
```

`/dev/cu.usbmodem101` is this board on the current machine; `ls /dev/cu.usb*` shows what is there. This is the only step that needs USB.

If the board previously ran a different partition layout, the new `otadata` partition overlaps whatever used to live at `0x10000`, and those stale bytes make the whole partition fail its CRC check. The firmware handles that by scrubbing `otadata` and re-selecting the running slot at boot (see the log line `otadata is unusable; scrubbing both slots`), so no extra step is needed. `espflash erase-flash` before the first flash does the same thing from the outside and is the cleaner option when the board holds nothing you want to keep.

## Credentials and address

`wifi.env` is looked up in this directory first and then in `../esp32c3-wifi`, so the credentials from the Wi-Fi spike are reused without a copy.

`ota.env` is optional and pins the address:

```bash
cp ota.env.example ota.env
$EDITOR ota.env
```

A static address is what makes the USB-free workflow practical, because otherwise you have to look the DHCP lease up in the router before every update. Without `ota.env` the firmware uses DHCP and prints the address once at boot over serial.

## Build the payload

```bash
espflash save-image --chip esp32c3 \
  target/riscv32imc-unknown-none-elf/release/esp32c3-ota /tmp/wordclock-ota.bin
```

That writes the application image itself, starting with the `e9` magic byte. **Do not pass `--merge`**: the merged form contains the bootloader and the partition table, which must not be written into an application slot. `--skip-padding` is only accepted together with `--merge` in espflash 4.7, so leave both off; the resulting image is not padded (522,848 bytes at the time of writing, against a 1,966,080-byte slot).

`espflash` has no OTA client of its own. Its commands are `flash`, `save-image`, `write-bin`, and `read-flash`, and none of them speaks to a running device, which is why this spike uses HTTP.

## Upload

```bash
curl -v -T /tmp/wordclock-ota.bin http://192.168.1.50:3232/ota
```

`-T` sends a `PUT` with a `Content-Length`, which is exactly what the endpoint expects. The device answers, switches the boot pointer to the new slot, marks it `New`, and resets. If curl offers `Expect: 100-continue`, the server answers it before reading the body, so there is no stall.

## Verify without USB

Bump `REVISION` in `src/bin/main.rs` before building the payload. The on-board WS2812 on GPIO2 blinks `REVISION` times at boot and then shows the status colour, so the number of blinks tells you which image is running without any console.

The status endpoint answers the same question over the network:

```bash
curl http://192.168.1.50:3232/
```

```text
revision=2
uptime_s=33
heap_used=47704 heap_max=52976 heap_size=103184
running=Ota1
table=entries=5 apps=2 ota_data=1 labels: nvs(0x9000,0x6000) otadata(0xf000,0x2000) phy_init(0x11000,0x1000) ota_0(0x20000,0x1e0000) ota_1(0x200000,0x1e0000)
selected=Ota1
slot_state=Valid
```

`running` is read from the MMU mapping and names the slot the running image was booted from, which is the honest answer to "did the update take effect?". `selected` is what otadata points at and can differ from it if an image failed to boot. `table` repeats the flashed layout using the same counting rules the OTA layer uses, so a wrong `partitions.csv` is visible rather than mysterious.

## Verified on the bench

First round on 2026-10-31 with the board powered over USB; second round with USB **unplugged** and the board running from the `5V` pin:

| Step | Result |
| :--- | :--- |
| First flash over USB, revision 1 | `running=Ota0`, `selected=Ota0`, `slot_state=Valid` after the boot scrub |
| `curl -T` revision 1 | `200 OK`, bootloader log `Loaded app from partition at offset 0x200000`, `running=Ota1` |
| `curl -T` revision 1 again | Back to `running=Ota0`, so A/B switching works both ways |
| `curl -T` revision 2 | `revision=2`, `running=Ota1`, `confirmed the running image: New -> Valid` |
| `curl -T` revision 2 again | `revision=2`, `running=Ota0` |
| **USB unplugged, powered from `5V`**, `curl -T` revision 3 | `revision before the update: 2`, then `revision=3`, `running=Ota1`, 35.6 s |
| `curl -T` revision 3 again | `revision before the update: 3`, still `revision=3`, `running=Ota0`, 39.4 s |
| `curl -T` revision 4 with `--features strip-check`, 1000 µF added | `strip_check=on`, `running=Ota1`, 15.5 s; the first five pixels cycled red → green → blue → dim white → pixel 0 only → off, confirmed by eye |

The last row is the full single-rail proof: the strip is driven from GPIO3 through the level shifter from the same 5 V rail as the controller, the update arrived over Wi-Fi, and no USB cable was attached at any point.

The last two rows are the point of the exercise: the firmware went from revision 2 to revision 3 and back across both slots with no USB cable attached and no serial console, and the status endpoint was the only way to see the device.

### Upload time differs between the two power sources

| Power source | Upload time for 522,848 bytes |
| :--- | :--- |
| USB | 10.8 s (about 48 KB/s) |
| `5V` pin | 35.6 s and 39.4 s (about 14 KB/s) |
| `5V` pin with 1000 µF + 100 nF at the board | 15.5 s (about 34 KB/s), one sample |

All three succeed, so this is a throughput observation, not a fault. The fixed cost is the flash path: 128 pages of 4 KiB, each erasing a sector with interrupts masked. The bulk capacitor appears to matter: the rail without it was consistently three times slower, which points at supply noise on the radio rather than at retransmission timing. One sample is not proof, so treat the last row as a hint and re-measure before relying on it.

## What the device does with a request

| Request | Behaviour |
| :--- | :--- |
| `GET /` | Status document: revision, uptime, heap, running slot, table summary, boot selection, slot state |
| `PUT /ota`, `POST /ota` | Streams the body into the inactive slot, then activates it and resets |
| Anything else | `404` with a hint |

The body is buffered into 4 KiB pages; `esp_storage` erases the affected sector before each write, so no explicit erase is needed. Writes run inside `esp-storage`'s critical section, which masks interrupts for the duration of each ROM call, because code cannot be fetched from flash while the cache is off. A 523 KB image is about 128 page writes and takes roughly 11 seconds.

Before activating the new slot the first byte is read back and checked against the `0xE9` magic byte, so uploading the wrong file fails with a `500` and leaves the boot pointer alone.

On boot the firmware reads `otadata` before Wi-Fi starts and repairs it if it is unreadable, then selects the slot it actually booted from. This is what makes the first update work on a board that was flashed with a different partition layout before; without it the library rejects the whole partition with `Error::Invalid`.

## Strip check

`--features strip-check` adds a second RMT channel on GPIO3 (through the level shifter, same wiring as the offcut work) and cycles the first five pixels through red, green, blue, dim white, a single-pixel index check, and off, 500 ms per step. The OTA server keeps running beside it, so the image can be replaced over the air like any other.

```bash
cargo build --release --features strip-check
espflash save-image --chip esp32c3 \
  target/riscv32imc-unknown-none-elf/release/esp32c3-ota /tmp/wordclock-strip.bin
curl -T /tmp/wordclock-strip.bin http://192.168.1.50:3232/ota
```

`GET /` reports `strip_check=on` when the driver was created, `failed` when the RMT channel could not be allocated, and `off` for an image built without the feature. The check is deliberately non-fatal: the only way back into the board is over the air, so a missing channel is logged and the server keeps serving.

Five pixels at these levels draw at most about 75 mA, well inside the supply. Both drivers ask for a single RMT memory block, which is what keeps the second channel usable; asking for two would take the whole RMT RAM and the on-board indicator would stop working.

### Write the whole strip, every frame

The first version of this check wrote only five pixels, and on roughly every third power-up the remaining thirty came up **solid red** and stayed that way. That is not a bug in the pattern. A WS2812 is a shift-register chain: pixels beyond the transmitted length receive no data and keep whatever they latched while the rail came up. On this build the 5 V ramp is slow (1000 µF), and the shifter's input floats until the firmware configures GPIO3; a floating CMOS input can sit high, and a WS2812 reading a line that is high and then goes low shifts `0xFF0000` into every pixel — red, all the way down the chain.

Two changes, one firmware and one wiring:

- Every frame now covers all 35 pixels, with the ones the pattern does not use set to black, and the strip is cleared once at boot. Any junk is overwritten within a second instead of surviving until the next full frame happens to touch it. Verified over five power cycles on revision 6: two of them flashed the whole strip at power-up (once red, once green) and in **every** case the unused pixels went dark as soon as the first frame arrived.
- A **10 kΩ pull-down from the shifter's input (pin 2, 1A) to ground** keeps DIN low through the whole power-up, so the junk is never latched in the first place. This is the cure; the full-length frame only hides the symptom quickly. It is reasoning from the cause and not yet measured: the two-of-five run was made without the resistor. `docs/hardware.md` has it in the connection list and the bill of materials.

The two different flash colours are what identify the cause. In GRB order a red pixel is `0x00FF00` and a green one is `0xFF0000`, so the same high-then-low burst on the data line lands on different byte boundaries on different power-ups. Firmware would send the same colour every time; noise does not.

The general rule for the clock: never send a short frame. With 300 pixels, a prefix frame leaves the tail at whatever it was, and the tail is most of the clock.

Verified on the bench: revision 4 ran this pattern while the board was powered from the `5V` pin with USB unplugged, and the cycle above was confirmed by eye.

## LED status

| Colour | Meaning |
| :--- | :--- |
| Blue | Booted, before Wi-Fi |
| Amber | Connecting |
| Green | Ready, waiting for a request |
| Amber to green | Upload progress |
| Red | The last request failed |

## Limits and things to know

- **Plain HTTP on the local network, no authentication.** Anyone who can reach port 3232 can replace the firmware. Do not expose it to the internet; if that ever becomes a requirement, this design needs a token at minimum and TLS at best.
- **One request at a time.** The socket is reopened per connection, so a second client waits.
- **Wi-Fi may stutter during an upload.** Each erase and write masks interrupts for tens of milliseconds.
- **Upload time varies with the power source.** 10.8 s over USB power and 35–39 s on the `5 V` rail for the same image; see the timing section above.
- **A power cut mid-upload is safe.** The boot pointer still refers to the running image until the very end, so the device comes up on the old firmware; the half-written slot is simply overwritten by the next upload.
- **Rollback is best-effort.** The image marks itself `Valid` once Wi-Fi is up, which confirms it when the bootloader has rollback enabled and is a harmless no-op when it does not.
- **Recovery is over USB.** If a new image is broken badly enough that Wi-Fi never comes up, unplug the rail and flash over USB again.
- **Image size**: about 523 KB against a 1.9 MB slot, so there is room to grow.

## If the Pi is selected instead

Delete this directory. It exists to prove that the single-supply, OTA-only workflow is practical on the C3, which is the reason the C3 can win design D16 without giving up a service path.
