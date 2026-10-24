# Spec Delta

## Purpose

Builds, publishes, and installs the daemon on a 64-bit Raspberry Pi reproducibly, with a tested rollback and no remaining Fadecandy or Ruby dependency.

## ADDED Requirements

### Requirement: Aarch64 release artifact
Every tagged release SHALL publish an aarch64 Linux binary, its checksum, the systemd unit, and an example configuration.

#### Scenario: Release download
- **WHEN** a release tag is pushed
- **THEN** the release contains a runnable aarch64 artifact whose `--version` succeeds on the Pi and whose checksum matches the published value

### Requirement: One-run provisioning
A fresh 64-bit Raspberry Pi OS Lite installation SHALL be provisioned by one Ansible run that enables SPI, creates the service user and groups, installs the binary and configuration, and enables the systemd unit.

#### Scenario: Blank SD to running clock
- **WHEN** Ansible runs against a freshly flashed 64-bit SD card
- **THEN** the SPI device exists, the `wordclock` service is active, and the clock renders without manual steps

### Requirement: Legacy services removed
Provisioning SHALL remove or disable `fcserver`, the Ruby runtime role, and the per-minute wordclock cron entries on the clock host.

#### Scenario: No legacy processes
- **WHEN** provisioning finishes
- **THEN** `fcserver` is not running or enabled, no wordclock cron entry remains, and Ruby is not required by the clock

### Requirement: Version pinning and checksum
Ansible SHALL install a pinned release version and SHALL verify the published checksum before placing the binary.

#### Scenario: Checksum mismatch
- **WHEN** the downloaded artifact does not match the expected checksum
- **THEN** provisioning fails without replacing the running binary

### Requirement: Shutdown stop budget
The systemd unit SHALL keep the default shutdown dependencies and SHALL set a stop timeout of at least 10 seconds, so the daemon can write the shutdown pattern and exit before an orderly halt, poweroff, or reboot removes power.

#### Scenario: Orderly poweroff
- **WHEN** the operator runs `systemctl poweroff` or `systemctl reboot`
- **THEN** the unit is stopped with SIGTERM, the shutdown pattern is visible, and the daemon is not killed by SIGKILL

### Requirement: Non-root service
The daemon SHALL run as a dedicated non-root `wordclock` user with access to the SPI, GPIO, and I2C device groups.

#### Scenario: Service identity
- **WHEN** the service is running
- **THEN** its main process is owned by `wordclock`, not root, and it can open the SPI device

### Requirement: Configuration
The installed configuration SHALL control logical pixel count, physical strip length, SPI device and clock, gamma, whitepoint, timezone, birthday list, refresh interval, and the NTP gate.

#### Scenario: Pixel-count change
- **WHEN** the physical strip length is changed in the configuration and the service restarts
- **THEN** the daemon writes the new number of pixels and pads or truncates the word area as configured

### Requirement: Rollback
The legacy SD card and Fadecandy board SHALL remain usable as a documented rollback path until the new implementation has proven itself, and the Fadecandy board SHALL be removed from the signal path once that rollback window closes.

#### Scenario: Rollback drill
- **WHEN** the operator swaps the old SD card back in and reconnects Fadecandy
- **THEN** the legacy Ruby cron and `fcserver` render the clock again without code changes

#### Scenario: Retire the rollback board
- **WHEN** the new implementation has run for the agreed stability window
- **THEN** the operator removes the Fadecandy board and USB cable from the installation, and the SPI wiring remains the only data path

### Requirement: Hardware guide
The repository SHALL document the level-shifter wiring, bill of materials, SPI enablement, test-pattern procedure, and rollback procedure.

#### Scenario: New builder
- **WHEN** a builder follows the hardware guide from a bare Pi and strip
- **THEN** they can wire the level shifter, run the test pattern, and identify the physical pixel order before installing the service
