# Design

Decision numbers here are local to this change; `replace-fadecandy-with-rust` uses its own D-series.

## Context

The build is a 300-pixel WS2812B strip (288 logical plus 12 padding) on a 60 W, 5 V supply, which is 12 A continuous. At the datasheet's 60 mA per lit pixel an all-white frame commands about 18 A; a normal clock frame lights 11–39 pixels, roughly 0.7–2.3 A plus about 0.3 A of quiescent current across 300 pixels. So the exposure is not steady state but any frame that lights the strip.

What already exists: the strip is fed through per-row rails in 1.5 mm² solid copper, and an ACS712-20A Hall sensor is installed in the LED branch, so it measures the strip and not the controller. The host-independent core already ends in a corrected `Frame` and a `Display` trait, and the ESP32-C3 has ADC1 on GPIO0–GPIO4 with GPIO3 now carrying the strip data.

Supplier reality shaped one decision: an INA226, INA260 or INA228 could not be bought from the hobbyist shops checked (BerryBase, Reichelt, Conrad, Pollin, Botland), and Mouser and TME blocked automated checks. The common INA219 breakout carries a 0.1 Ω shunt rated 3.2 A, which dissipates about 14 W at 12 A and is unusable here.

## Goals / Non-Goals

**Goals:**

- The commanded current never exceeds a configured limit, for any frame including the all-white test pattern.
- Normal operation is unchanged: below the limit the frame is byte-identical to what the color pipeline produced.
- The actual current is observable and can calibrate the estimate.
- The behaviour lives in the host-independent core, so both host paths inherit it.

**Non-Goals:**

- Replacing the supply's over-current protection or a fuse. This is an open-loop bound, not a safety device.
- Precision metering. The sensor's 1.5 % error and the ADC's noise are accepted.
- Per-row current sensing. One sensor in the main rail measures the quantity the limit is about.
- Any user-facing feature, display mode, or configuration surface beyond the limit and its calibration.

## Decisions

### D1: Bound the commanded current at 9 A

The configured limit is 9 000 mA, not the supply's 12 A.

Twelve amps is 100 % of nameplate; a 24/7 device should not sit there, and the estimate itself has a tolerance of roughly ±10–20 %. Nine amps is 75 % of rating, leaves room for that error, and is still above the ~2.3 A a worst-case normal frame draws, so the limiter stays inert where it matters.

Alternatives: 12 A (rejected, no headroom and no derating), 8 A (rejected as needlessly dark for the all-on test with no safety gain), a limit derived from measured current only (rejected, see D4).

### D2: Apply one uniform scale after gamma and whitepoint correction

The budget step sits between the corrected frame and the display write, because those are the values the LEDs actually emit. When the estimate exceeds the limit, every channel of every pixel is multiplied by the same factor.

Uniform scaling preserves hue within a pixel and relative brightness between pixels, which matters because the clock's word brightness is part of its appearance. Clipping individual channels would shift hues and create banding; scaling before gamma would apply the wrong curve; suppressing whole test patterns would hide the condition rather than bound it.

Alternatives: per-channel clipping (rejected, changes colour), scaling in linear light before gamma (rejected, wrong mapping for this pipeline), refusing to render over-budget frames (rejected, leaves the strip stale).

### D3: Estimate from quiescent plus drive, calibrated rather than assumed

The estimate is:

```
I_mA = pixels × I_q  +  Σ_pixels (r + g + b) × k_drive
```

with `I_q ≈ 1 mA` per pixel and `k_drive` the drive constant in mA per count. The datasheet's 20 mA per channel at full scale puts `k_drive` near 20/255, but the strip in hand should set it: measure all-white once and derive `k_drive = (I_measured − pixels × I_q) / Σ(r + g + b)`. Real strips typically land nearer 45–55 mA per pixel at full white than the 60 mA worst case.

Scaling must account for the quiescent term, or the result undershoots:

```
budget_drive = limit_mA − pixels × I_q
scale        = budget_drive / measured_drive        // clamp to 1.0
```

A 95 % margin is applied to the limit, and after scaling the estimate is recomputed; if rounding leaves it above the limit, the scale is stepped down until it does not. That keeps the "never exceeded" requirement true even with 300 pixels rounding upward.

Alternatives: a flat 60 mA per lit pixel (rejected, over-conservative and blind to brightness), a per-pixel lookup table (rejected, no benefit at this accuracy).

### D4: Use the existing ACS712-20A rather than buying a sensor

The installed ACS712ELCTR-20A gives 100 mV/A over ±20 A, runs from 5 V, is galvanically isolated, and has 1.5 % total error at 25 °C. That is enough for a limit that only needs to keep the commanded current comfortably under 12 A.

