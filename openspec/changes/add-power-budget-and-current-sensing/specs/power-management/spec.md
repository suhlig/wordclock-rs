# Spec Delta

## Purpose

Bounds the current the clock commands from its 5 V supply, estimates that current from the corrected frame, and uses a measured current to calibrate and trim the bound.

## ADDED Requirements

### Requirement: Commanded-current limit
The system SHALL bound the current it commands from the 5 V supply to a configured limit in milliamperes, applied after gamma and whitepoint correction and before the frame is written to the display.

#### Scenario: Frame above the limit
- **WHEN** the estimated current for a corrected frame exceeds the configured limit
- **THEN** every channel of every pixel is scaled by one common factor, and the scaled frame's estimate is at or below the limit

#### Scenario: Relative colors preserved
- **WHEN** a frame is scaled to meet the limit
- **THEN** channel ratios within each pixel and brightness ratios between pixels are preserved within rounding error

#### Scenario: Limit never exceeded
- **WHEN** any frame, including an all-white test pattern, is written
- **THEN** the estimated commanded current does not exceed the configured limit

### Requirement: Transparency below the limit
The system SHALL write frames unchanged when their estimated current is at or below the configured limit, so normal operation is unaffected.

#### Scenario: Normal clock frame
- **WHEN** the clock renders a frame with its usual 11–39 lit pixels
- **THEN** the frame written to the display is byte-identical to the frame the color pipeline produced

#### Scenario: Dim shutdown pattern
- **WHEN** the orderly shutdown pattern is written and its estimate is below the limit
- **THEN** it is written unchanged

### Requirement: Current estimate model
The system SHALL estimate current as a per-LED quiescent term plus a per-channel drive term computed from the corrected pixel values, and SHALL allow the per-pixel drive constant to be set from measurement.

#### Scenario: Black frame
- **WHEN** every pixel is black
- **THEN** the estimate equals the quiescent term for the configured physical strip length

#### Scenario: Calibrated constant
- **WHEN** an operator supplies a measured current for an all-white frame
- **THEN** the per-pixel drive constant is derived from it and used for subsequent estimates

### Requirement: Measured current
The system SHALL sample the current sensor in the 5 V feed, average multiple samples into one reading, and make that reading available to calibration, trimming, and logging; loss of the sensor SHALL NOT stop rendering.

#### Scenario: Averaged reading
- **WHEN** the sensor output is sampled during a frame
- **THEN** the reported current is an average over multiple samples rather than a single conversion

#### Scenario: Sensor unavailable
- **WHEN** the sensor is missing or its reading is out of range
- **THEN** the clock keeps rendering, the estimate-only limit stays active, and the condition is logged

### Requirement: Ratiometric compensation on a shared supply
When the sensor is powered from the same 5 V rail it measures, the system SHALL sample that rail's voltage and use it to compensate both the sensor's zero point and its sensitivity, because the ACS712 output is ratiometric.

#### Scenario: Rail sagging under load
- **WHEN** the 5 V rail drops from 5.0 V to 4.8 V while the strip draws current
- **THEN** the reported current is compensated for the shifted zero point and reduced sensitivity, and stays within the sensor's stated error of the true current

#### Scenario: Supply-voltage channel unavailable
- **WHEN** the voltage-reading channel is missing or out of range
- **THEN** the system falls back to the nominal 5.0 V, keeps rendering, and logs that the current reading is uncompensated

### Requirement: Slow trim
The system SHALL adjust the applied scale slowly when the measured current is sustained above the limit and SHALL recover it slowly when the measurement falls below a lower threshold, using hysteresis so the scale does not change every frame.

#### Scenario: Sustained over-current
- **WHEN** the measured current stays above the limit
- **THEN** the applied scale is reduced in small steps until the measurement is below the limit

#### Scenario: Recovery
- **WHEN** the measured current falls below a configured fraction of the limit
- **THEN** the applied scale recovers in small steps without overshooting the limit

#### Scenario: No per-frame oscillation
- **WHEN** the measurement sits near the limit
- **THEN** the scale changes at a bounded rate rather than on every frame

### Requirement: Current observability
The system SHALL log the measured current, the supply voltage where available, and the applied scale at a bounded rate, and SHALL make them available without a serial console.

#### Scenario: Periodic log line
- **WHEN** the clock is running with a sensor attached
- **THEN** it emits current, voltage, and applied scale at a configured interval without flooding the log

#### Scenario: No console attached
- **WHEN** the clock runs from its 5 V supply with no USB connection
- **THEN** the same values are readable over the network
