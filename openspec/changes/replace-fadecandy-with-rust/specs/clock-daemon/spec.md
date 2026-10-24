# Spec Delta

## Purpose

Runs the clock continuously on the Pi, updating at minute boundaries, surviving reboots, NTP delays, and transient output errors, and making an orderly stop unmistakable on the strip.

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
On SIGTERM, the daemon SHALL stop scheduling new frames, attempt to write the shutdown pattern, log the outcome, and exit successfully.

#### Scenario: Service stop
- **WHEN** the operator runs `systemctl stop wordclock`
- **THEN** the strip replaces the last minute with the shutdown pattern and the service exits within the configured stop timeout

#### Scenario: Failing shutdown write
- **WHEN** the shutdown-pattern write fails
- **THEN** the daemon logs the error and still exits within the stop timeout

### Requirement: Shutdown pattern
On an orderly stop, the daemon SHALL replace the last clock frame with a uniform dim fill of all logical pixels, so the strip cannot be read as showing a time.

#### Scenario: Pattern cannot read as a time
- **WHEN** the shutdown pattern is written
- **THEN** every logical pixel is lit at the same dim value and no word from the previous minute is visible

#### Scenario: Pattern survives process exit
- **WHEN** the daemon has exited and the strip still has power
- **THEN** the strip continues to show the uniform fill, because the WS2812 strip latches its last frame

#### Scenario: System shutdown or reboot
- **WHEN** the system is shut down or rebooted in an orderly way
- **THEN** systemd delivers SIGTERM, the daemon writes the shutdown pattern before exiting, and the pattern remains until the LED supply is cut

### Requirement: Shutdown fill rendering
The fill SHALL use the raw value 32,32,32 for every logical pixel, bypass the clock gamma table, and leave padding beyond 288 black. A uniform fill cannot encode a time, and bypassing gamma keeps it clearly visible. This applies to orderly stops only, not to SIGKILL, crashes, or power loss.

#### Scenario: Gamma bypass
- **WHEN** the shutdown fill is rendered
- **THEN** each logical pixel is transmitted as 32,32,32 rather than the gamma-corrected value for 32

#### Scenario: Padded strip
- **WHEN** the configured physical strip is longer than 288 pixels
- **THEN** the shutdown pattern lights all logical pixels and leaves the padding pixels black

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
