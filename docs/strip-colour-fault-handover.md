# Handover: WS2812 colour fault on the ESP32-C3 strip

This supersedes the earlier state in `HANDOFF.md` for the strip-transport question only. It exists because the colour fault was chased for several rounds along a wrong theory, and the next model should not repeat that.

## Summary

The clock core renders correctly and the encoder's byte layout is proven correct by a static readback test. The remaining fault is a **single LED bit slipping across a pixel boundary**: when a pixel's last LED bit is `1`, the next pixel's first bit is read as `1` as well. That one rule explains every wrong colour reported so far, including the original "simulator red, strip not red" complaint. A static readback test isolated it; the moving-pixel tests had obscured it.

## Verified state

Board: ESP32-C3-DevKit-RUST-1, MAC `34:85:18:02:70:14`, IP `192.168.10.244:3232`, currently on `Ota0` at revision 19. Strip is 335 pixels (35-pixel offcut + 300 added), driven on GPIO3.

- **The frame data is uniform and correct.** `GET /frame` on the device for the 06:30 fixture (`15.07.2026 06:30`, seed 42) is byte-identical to `examples/dump_frame.rs`: 290 lines, `diff` clean, 15 lit pixels at `0,1,3,4,5` and `140..=149`, all raw `f8481c` → gamma-corrected `ee0b01`.
- **The simulator is the trustworthy side.** `ColorPicker::choose` caches one colour per minute on regular days (`regular_day_caches_one_color_per_minute`). The only path that resamples per pixel is a birthday, and `DEFAULT_BIRTHDAYS` is `(1,13) (3,10) (4,8) (7,18) (12,31)` — 15 July is not among them. A uniform frame is therefore intended.
- **Byte order and channel values are correct on the wire.** A static five-pixel identity test (revision 17) at offset 0 returned exactly `red, green, blue, yellow, white` for pixels 0–4. So GRB byte order, per-channel values, the 3-bit SPI mapping, the SN74AHCT125N level shifter, and the current budget are all fine.
- **The stray pixel follows the data, not the LED.** At offset 0 the run was pixels 0–4 and a stray green appeared at pixel 5. At offset 1 the run was pixels 1–5 and the stray moved to pixel 6. The pattern, not the hardware position, determines it.

## The fault, characterised

The identity run is `red, green, blue, yellow, white`, then black. Only white ends in a `1` LED bit:

| Pixel | Commanded | Wire bytes `(g,r,b)` | Last LED bit |
| --- | --- | --- | --- |
| 0 | red | `00 ee 00` | `0` |
| 1 | green | `ee 00 00` | `0` |
| 2 | blue | `00 00 ee` | `0` |
| 3 | yellow | `00 ee ee` | `0` |
| 4 | white | `ff ff ff` | **`1`** |
| 5 | black | `00 00 00` | `0` |

The stray lands on pixel 5, whose first LED bit should be `0` but reads as `1` — and the reported colour is green, i.e. `G = 0x80`, which is exactly the green MSB and nothing else.

That single rule reproduces all the earlier reports:

- The fixture frame lights `0,1,3,4,5`, and all of them are `ee0b01` with `B = 0x01`, so every one of them ends in a `1` bit. Strays land on `1,2,4,5,6`; the ones on already-lit pixels add green to red (visually **yellow**), the ones on dark pixels show as **green**. Visible result: `0..=6` lit, which is the user's original "first seven LEDs lit", with "3 green" and "7 green" being the two strays and "5 yellow" being a stray added to a red pixel.
- The dense walk lights `ee0b01`, whose last bit is `1`, so every step produces one stray green immediately behind the red walker.
- After a one-bit slip everything downstream shifts by one bit, but the frame is black there, so the slip is invisible apart from that one pixel.

The mechanism is not yet proven. The leading hypothesis is that the low time between the last bit of a pixel and the next pixel's leading `0` is too short: at 2.5 MHz SPI the encoding `0 → 100` and `1 → 110` gives `T1L = 0.4 µs`, at the tight end of the WS2812B tolerance.

## What is ruled out

Do not re-open these; each cost a round trip.

