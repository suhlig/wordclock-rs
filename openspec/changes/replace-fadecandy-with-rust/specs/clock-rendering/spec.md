# Spec Delta

## Purpose

Maps local wall-clock time to the 18×16 German word grid so every minute of the day produces the clock's intended sentence and pixel pattern.

## ADDED Requirements

### Requirement: Full-minute coverage
For every local time from 00:00 through 23:59, the system SHALL produce a non-empty, strictly increasing list of pixel indices in the range 0–287.

#### Scenario: All 1,440 minutes render
- **WHEN** the renderer is invoked for each minute of a day
- **THEN** every invocation returns at least one index, all indices are strictly increasing, and all indices are below 288

### Requirement: Exact legacy phrasing
The system SHALL produce exactly the same lit pixel indices and the same concatenated word string as the legacy Ruby implementation for all 1,440 minutes, including the current edge-case phrasing.

#### Scenario: Known edge cases
- **WHEN** the renderer is invoked for 00:00, 00:06, 01:05, 11:15, 12:15, 13:15, and 23:55
- **THEN** the pixel lists and word strings match the legacy fixture exactly

#### Scenario: Full-day golden fixture
- **WHEN** the full 1,440-minute golden fixture is replayed
- **THEN** every pixel list and word string matches the fixture byte-for-byte

### Requirement: Fixed 18×16 grid
The system SHALL use the legacy 18-character by 16-row word stripe, totaling exactly 288 characters, as the only source of letter positions.

#### Scenario: Grid integrity
- **WHEN** the grid is loaded
- **THEN** it contains 288 characters, 16 rows of 18 characters, and matches the legacy stripe checksum

### Requirement: Sequential word lookup
The system SHALL locate each word of a phrase sequentially in the grid, starting after the previous word, and SHALL skip one character between words unless the previous word ended at a row boundary.

#### Scenario: Adjacent and cross-row words
- **WHEN** a phrase contains multiple words
- **THEN** the first word's first index precedes the second word's first index, and the two words' pixel ranges do not overlap

### Requirement: Host-independent core
The clock renderer SHALL NOT depend on the Raspberry Pi, the ESP32-C3, SPI, RMT, or an operating system. The same 1,440-minute fixture SHALL pass unchanged in the host simulator and in either target's test build.

#### Scenario: Same fixture on both hosts
- **WHEN** the fixture replay runs in the host simulator and in the selected target's test build
- **THEN** both produce the fixture's pixel lists and word strings

### Requirement: Diagnostic sentence
The system SHALL expose the concatenated characters of a rendered pixel list as a sentence string for diagnostics and the simulator, omitting grid positions that are not lit.

#### Scenario: Sentence for a frame
- **WHEN** the renderer returns the indices for 17:23
- **THEN** the diagnostic string equals the concatenation of the grid characters at those indices
