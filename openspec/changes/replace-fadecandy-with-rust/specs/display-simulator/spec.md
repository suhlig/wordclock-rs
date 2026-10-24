# Spec Delta

## Purpose

Renders the same frame the Pi would show to a terminal, so the clock logic and color pipeline can be developed and tested without hardware.

## ADDED Requirements

### Requirement: Hardware-free operation
The simulator SHALL run on macOS and Linux without SPI, GPIO, Fadecandy, or network access.

#### Scenario: Simulator on a laptop
- **WHEN** the simulator is built and run on macOS
- **THEN** it renders a frame without opening a hardware device or failing on missing permissions

### Requirement: Same frame as hardware
For the same time, occasion, color seed, and pixel configuration, the simulator SHALL render the same lit indices and the same corrected colors as the hardware frame.

#### Scenario: Simulator and hardware agree
- **WHEN** a fixed time and fixed color sequence are rendered through both backends
- **THEN** the two frames are identical except for the physical padding beyond 288 pixels

### Requirement: Time snapshot
The simulator SHALL accept a time argument, render one frame, and exit; it SHALL accept the legacy `DD.MM.YYYY HH:MM` format and an ISO-like format.

#### Scenario: Legacy time format
- **WHEN** the simulator is invoked with `12.11.2017 17:23`
- **THEN** it renders the frame for 17:23 and exits successfully

#### Scenario: Default to now
- **WHEN** the simulator is invoked without a time argument
- **THEN** it renders the current local time and exits successfully

### Requirement: Demo loop
The system SHALL provide a demo mode that walks all 1,440 minutes at a configurable rate, matching the legacy showoff tool.

#### Scenario: Full-day demo
- **WHEN** the demo mode runs from 00:00 through 23:59
- **THEN** it visits every minute once, prints the current time for progress, and sleeps for the configured interval between frames

### Requirement: Fixed color override
The simulator SHALL accept a fixed RGB color override so the output can be compared visually and in tests without random sampling.

#### Scenario: Fixed red frame
- **WHEN** the simulator is invoked with the color override `ff0000`
- **THEN** every lit pixel is corrected red and no random color is sampled

### Requirement: Deterministic sampling
The simulator SHALL accept a seed for random color sampling so repeated runs with the same arguments produce the same output.

#### Scenario: Same seed, same output
- **WHEN** the simulator is run twice with the same time, seed, and configuration
- **THEN** both runs produce identical frames

### Requirement: Terminal layout
The simulator SHALL render the word grid as 16 rows of 18 characters, with unlit characters dim and lit characters colored by their corrected RGB value.

#### Scenario: 18×16 output
- **WHEN** a frame is rendered to the terminal
- **THEN** the output contains 16 rows and 18 character cells per row, plus any border or spacing the legacy terminal client used