- **Power and current.** The stray reproduces with a single lit pixel at roughly 355 mA, well inside the 500 mA budget. Sag on a single-ended feed cannot cause it.
- **The colour model, the gamma LUT, the picker and the simulator.** Proven by the byte-identical dump and the correct static readback.
- **Byte order / channel order (GRB vs RGB).** Proven correct by the identity test. An earlier claim in this session that the fill test confirmed it was wrong: the fill only proved per-pixel uniformity, not hue.
- **A faulty LED.** The stray moved when the pattern moved.
- **The level shifter and wiring.** Five independent colours came out exactly right through it.

## Experimental history

This is the trail, not just the conclusions. It is here because four of the six strip builds in this session were badly designed and the next model should see why rather than rediscover it.

### Table of builds

| Revision | Feature | What it was for | Outcome |
| --- | --- | --- | --- |
| 13 | `frame-check` | Serve the fixture frame, diff against the host | Passed; not a wire test |
| 14 | `fill-check` | Uniform fill, one channel at a time, level 6 | Flat, and uninformative |
| 15 | `dense-check` | Walk `ee0b01`, green marker every tenth | Two pixels lit; misread |
| 16 | `dense-check`, no marker | Remove the green-by-design confounder | Same pair, now ambiguous |
| 17 | `identity-check`, offset 0 | Static five-colour readback | Decisive; found the stray |
| 18 | `identity-check`, offset 1 | Does the stray follow data or position | Decisive; follows data |
| 19 | `identity-check`, SPI 2.222 MHz | Timing-margin hypothesis | **Never observed** |

### Phase 1: reading the code instead of the strip

First move was to compare `GET /frame` with `examples/dump_frame.rs`. They were byte-identical, and checking `ColorPicker::choose` plus `DEFAULT_BIRTHDAYS` showed a uniform frame was intended. That correctly ruled out the renderer and the simulator, and it was the right thing to do first because it cost nothing. It proved nothing about the wire, which is where the fault actually was, so the work did not advance the diagnosis — it only narrowed it.

### Phase 2: the fill test, which was worthless as designed

The reasoning was: if the whole chain renders one known command uniformly, the transport is fine. The first draft used level 48 and white at 32, which by the project's own current model is roughly 1.3 to 1.7 A. The human stopped it with the 500 mA limit before it was flashed, and the level was cut to 6 to fit at about 493 mA.

Result: "The whole chain _is_ one flat colour, exactly as specified."

I concluded the transport was healthy. That was wrong, and the error was in the design, not the measurement. Level 6 is `06` = `00000110` — one set bit in eight, the sparse end of the payload space. The failing frame is `ee0b01`, eight set bits in the green byte. A transport that mangles dense runs of ones passes a sparse fill trivially. The current budget forced the level down and the low level silently destroyed the test's power to discriminate.

The second error in this build: it tested **uniformity**, and I reported it as confirming **hue**. The human later pointed out the distinction, and the handover should keep it explicit. Uniformity says every pixel agrees; it says nothing about whether they agree on the right colour.

### Phase 3: dense walk with markers

To actually exercise the dense payload, the walk was changed to carry `ee0b01` at 400 ms per step with a green marker every tenth pixel. Reported back: "At any point, there are two pixels lit - one red and one green. Every tenth is a single green."

A clarifying question was asked and answered "red and green are adjacent (an off-by-one), so two neighbouring pixels are lit and they move together". That answer was correct and, in hindsight, was already the whole diagnosis.

### Phase 4: dense walk without markers, and the misreading

Sometimes the marker made a stray green indistinguishable from design, so the marker was removed and every step was the same colour. Reported back: "It's the same as before, withput the marker. 1. First pixel becomes green 2. the second becomes green and the first one red 3. Third green, second red, first off".

**This is where the diagnosis went wrong.** I read that as "the current pixel is green and the previous is red" and built a theory on it. The correct reading is the one the human was actually giving: the walker is red, and the pixel *ahead* of it is green. The clue that should have caught it immediately is that the walk payload renders **red**, so a green pixel cannot be the walker's own data — it can only be a stray or a leftover marker. "Current pixel green" was an impossible observation and I rationalised it instead of questioning it.

### Phase 5: the byte-slide theory, and why it was wrong

