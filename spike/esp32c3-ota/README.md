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

`--features strip-check` drives the strip and **walks one dim pixel down the whole chain**, 300 ms per step, looping. Every tenth pixel is lit green instead of white, so a failure can be counted to rather than guessed at. The chain is 335 pixels — the 35-pixel offcut plus the 300 added to it — so one sweep takes about 100 s; `STRIP_LENGTH` is the constant to change when that changes. The OTA server keeps running beside it, so the image can be replaced over the air like any other.

```bash
cargo build --release --features strip-check
espflash save-image --chip esp32c3 \
  target/riscv32imc-unknown-none-elf/release/esp32c3-ota /tmp/wordclock-strip.bin
curl -T /tmp/wordclock-strip.bin http://192.168.1.50:3232/ota
```

`GET /` reports `strip_check=on` when the driver was created, `failed` when the SPI+DMA driver could not be set up, and `off` for an image built without the feature, plus `strip_pixels` for the chain length baked into the image. The check is deliberately non-fatal: the only way back into the board is over the air, so a missing driver is logged and the server keeps serving.

One lit pixel at these levels draws a few milliamps, so a whole sweep stays inside the supply's noise floor. The strip is driven over SPI2 with DMA (GPIO3 routed as MOSI), which leaves the RMT peripheral entirely to the on-board status LED. The encoded bitstream is about 4 KiB and lives in a static, as does the frame being walked.

Revision 20 carried a second cell layout and a `GET /strip?mapping=3bit|4bit` switch so the fault and the fix could be compared in one session, on one power state and one temperature. That comparison was made, the four-bit cell removed the fault, and the switch has been deleted; what remains is the single waveform in `src/strip.rs`.

`--features boundary-check` is the pattern that checks the fix across the whole chain rather than the first few pixels: every second pixel carries the frame's own `0xee0b01`, whose blue byte is `0x01` — a `0` followed by a `1`, the shape that produced the stray — and the pixels in between stay black. A uniform fill is the wrong instrument, because every pixel would receive the same `+0x80` green and the strip would only look uniformly greener; with a dark pixel after each lit one, a stray shows up as a lit pixel. The clean readback is one lit, one dark, repeating to the end; the fault read as a red/green checkerboard, 168 times over.

It draws roughly 2 A, scaled from 578 mA measured on the dim variant of the same pattern. For a smaller supply, `BOUNDARY_COLOUR` in `src/bin/main.rs` documents dropping to `0x200801`, which keeps the `0` then `1` ending at about a sixth of the drive.

### Why the strip runs on SPI, not RMT

The C3 gives an RMT channel 48 codes of memory against a frame of 8,042, so the driver has to refill that block from the CPU every 30 µs. In blocking mode the refill happens in a busy-poll loop, and any interrupt that runs past the deadline makes the hardware wrap the block and send it again: the rest of the frame shifts by two pixels per wrap. Measured on a 335-pixel walk, one step's lit pixel appeared about **36 pixels further down the chain** — 18 wraps, roughly 1.1 ms of Wi-Fi interrupt time — and the walk carried on normally afterwards, because only the single lit word made the shift visible.

Writing each frame twice hid it rather than fixing it: the stray pixel became a brief flicker elsewhere in the chain. The fix is to take the CPU out of the timing path. The strip runs on **SPI2 with DMA at 2.5 MHz, four SPI bits per LED bit**: `0` is `1000` (0.4 µs high, 1.2 µs low) and `1` is `1100` (0.8 µs high, 0.8 µs low). The clock pin is unused, GPIO3 stays the data pin as MOSI, and DMA streams the 4 KiB bitstream out of memory while the CPU services Wi-Fi. With that, a full 335-pixel walk runs with no glitches at all.

The first SPI build used **three** SPI bits per LED bit, which ties the gap after a `1` to one clock — 400 ns at 2.5 MHz, and it cannot exceed 475 ns at any clock that keeps `T1H` inside its window. That gap is where a pixel ending in `1` produced a stray pixel behind it, confirmed by switching between the two cells on one build. Four bits decouple the pulse from the gap. The clock, the cell and the reset live in `src/strip.rs`, and that module's host tests derive `T0H`, `T0L`, `T1H`, `T1L` and the reset from them and check them against the WS2812B datasheet; do not reintroduce a tuned clock constant here.

Two consequences worth keeping:

- The earlier RMT measurement on the 288-pixel gate frame was clean because that build ran with `frame-loop`, which skips Wi-Fi. RMT is fine for a strip nobody interrupts; it is not fine for a clock that shares the CPU with a radio.
- The real firmware should use the same transport, and the display backend should treat "CPU in the timing path" as a design smell.

### Write the whole strip, every frame

An early version wrote only five of the 35 pixels, and on roughly every third power-up the rest came up **solid red** and stayed that way. A WS2812 is a shift-register chain: pixels beyond the transmitted length keep whatever they latched when the rail came up. Two power-ups in five flashed the whole strip, red or green; adding a 10 kΩ pull-down to the shifter's input changed that to five in ten on only the first 19-20 pixels; and once the first frame moved from about 2.4 s after reset to about 0.2 s, ten of ten were clean.

The colours are what identified the cause. In GRB order red is `0x00FF00` and green is `0xFF0000`, so the same high-then-low burst on the data line lands on different byte boundaries on different power-ups. Firmware sends the same colour every time; noise does not.

What the investigation settled, in the order the pieces matter:

1. **Write the first frame as soon as the driver exists**, not after status blinking. A 2.4 s gap put the strip's power-up latch on display for seconds.
2. **Cover every pixel in every frame.** A short frame cannot clear a tail it never addresses, and on the real strip the tail is most of the clock.
3. **Send the first frame two or three times.** The far end of the chain can still be finishing its own power-on reset when the first frame lands; a single early frame left the last pixel dark on three of ten power-ups. The clock redraws once a minute, so a missed first frame would stay wrong for a minute.
4. **Keep the 10 kΩ pull-down on the shifter, pin 2 (1A).** An undriven CMOS input is undefined on its own terms. The one combination never measured is an early first frame *without* it, so it is kept on principle rather than because its absence was shown to break the early-frame case.

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
