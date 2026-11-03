# led-output Specification

## Purpose
Delivers a complete color frame to the physical WS2812/WS2812B strip over the selected host's transport, with the correct pixel count, pixel order, color order, and recovery behavior.

## Requirements

### Requirement: Host output transport
The system SHALL drive the strip over SPI2 with DMA on the ESP32-C3, the selected host, encoding the four-bit cell defined in `wordclock-core/src/strip.rs` — `0` as `1000` and `1` as `1100` at 2.5 MHz — and SHALL use the same pixel order and green-red-blue color order. The Raspberry Pi SPI0 path is the non-normative rollback and SHALL use the same cell if exercised. The Pi SHALL NOT use a third-party encoder whose cell leaves a shorter gap after a `1` than the four-bit cell.

#### Scenario: Pi SPI output (fallback)
- **WHEN** a frame is written on the Pi
- **THEN** it is encoded with `strip::encode` and transmitted over SPI0 to `/dev/spidev0.0` as one pre-encoded buffer at 2.5 MHz

#### Scenario: ESP32-C3 SPI output
- **WHEN** a frame is written on the ESP32-C3
- **THEN** it is encoded with the same `strip::encode` and transmitted over SPI2 with DMA and GPIO3 as MOSI

#### Scenario: Pixel boundary after a 1
- **WHEN** either host transmits a pixel whose last LED bit is `1` immediately followed by a pixel whose first LED bit is `0`
- **THEN** the low time between them is at least 800 ns and the second pixel shows only its own value

### Requirement: Transport independent of CPU scheduling
On the ESP32-C3 the frame SHALL be transmitted without the CPU having to service the transport at a bounded interval, so that radio interrupts cannot corrupt the bitstream.

#### Scenario: Frame under radio load
- **WHEN** a complete frame is transmitted while Wi-Fi traffic and radio interrupts are active
- **THEN** every pixel receives its own value, and no pixel shows a value that belongs to another position in the frame

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

### Requirement: WS2812/WS2812B color order
The system SHALL transmit each pixel in the green-red-blue order required by the WS2812/WS2812B strip.

#### Scenario: Primary colors
- **WHEN** the test pattern requests pure red, pure green, and pure blue
- **THEN** the strip shows red, green, and blue respectively with no swapped channels

### Requirement: Deterministic start-up frame
At start-up the system SHALL write a complete black frame as soon as the output transport is available, before any other content or status indication, and SHALL write it more than once. This keeps no pixel holding a value latched while the supply was rising, and catches a pixel that is still completing its own power-on reset when the first frame arrives.

#### Scenario: Strip latched a value at power-up
- **WHEN** the strip powers up holding a latched or undefined value
- **THEN** a full-length black frame is written as soon as the transport is ready, and the strip is dark before any clock content appears

#### Scenario: Tail pixel still resetting
- **WHEN** the far end of the strip is still completing its power-on reset when the first frame arrives
- **THEN** a later start-up frame lights it correctly, without needing a power cycle

### Requirement: Periodic refresh
The system SHALL refresh the current frame at least once per second while the daemon is running, so a transient strip reset or glitch recovers without waiting for the next minute.

#### Scenario: Recovery after a strip glitch
- **WHEN** the strip resets while the clock is showing a static minute
- **THEN** the daemon rewrites the same frame within one second and the display recovers

### Requirement: Output failure handling
If the LED output device is unavailable or a write fails, the system SHALL report the failure and keep retrying or restarting the output task; it SHALL NOT exit silently or leave the previous error unreported.

#### Scenario: SPI device missing on the Pi
- **WHEN** the daemon starts without the configured SPI device
- **THEN** it reports a clear error, remains running, and retries until the device becomes available

#### Scenario: SPI output unavailable on the ESP32-C3
- **WHEN** the SPI or DMA channel cannot be initialized or a transfer fails
- **THEN** the firmware reports a clear error and retries or restarts the output task without silently stopping

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
- **WHEN** the selected host boots with no Fadecandy board attached
- **THEN** it still renders the clock over its configured transport