From the misreading I argued that `[g=0b, r=ee, b=01]` re-read one byte later yields green, then red, then off, which matches the reported three-step sequence. That led to a confident claim that the channel order was inverted and that the simulator/strip divergence was explained, and to a false statement that the fill test had confirmed GRB. Both were wrong.

It felt plausible for two real reasons, which is worth recording because the same trap will appear again: the three-step colour sequence does match a one-byte slide exactly, and the human's much earlier "medium orange, then dark orange, then pink-ish" is genuinely what a red/green swap does to xkcd colours. Both observations were true; the inference from them was not. A static test contradicted it within one build, and it could never have survived the fact that the first five identity colours came out perfect.

### Phase 6 and 7: the static identity test, which worked first time

The change in method that worked: hold **position** and **payload** constant, show several independent known colours at once, and ask for one readback. No motion, no markers, no timing between frames.

- Offset 0: "red, green, blue, yellow, white, green and then all black" — five correct colours, one stray immediately after the run.
- Offset 1: "pixel 0 black, 1 red, 2 green, 3 blue, 4 yellow, 5 white, 6 green, 7 onwards black" — the stray moved with the pattern, so the LED is fine.

Two flashes produced more certainty than the preceding four, because the test was designed to *distinguish hypotheses* rather than to pass.

### Phase 8: deriving the rule

With the readback in hand, the rule falls out: only white ends in a `1` LED bit, and only the pixel after it is wrong. Every colour in the walk and in the fixture frame is `ee0b01`, whose blue byte is `01`, so every lit pixel ends in a `1` and every one of them produces a stray. That retro-explained the "first seven LEDs lit", the green strays at 3 and 7, and the yellow at 5 in the original report.

### Phase 9: the clock reduction, never read back

Revision 19 lowered the SPI clock from 2.5 MHz to 2.222 MHz to widen the suspect low time from 0.4 µs to 0.45 µs. It flashed and booted, confirmed as `revision=19` on `Ota0`, and then the session moved on without the strip ever being observed again. **The result does not exist.** Do not assume it passed or failed.

### Coordination mistakes worth recording

The human offered a beefier bench supply specifically to lift the current ceiling, and the experiment never used it — the fill stayed at level 6 and the brighter variant was never built. Three separate times a test result was reported back to me and I used it to confirm what I already believed rather than to falsify it.

The pattern across the whole session: tests were designed around what was convenient — a power-safe level, a moving pixel, a uniform payload — rather than around what could separate the competing hypotheses. The static identity test was the first build designed to be decisive, and it was the only one that changed anything.

### If starting over

1. Read back the intended frame. One command, and it rules out half the system.
2. Immediately run a static readback containing a colour whose last LED bit is `1` next to one whose last bit is `0`, plus three independent primaries. That is the identity test, and it finds the rule in one flash.
3. Drive the strip with `ws2812-spi`, which the project already depends on, rather than a hand-rolled encoder. Do not reproduce the bit patterns from memory.

## Reasoning errors made in this session (do not repeat)

1. Treating a uniform flat-colour fill as proof that the transport was healthy. The payload was level 6, nearly all zero bits, and could not exercise a dense payload.
2. Treating a uniformity result as a confirmation of hue.
3. Inferring a byte-level window slide from a *moving* pixel, built on a backwards reading of a correct description.
4. Accepting "the current pixel is green" as an observation when the payload renders red, and therefore could not have produced it.
5. Probing paths outside the project instead of asking for access.

## Immediate next steps, in order

1. **Read back revision 19, then record the result here.** It is the same static identity frame with the SPI clock lowered to `STRIP_KHZ = 2_222` (from 2.5 MHz), which makes `T1L` 0.45 µs and `T1H` 0.9 µs. If the stray at pixel 6 disappears, the fault is timing margin and the clock is the fix. If it persists unchanged, the clock is not the lever. **This has not been done, so the state is unknown.**
2. **Confirm whether `ws2812-spi` should be used instead of the hand-rolled encoder.** The host crate already depends on `ws2812-spi = { version = "0.5", features = ["std"], optional = true }` under its `hardware` feature, and there is *no* Pi backend in the tree — `src/display.rs` mentions `rppal` and `ws2812-spi` only in a doc comment. The intended direction is presumably to drive the ESP32-C3 SPI bus through the crate (`embedded_hal::spi::SpiBus` on `SpiDma`) and delete `encode`/`set_bit` from `spike/esp32c3-ota/src/bin/main.rs`. **Ask the human before asserting the crate's encoding details** — do not guess the bits-per-bit or the required SPI frequency. Read the crate source, which lives in the cargo registry outside the project.
3. **If timing is not the lever and the library is not adopted,** the definitive instrument is a wire capture: GPIO3 into the Pico at 24 MS/s, decoded with the sigrok `rgb_led_ws281x` decoder, compared against `GET /frame`. That settles whether the wire carries the stray bit or the strip invents it. The recipe is in the sibling `sigrok-tutorial` repo (`content/appendix/cli-cookbook.md`, WS2812 capture at `samplerate=24000000 --channels D2 --samples 380000`).

