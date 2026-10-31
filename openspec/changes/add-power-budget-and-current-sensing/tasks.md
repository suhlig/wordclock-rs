# Tasks

## 1. Core power budget

- [ ] 1.1 Add the current estimate as a pure core function over a corrected frame: `pixels × I_q + Σ(r+g+b) × k_drive`, with both constants configurable. Verify with unit tests: an all-black frame returns only the quiescent term, an all-white frame matches the formula, and the result is monotonically increasing with brightness.
- [ ] 1.2 Add the budget step between the corrected frame and the display write, applying one uniform scale when the estimate exceeds the limit. Verify with a test asserting every channel is multiplied by the same factor and that the recomputed estimate is at or below the limit.
- [ ] 1.3 Handle rounding by stepping the scale down until the recomputed estimate is at or below the limit. Verify with a 300-pixel all-white frame at the limit: the result must not exceed the limit, and no channel may be clamped independently.
- [ ] 1.4 Verify transparency: render a normal 11–39-pixel frame with the limiter enabled and disabled and assert the two frames are byte-identical.

## 2. Sensor read path

- [ ] 2.1 Add the ACS712 conversion as a pure function: divider factor, nominal VCC, boot-time zero, and clamping of negative readings to zero. Verify with unit tests at 0 A, 9 A, 12 A, and a negative input.
- [ ] 2.2 Add ADC sampling and averaging on the selected backend using an ADC1 channel, 32–64 samples per reading. Verify on hardware with a known load: the reported current is within ±0.2 A of a clamp meter.
- [ ] 2.3 Add the degradation path for a missing or out-of-range sensor: keep rendering, keep the estimate-only limit active, and log the condition once. Verify by disconnecting the sensor pin and confirming the clock keeps showing the time and logs a warning.
- [ ] 2.4 Document the divider, the 100 nF filter, the ADC pin, and the 5 V sensor supply in `docs/hardware.md`. Verify that the pin and divider constants in the documentation match the constants in the code.

## 3. Calibration and trim

- [ ] 3.1 Add a calibration path that derives `k_drive` from a measured all-white current and the quiescent term. Verify with a test that the derived constant reproduces the input measurement within 1 %.
- [ ] 3.2 Implement the slow trim with hysteresis and a bounded rate: reduce when the measurement is sustained above the limit, recover below a configured fraction, and never change more than one step per interval. Verify with a simulated sustained over-current and a subsequent recovery.
- [ ] 3.3 Verify the trim does not move on a noisy measurement near the limit: feed alternating samples just above and just below the limit and assert the scale is unchanged.
- [ ] 3.4 Log measured current, supply voltage where available, and applied scale at a configured interval. Verify the log line appears at that interval and not once per frame.

## 4. Configuration

- [ ] 4.1 Add the limit (default 9 000 mA), `k_drive`, the quiescent constant, and the sensor enable switch to the configuration. Verify that a zero, negative, or absurd limit is rejected at startup with a clear error.
- [ ] 4.2 Add the disable path that restores the pre-change output. Verify that disabling produces byte-identical frames and logs a warning that the supply is not bounded by firmware.
- [ ] 4.3 Verify the default configuration: a normal clock frame is unchanged, and only an all-white test pattern engages the limiter.

## 5. Build documentation

- [ ] 5.1 Add the sensor wiring and rationale to `docs/hardware.md`: 5 V from the C3 rather than the measured rail, 10 k/10 k divider, 100 nF, ADC1 pin, and the conversion formula. Verify the connection table and any diagram match design D5 and D6.
- [ ] 5.2 Record the distribution figures in `docs/hardware.md`: 1.5 mm² row rails are sufficient at 1.1–1.5 A per row, and the trunk moves to 2.5–4 mm² beyond about 0.5 m or for the un-limited 17 A case. Verify the numbers match design D8.
- [ ] 5.3 Document the all-white calibration procedure and the expected currents (roughly 15–18 A un-limited, about 9 A limited). Verify the procedure can be followed as written without extra explanation.

## 6. Integration with the real strip

- [ ] 6.1 Run the all-white pattern on the real 300-pixel strip with the sensor attached. Verify the measured current stays at or below the limit and the estimate is within 20 % of the measurement.
- [ ] 6.2 Run a normal clock frame on the real strip. Verify the displayed words, colours, and brightness are unchanged from the pre-change build.
- [ ] 6.3 Record the measured `k_drive`, the final limit, and the observed peak current in this change's `design.md`.

## 7. Single 5 V rail and OTA updates

- [ ] 7.1 Document the single-rail topology in `docs/hardware.md`: 5 V from the supply into the C3's `5V` pin (the silkscreen label, header position 19 — not GPIO19, which is USB D+), 100 µF + 100 nF at that pin, the controller tapped near the supply, sensor VCC on the measured rail, and the never-both-supplies rule. Verify the README warning, the hardware guide, and design D9 agree.
- [x] 7.2 Prototype OTA over HTTP on the C3 in `spike/esp32c3-ota` and update the running board with `curl` while it is powered from its 5 V pin and no USB is attached. Verify the reported revision changes after the update.
- [ ] 7.3 Add the supply-voltage channel and the ratiometric compensation from design D6/D9 to the sensor path. Verify against the bench supply at two rail voltages that the reported current tracks within ±0.2 A.
- [ ] 7.4 Power the C3 from the 5 V pin instead of USB and re-run a normal clock frame. Verify the frame is byte-identical and the strip shows no difference.

## Workflow follow-up

- Archive the change after review satisfies the project's requirements.
- Verify the archived result with `openspec validate --strict`.