Alternatives and why they lost: INA226 with a 2 mΩ Kelvin shunt is the better instrument (16-bit, bus voltage, 1.25 mA per LSB) but had no hobbyist stock; INA260 has an integrated 2 mΩ shunt and a 15 A ceiling but was out of stock at the only shop that listed it; the INA219 breakout is 3.2 A and therefore unusable; the M5Stack INA226-10A at Botland (€9.50, ±10 A) remains the fallback if precision is ever wanted, since 10 A still clears a 9 A limit.

### D5: Divide the sensor output and read it on ADC1

The output idles at VCC/2 and rises 100 mV per amp, so at 5.0 V it is 2.5 V at zero and 3.7 V at 12 A — above the 3.3 V rail. A 10 k/10 k divider halves it to 1.25–2.25 V, which also keeps the load above the datasheet's 4.7 kΩ minimum, and 100 nF from the midpoint to ground gives a ~318 Hz low-pass that suppresses PWM ripple.

The reading goes to GPIO0 or GPIO1, which are ADC1 channels. ADC2 is unusable while Wi-Fi is active, GPIO3 carries the strip data, and GPIO2 carries the on-board LED.

Alternatives: feeding 3.3 V to the sensor (rejected, below its 4.5 V minimum), connecting the output directly (rejected, over-voltage above roughly 8 A), a smaller divider ratio (rejected, leaves no headroom for supply variation).

### D6: Power the sensor from a stable 5 V, not the measured rail

The ACS712 is ratiometric: zero is VCC/2 and sensitivity scales with VCC. Powered from the LED rail, a 0.2 V sag moves the zero by 100 mV — a full amp of apparent current — and compounds with the sensitivity error to roughly 15 % low at 9 A. Taking VCC from the C3's own 5 V pin costs 10 mA and keeps the zero stable; the current path stays isolated, so there is no penalty.

If the build instead shares one 5 V supply across the controller, the sensor and the strip, this choice disappears: the sensor's VCC is the rail it measures, and the second ADC channel on VCC becomes required rather than optional. Measuring VCC on that channel makes the compensation exact, because both the zero and the sensitivity are known functions of the same node.

Alternatives: powering from the measured rail with a second ADC channel measuring VCC and compensating (the single-supply case, and the refinement to reach for if drift shows up), or zeroing on every boot without compensating (rejected, boot happens rarely and the rail state varies).

### D7: Slow outer loop with hysteresis, never per frame

The estimate is the primary limiter and runs every frame. The measurement drives a slow trim: if the measured current stays above the limit for more than a second, reduce the applied scale in small steps; recover only when it falls below a configured fraction of the limit; bound the rate so the scale cannot oscillate.

The current changes once per minute when the colour changes, so there is nothing to gain from a fast loop, and a Hall sensor's noise makes one actively harmful. The first second after power-up is ignored, because charging the bulk capacitor looks like a current spike.

Alternatives: per-frame closed-loop control (rejected, oscillates), using the measurement as the sole limiter (rejected, too slow for a bad frame and dependent on a single sensor).

### D8: Keep the existing distribution, thicken only the trunk if needed

1.5 mm² copper is about 11.7 mΩ/m. A row rail carries 1.08 A at full white, or 1.5 A with reserve, so a one-metre 5 V-and-ground pair drops only 26–35 mV. The row rails are already oversized by roughly 7× and should not be changed.

The trunk is the only place to look: 16 rows at full white is 17 A, and 9 A limited is still substantial. A 1.5 mm² trunk is acceptable up to about 0.5 m and marginal at 1 m (0.21 V), so a longer trunk, or any hope of the un-limited 17 A case, should move to 2.5–4 mm² while the row rails stay as they are. Terminations matter more than the copper at these currents: screw terminals or soldered joints, never JST or Dupont.

Alternatives: replacing all rails with 2.5 mm² (rejected, no measurable benefit and much harder to route), moving the supply closer instead (equally valid where the layout allows).

### D9: One 5 V supply for everything, firmware updates over OTA

The clock runs from a single 5 V supply that feeds the controller, the sensor and the strip. There is no second supply and no USB power in the finished build.

This is the simplification the user asked for, and it is what the existing build already looks like: one 60 W supply, a distribution trunk, and per-row rails. It also removes a class of failure that a two-supply build invites — ground offsets and one supply back-feeding the other through a shared USB cable.

