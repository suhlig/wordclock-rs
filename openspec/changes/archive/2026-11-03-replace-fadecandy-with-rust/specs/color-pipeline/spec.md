# Spec Delta

## Purpose

Selects the color shown for a frame and applies the same gamma and whitepoint correction as Fadecandy, so the clock keeps its current look and color cadence.

## ADDED Requirements

### Requirement: One uniform color per minute
On a regular day, the system SHALL choose one color for the whole frame, apply it to every lit pixel, and resample it at each new minute boundary.

#### Scenario: Color changes at the minute boundary
- **WHEN** the clock advances from one regular minute to the next
- **THEN** every lit pixel changes to the newly sampled color together, and no pixel changes color between those two boundaries

#### Scenario: All lit pixels share the color
- **WHEN** a regular-day frame is rendered
- **THEN** every lit pixel has the same red, green, and blue values

### Requirement: Occasion precedence
The system SHALL evaluate occasions in the legacy order: carnival, Ash Wednesday, configured birthdays, New Year party, New Year hangover, then regular day.

#### Scenario: Carnival wins over a birthday
- **WHEN** the date is a carnival date that is also in the birthday list
- **THEN** the frame uses the carnival per-pixel color mode, not the birthday mode

### Requirement: Carnival mode
On 11 November from 11:11 through 23:59, on Rosenmontag, and on Faschingsdienstag, the system SHALL sample a new color for each lit pixel.

#### Scenario: 11 November after 11:11
- **WHEN** the time is 11 November at 11:11 or later
- **THEN** the frame contains independently sampled colors per lit pixel

#### Scenario: Rosenmontag and Faschingsdienstag
- **WHEN** the date is Rosenmontag or Faschingsdienstag
- **THEN** the frame contains independently sampled colors per lit pixel

### Requirement: Ash Wednesday and New Year hangover
On Ash Wednesday and on 1 January from 04:00 through 23:59, the system SHALL use the fixed grey color 64,64,64 for every lit pixel and SHALL NOT sample from the color table.

#### Scenario: Ash Wednesday is grey
- **WHEN** the date is Ash Wednesday
- **THEN** every lit pixel is 64,64,64

#### Scenario: New Year hangover is grey
- **WHEN** the time is 1 January at 04:00 or later
- **THEN** every lit pixel is 64,64,64

### Requirement: Birthday mode
On each configured birthday, the system SHALL sample a new color for each lit pixel; the default birthday list SHALL be 13 January, 10 March, 8 April, 18 July, and 31 December.

#### Scenario: Default birthday is polychromatic
- **WHEN** the date is 8 April
- **THEN** the frame contains independently sampled colors per lit pixel

#### Scenario: Birthday list is configurable
- **WHEN** the configured birthday list contains a different date
- **THEN** that date uses per-pixel colors and the removed default dates return to regular behavior

### Requirement: New Year party mode
On 1 January from 00:00 through 03:59, the system SHALL sample a new color for each lit pixel.

#### Scenario: New Year after midnight
- **WHEN** the time is 1 January at 00:30
- **THEN** the frame contains independently sampled colors per lit pixel

### Requirement: XKCD color source
The system SHALL sample colors with replacement from the legacy 922-entry XKCD color table and SHALL retain each color's name for diagnostics and future integrations.

#### Scenario: Table integrity
- **WHEN** the color table is loaded
- **THEN** it contains 922 unique hex values with 922 unique names and matches the legacy table checksum

### Requirement: Gamma and whitepoint correction
The system SHALL apply whitepoint scaling followed by gamma 2.5 to each channel, using the Fadecandy curve with linearCutoff 0 and linearSlope 1, and SHALL emit the rounded 8-bit result.

#### Scenario: Gamma lookup values
- **WHEN** a single channel is corrected
- **THEN** input 0 maps to 0, 32 to 1, 64 to 8, 96 to 22, 128 to 46, 160 to 80, 192 to 125, 224 to 184, and 255 to 255

#### Scenario: Identity whitepoint
- **WHEN** the configured whitepoint is 1.0,1.0,1.0
- **THEN** whitepoint scaling does not alter the channel value before gamma correction

### Requirement: Stable frame between boundaries
The system SHALL keep the corrected frame unchanged between minute boundaries, including across periodic output refreshes, so the clock does not flicker or change color on every refresh.

#### Scenario: Refresh does not resample
- **WHEN** the same minute's frame is written more than once
- **THEN** every write contains the same pixel values
