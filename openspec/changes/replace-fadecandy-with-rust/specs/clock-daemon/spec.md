# Spec Delta

## Purpose

Runs the clock continuously on the Pi, updating at minute boundaries and surviving reboots, NTP delays, and transient output errors.

## ADDED Requirements

### Requirement: Minute-boundary scheduling
The daemon SHALL recompute the frame when the local minute changes and SHALL keep the current frame between boundaries.

#### Scenario: Normal minute change
- **WHEN** the local clock advances from 17:23:59 to 17:24:00
- **THEN** the frame changes exactly once to the 17:24 pattern

#### Scenario: Midnight rollover
- **WHEN** the local clock advances from 23:59 to 00:00
- **THEN** the frame changes to the midnight pattern without skipping or duplicating a minute

#### Scenario: Daylight-saving transition
- **WHEN** the local timezone enters or leaves daylight saving time
- **THEN** the daemon continues to follow local wall-clock minutes without a gap longer than one refresh interval

### Requirement: NTP startup gate
After boot, the daemon SHALL NOT display a time until the operating system reports that the clock is synchronized; while waiting it SHALL show an all-off or neutral frame and SHALL log the wait.

#### Scenario: Pi 3 without RTC
- **WHEN** the PI boots without a hardware clock and NTP has not yet synchronized
- **THEN** the strip stays off or neutral and the daemon logs the wait

#### Scenario: NTP synchronization completes
- **WHEN** the operating system reports NTP synchronization
- **THEN** the daemon renders the current local time within one refresh interval

### Requirement: Per-minute color resampling
The daemon SHALL preserve the legacy cron behavior by resampling the uniform color once per minute, and SHALL NOT resample it on each periodic frame refresh.

#### Scenario: Color cadence
- **WHEN** the daemon writes the same minute's frame several times
- **THEN** the color is unchanged, and it changes only when the minute changes

### Requirement: Output resilience
The daemon SHALL keep running after a failed LED write and SHALL retry on the next refresh; fatal errors SHALL be reported so the service manager can restart it.

#### Scenario: Transient SPI failure
- **WHEN** one frame write fails
- **THEN** the daemon logs the error and attempts the next write without exiting

### Requirement: Graceful shutdown
On SIGTERM, the daemon SHALL stop scheduling new frames, clear the strip to black, and exit successfully.

#### Scenario: Service stop
- **WHEN** the operator runs `systemctl stop wordclock`
- **THEN** the strip turns off and the service exits within the configured stop timeout

### Requirement: No runtime network dependency
After the initial time synchronization, the daemon SHALL continue rendering if the network becomes unavailable.

#### Scenario: Network loss
- **WHEN** Ethernet and Wi-Fi are disconnected after startup
- **THEN** the clock continues to update from the local system clock

### Requirement: Single writer
The daemon SHALL prevent a second instance from writing the same LED output concurrently.

#### Scenario: Second instance
- **WHEN** a second daemon is started while the first is running
- **THEN** the second instance exits with a clear error and does not alter the strip

### Requirement: Operational logging
The daemon SHALL log startup configuration, NTP waits, minute changes at debug level, color changes at info level, and all output errors to the system journal.

#### Scenario: Journal inspection
- **WHEN** the operator inspects the service journal
- **THEN** startup, synchronization, color changes, and errors are visible with timestamps
