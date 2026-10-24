# Spec Delta

## Purpose

Delivers a complete color frame to the physical WS2811B strip with the correct pixel count, pixel order, color order, and recovery behavior.

## ADDED Requirements

### Requirement: Complete frame with padding
The system SHALL write all 288 logical word pixels and, when the physical strip is longer, SHALL write the remaining pixels as black.

#### Scenario: 300-LED strip
- **WHEN** the configured physical strip length is 300 and a clock frame is written
- **THEN** 300 pixels are transmitted, the first 288 carry the word pattern, and the last 12 are black

#### Scenario: 288-LED strip
- **WHEN** the configured physical strip length is 288
- **THEN** exactly 288 pixels are transmitted and no extra pixels are addressed

### Requirement: Stable pixel order
The system SHALL transmit logical pixel 0 first and SHALL preserve ascending logical order unless a verified physical mapping is configured.

#### Scenario: Identity order
- **WHEN** no physical mapping is configured and a single-pixel test lights logical pixel 0
- **THEN** the first physical LED lights and no other LED does

#### Scenario: Verified mapping
- **WHEN** the test pattern shows that the physical strip is not in logical order
- **THEN** the configured mapping is applied consistently to every frame and to the test pattern

### Requirement: WS2811B color order
The system SHALL transmit each pixel in the green-red-blue order required by the WS2811B strip.

#### Scenario: Primary colors
- **WHEN** the test pattern requests pure red, pure green, and pure blue
- **THEN** the strip shows red, green, and blue respectively with no swapped channels

### Requirement: Periodic refresh
The system SHALL refresh the current frame at least once per second while the daemon is running, so a transient strip reset or glitch recovers without waiting for the next minute.

#### Scenario: Recovery after a strip glitch
- **WHEN** the strip resets while the clock is showing a static minute
- **THEN** the daemon rewrites the same frame within one second and the display recovers

### Requirement: Output failure handling
If the LED output device is unavailable or a write fails, the system SHALL log the failure and keep retrying; it SHALL NOT exit silently or leave the previous error unreported.

#### Scenario: SPI device missing
- **WHEN** the daemon starts without the configured SPI device
- **THEN** it logs a clear error, remains running, and retries until the device becomes available

### Requirement: Hardware diagnostic pattern
The system SHALL provide a test pattern that can walk individual pixels, light rows in distinct colors, and show the primary colors.

#### Scenario: Walking pixel
- **WHEN** the operator runs the walking-pixel pattern
- **THEN** exactly one pixel lights at a time in ascending logical order and the operator can identify the physical path

#### Scenario: Row pattern
- **WHEN** the operator runs the row pattern
- **THEN** each physical row is a distinct color and the operator can verify orientation and serpentine wiring

### Requirement: No Fadecandy dependency
The system SHALL drive the strip without Fadecandy, USB, or Open Pixel Control.

#### Scenario: Boot without Fadecandy
- **WHEN** the Pi boots with no Fadecandy board attached
- **THEN** the daemon still renders the clock over SPI
