# Spec Delta

## Purpose

Runs the clock continuously on the selected host — the Pi or the ESP32-C3 — updating at minute boundaries, surviving reboots, time-source delays, and transient output errors, and making an orderly stop unmistakable where the host supports one.

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

### Requirement: Time-source startup gate
After boot, the daemon or firmware SHALL NOT display a time until its time source reports synchronization; while waiting it SHALL show an all-off or neutral frame and SHALL report the wait.

#### Scenario: Pi boot without RTC
- **WHEN** the Pi boots without a hardware clock and NTP has not yet synchronized
- **THEN** the strip stays off or neutral and the daemon logs the wait

#### Scenario: Pi NTP synchronization completes
- **WHEN** the operating system reports NTP synchronization
- **THEN** the daemon renders the current local time within one refresh interval

#### Scenario: ESP32-C3 boot before SNTP
- **WHEN** the ESP32-C3 boots and Wi-Fi or SNTP has not synchronized
- **THEN** the strip stays off or neutral and the firmware reports the wait over its log

#### Scenario: ESP32-C3 SNTP synchronization completes
- **WHEN** the SNTP client reports synchronization
- **THEN** the firmware renders the current local time within one refresh interval

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

### Requirement: Graceful shutdown on hosts with an orderly stop
On a host that delivers an orderly stop signal, the daemon SHALL stop scheduling new frames, attempt to write the shutdown pattern, report the outcome, and exit successfully.

#### Scenario: Pi service stop
- **WHEN** the operator runs `systemctl stop wordclock` on the Pi
- **THEN** the strip replaces the last minute with the shutdown pattern and the service exits within the configured stop timeout

#### Scenario: Failing shutdown write
- **WHEN** the shutdown-pattern write fails
- **THEN** the daemon reports the error and still exits within the stop timeout

### Requirement: Shutdown pattern on the Pi
On an orderly Pi stop, the daemon SHALL replace the last clock frame with a uniform dim fill of all logical pixels, so the strip cannot be read as showing a time.

#### Scenario: Pattern cannot read as a time
- **WHEN** the shutdown pattern is written
- **THEN** every logical pixel is lit at the same dim value and no word from the previous minute is visible

#### Scenario: Pattern survives process exit
- **WHEN** the daemon has exited and the strip still has power
- **THEN** the strip continues to show the uniform fill, because the WS2812 strip latches its last frame

#### Scenario: System shutdown or reboot
- **WHEN** the system is shut down or rebooted in an orderly way
- **THEN** systemd delivers SIGTERM, the daemon writes the shutdown pattern before exiting, and the pattern remains until the LED supply is cut

### Requirement: ESP32-C3 power-loss behavior
The ESP32-C3 firmware SHALL NOT claim an orderly shutdown pattern. It SHALL keep the strip neutral after boot until time is synchronized and SHALL document that a power loss leaves the last frame latched.

#### Scenario: Power loss leaves the last frame
- **WHEN** the ESP32-C3 loses power without an orderly stop
- **THEN** the strip retains the last frame until power returns, and the limitation is documented

#### Scenario: Boot after power loss
- **WHEN** the ESP32-C3 boots without a synchronized time source
- **THEN** the strip stays off or neutral until SNTP synchronization completes

### Requirement: Shutdown fill rendering on the Pi
The Pi shutdown fill SHALL use the raw value 32,32,32 for every logical pixel, bypass the clock gamma table, and leave padding beyond 288 black. A uniform fill cannot encode a time, and bypassing gamma keeps it clearly visible. This applies to orderly Pi stops only, not to SIGKILL, crashes, power loss, or the ESP32-C3.

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

### Requirement: Single writer where multiple processes are possible
On a host that can run multiple processes, the daemon SHALL prevent a second instance from writing the same LED output concurrently. The ESP32-C3 firmware is the only program on the device, so the lock is not required there.

#### Scenario: Second instance on the Pi
- **WHEN** a second daemon is started while the first is running on the Pi
- **THEN** the second instance exits with a clear error and does not alter the strip

### Requirement: Operational logging
The system SHALL log startup configuration, time-source waits, minute changes at debug level, color changes at info level, and all output errors to the host's operational log: the system journal on the Pi, and serial or network logging on the ESP32-C3.

#### Scenario: Pi journal inspection
- **WHEN** the operator inspects the service journal on the Pi
- **THEN** startup, synchronization, color changes, and errors are visible with timestamps

#### Scenario: ESP32-C3 log inspection
- **WHEN** the operator inspects the serial console or the network log
- **THEN** startup, synchronization, color changes, and errors are visible with timestamps
