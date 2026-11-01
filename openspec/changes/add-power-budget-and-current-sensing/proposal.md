# Proposal

## Why

The clock runs from a 60 W, 5 V supply — 12 A continuous — while the 300-pixel WS2812B strip is rated at roughly 18 A at full white and measures about 10 A when extrapolated from two bench points (see *Measured draw* in `docs/hardware.md`). Normal operation is nowhere near that: a clock frame lights 11–39 pixels, about 0.4–1.6 A including quiescent current. The exposure is the exception — the all-on test pattern in the hardware guide, a future feature that lights the whole strip, or a rendering bug — and no code path today bounds it. The build also already carries an ACS712-20A Hall sensor in the LED branch, so the actual current can be observed without buying anything. Encoding the limit in firmware, and using the existing sensor to confirm it, turns an assumption into a checkable property.

## What Changes

- Add a power-budget stage to the frame path, after gamma and whitepoint correction and before the display write: estimate the current the frame will command and, if it exceeds the configured limit, scale every channel by the same factor.
- Configure the limit at 9 000 mA rather than the supply's 12 A. Seventy-five percent of nameplate is the continuous design point for a 24/7 device.
- Keep the estimate explicit about its terms: quiescent current per LED plus a per-channel drive term, with the per-pixel constant calibrated from two measurements at different brightness instead of assumed from the datasheet.
- Read the existing ACS712-20A through a 10 k/10 k divider and 100 nF into an ADC1 pin, average over many samples, zero at boot, and account for the ratiometric output.
- Use the measurement for three distinct jobs: calibrating milliamps per pixel, trimming the applied scale slowly with hysteresis, and logging current and bus voltage.
- Treat the limiter as an open-loop bound, not a safety device. The supply's over-current protection and a fuse remain the fault path.
- Record the distribution decision that follows from the budget: 1.5 mm² row rails are sufficient at 0.65–1.0 A per row, and the trunk needs 2.5–4 mm² only beyond roughly half a metre or if the un-limited all-on case, which the measured constant puts near 10 A, is ever required.
- State that the limiter is inert at normal load, so parity frames are unchanged and the existing color-pipeline contract still holds.

## Capabilities

### New Capabilities

- `power-management`: bounds the current the clock commands from its 5 V supply, estimates it from the corrected frame, applies a uniform scale when over budget, and consumes a measured current for calibration, slow trim, and logging.

### Modified Capabilities

None. `led-output` and `color-pipeline` are defined in the `replace-fadecandy-with-rust` change; this change adds a stage in front of the display write rather than changing their requirements.

## Impact

- The host-independent core gains a pure post-processing step and configuration, so both host paths inherit the limit.
- The selected display backend gains an analog input for the sensor alongside the digital path that an INA226 would have used.
- No purchase: the ACS712-20A is already installed in the main rail. No INA226 or INA260 was available from the hobbyist suppliers checked, and the common INA219 breakout's 0.1 Ω shunt is unsuitable at this current.
- The hardware guide gains the divider, the sensor supply decision, and the trunk-wire guidance.
- No change to the rendered words, colors, cadence, or the viewer's experience at normal load.