## Repository state

HEAD is `7873ad4 Add strip payload diagnostics for the fill and dense cases`, which contains `fill-check` and `dense-check`. Everything after that is **uncommitted**: the `identity-check` feature and `IDENTITY_OFFSET`, revisions 16 through 19, the `STRIP_KHZ` clock change, the consolidation of the walk constants into a `walk_pattern` module, and this document. The tree is otherwise clean.

Also uncommitted and worth keeping: the default no-feature build was broken before this session — `walk` was gated without requiring `strip-check`, so it referenced `Strip` and `STRIP_LENGTH` where they did not exist. That gate and two unused imports are now correct, and every feature combination builds warning-free.

`openspec validate --strict --changes` passes for both changes at the time of writing (2 passed, 0 failed). Run it with `--changes`; a bare `openspec validate` reports "Nothing to validate".

## Firmware build and flash

Run cargo from inside `spike/esp32c3-ota`, not with `--manifest-path` from the root; the latter bypasses `.cargo/config.toml` and breaks `portable-atomic`/`build-std`.

```bash
cd spike/esp32c3-ota
cargo build --release --features identity-check
espflash save-image --chip esp32c3 target/riscv32imc-unknown-none-elf/release/esp32c3-ota /tmp/identity.bin
curl -T /tmp/identity.bin http://192.168.10.244:3232/ota
curl http://192.168.10.244:3232/          # status: revision, running slot
```

Do not pass `--merge` or `--skip-padding` to `espflash save-image`; pass `--partition-table partitions.csv` only on the first USB flash.

Diagnostic features in `spike/esp32c3-ota/Cargo.toml`, all implying `strip-check`, all building warning-free:

| Feature | Behaviour |
| --- | --- |
| `strip-check` | Walks one dim white pixel, every tenth green |
| `dense-check` | Same walk carrying `ee0b01`, no marker, 400 ms per step |
| `fill-check` | Whole chain, one channel at a time, level 6 (~493 mA) |
| `identity-check` | Static `red, green, blue, yellow, white` run at `IDENTITY_OFFSET`, held still |
| `frame-check` | Renders the 06:30 fixture, holds it, serves `GET /frame` |

`identity-check` is the one that answered the question. Keep it.

## Hardware and power notes

- Single 5 V supply into the pin **labelled `5V`** (header position 19), never `3V3`, and never USB plus 5 V at the same time. 1000 µF + 100 nF at the pin.
- Data path: GPIO3 → SN74AHCT125N pin 2 (1A) → pin 3 (1Y) → 470 Ω → DIN; 10 kΩ from pin 2 to GND; shifter VCC 5 V; common ground.
- Current model used throughout, from `openspec/changes/add-power-budget-and-current-sensing/design.md`: `I_mA = pixels × 1 mA + Σ(r+g+b) × 20/255 mA`. At 335 pixels the quiescent term is 335 mA and dominates. By this model the fixture frame itself draws about 630 mA — the 500 mA bench limit is already exceeded by the failing frame, which the human should decide about.

## Pitfalls that cost time

- The board is frequently powered off between messages. Retry `curl` rather than concluding a flash failed.
- The OTA server handles one connection at a time. Back-to-back `curl` calls can return an empty body; leave a pause or retry.
- A WS2812 keeps whatever it latched at power-up. Always write the full strip length and make the first frames black and repeated three times.
- RMT is a dead end on this chip for a 335-pixel frame; the transport is SPI+DMA. Do not revisit.