The cost is one hard rule: **USB and the external 5 V are never connected at the same time.** VBUS on the C3 is the same node as the `5V` pin, so connecting both puts two supplies in parallel with no current sharing and no back-feed protection. A series diode does not fix that; only a power mux (TPS2116, LM66200) would, and a mux is not worth it here because firmware updates go over OTA.

Consequences that shape the rest of the design:

- Power enters the ESP32 at the pin **labelled `5V`** on the silkscreen (header position 19 in the 28-pin numbering), which is the VBUS net and is explicitly bidirectional. It has no GPIO number: `GPIO19` is USB D+ on header position 25 and must never be fed. The `3V3` pin is a regulated output and must never be fed either.
- Bulk capacitance plus 100 nF sits at the 5 V pin: at least 100 µF as designed, 1000 µF in the prototype. Without capacitance at the board, uploads on the rail took 35.6 s and 39.4 s against 10.8 s on USB power; with 1000 µF fitted, 15.5 s. One sample on the fast side, so suggestive rather than proven causal.
- The sensor's VCC is now the rail it measures, which is exactly the case D6 says turns the second ADC channel from optional into required; D9 makes it required.
- The controller and the strip were confirmed to run from that one rail on the bench: with the board powered from `5V` and no USB attached, an over-the-air update drove the offcut strip's first five pixels through the level shifter, and the pattern was correct by eye.
- The only in-situ observability is the network, because there is no USB console when the rail is the power source. The OTA spike therefore answers `GET /` with a status document as well as accepting `PUT /ota`, and the clock should keep a small status surface for the same reason.
- First flashing and any recovery still need USB. Unplug the rail, or use a charge-only cable with VBUS cut, rather than powering both.

Alternatives and why they lost: keeping the USB supply for the controller and the 5 V rail for the LEDs (rejected, that is the arrangement the user is replacing); a series diode between the rail and the 5 V pin (rejected — a 1N4148 both drops too much and is rated far below the C3's ~335 mA Wi-Fi peak, and it still does not stop the rail from feeding the host through VBUS); a power mux (rejected as unnecessary once OTA is the update path).

## Risks / Trade-offs

- **Estimate error of ±10–20 %** → the 9 A limit plus a 95 % margin keeps the command under 12 A even at the pessimistic end, and `k_drive` is calibrated from the strip rather than assumed.
- **Ratiometric drift if the sensor is mis-powered** → D6 powers it from a stable 5 V; on a single 5 V rail, add the second ADC channel on VCC and compensate.
- **Sensor placement** → the shunt sits in the LED branch, so the controller's own draw is not counted and the estimate and measurement describe the same load; keep it there if the distribution is reworked.
- **C3 ADC noise and non-linearity** → average 32–64 samples, two-point calibrate, and accept 0.1–0.2 A effective resolution; the M5Stack INA226-10A is the escape hatch.
- **The ACS712 module's own current path may be rated below the die** → check terminal temperature after a minute of all-white at 12–17 A; the die's 1.2 mΩ is not the module's limitation.
- **The limiter hides a real fault** → it bounds the command, not the load; supply OCP and a fuse stay in the design, and engaging the limiter is logged rather than silent.
- **Slow trim could alter a parity frame** → normal frames never reach the limit, and the trim only moves when the measurement is sustained above it; if it ever engages, that is logged as a fault condition, not a normal state.
- **A mis-set limit makes the clock dark** → the limit is validated at startup, the applied scale is logged, and the default is chosen so normal frames are untouched.
- **Someone connects USB while the rail is live** → the rule is stated in the README ahead of the wiring instructions, and the OTA path removes the everyday reason to plug in; a charge-only cable is the safe way to flash with the rail disconnected.
- **No console in situ** → the device is observable over HTTP instead, which is why the OTA endpoint doubles as a status endpoint.

## Migration Plan

1. Add the budget stage to the core with the limit and `k_drive` as configuration; default the limit to 9 000 mA and the constant to the calibrated value once measured.
2. Add the sensor read path to the selected display backend, and keep it optional so a build without a sensor still renders.
3. Verify with the all-white test pattern: compare the measured current against the estimate, adjust `k_drive`, and confirm the scaled frame stays under the limit.
4. Confirm a normal clock frame is byte-identical with the limiter enabled and disabled.
5. Rollback: set the limit to disabled in configuration, which restores the exact pre-change output and logs a warning that the supply is unprotected by firmware.

## Open Questions

- ~~Whether the ratiometric compensation needs the second ADC channel in practice~~ settled by D9: on a single rail the sensor's VCC is the measured rail, so the channel is required. Still open is the ADC pin, which must be an ADC1 channel not taken by the strip's data line.
- The exact `k_drive` for the strip in hand, pending the all-white measurement.
