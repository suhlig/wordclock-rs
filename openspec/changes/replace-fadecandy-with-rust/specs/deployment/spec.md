# Spec Delta

## Purpose

Builds, publishes, and installs the clock for the selected host — a 64-bit Raspberry Pi or an ESP32-C3 — reproducibly, with a tested rollback and no remaining Fadecandy or Ruby dependency.

## ADDED Requirements

### Requirement: Target selection
The implementation SHALL select exactly one runtime host before hardware provisioning — the Raspberry Pi 3B or the ESP32-C3 — and SHALL record the measured evidence for the choice.

#### Scenario: Host recorded
- **WHEN** the target spike finishes
- **THEN** the selected host and its evidence are recorded in the design, and only that host's provisioning path is executed

### Requirement: Target release artifact
Every tagged release SHALL publish the artifact for the selected host and its checksum.

#### Scenario: Pi release
- **WHEN** the selected host is the Raspberry Pi and a release tag is pushed
- **THEN** the release contains a runnable aarch64 Linux binary, the systemd unit, an example configuration, and their checksums

#### Scenario: ESP32-C3 release
- **WHEN** the selected host is the ESP32-C3 and a release tag is pushed
- **THEN** the release contains a flashable firmware image, its checksum, the partition/OTA metadata, and flashing instructions

### Requirement: One-run provisioning
A fresh host SHALL be provisioned by one documented, repeatable procedure.

#### Scenario: Blank Pi SD to running clock
- **WHEN** Ansible runs against a freshly flashed 64-bit Pi SD card
- **THEN** the SPI device exists, the `wordclock` service is active, and the clock renders without manual steps

#### Scenario: Blank ESP32-C3 to running clock
- **WHEN** the operator flashes the release image and supplies Wi-Fi credentials
- **THEN** the firmware joins the network, synchronizes time, and renders without a debugger attached

### Requirement: Legacy services removed or retired
When the Pi host is replaced, provisioning SHALL remove or disable `fcserver`, the Ruby runtime role, and the per-minute wordclock cron entries. When the ESP32-C3 host is selected, the legacy Pi and Fadecandy clock SHALL remain available as the rollback path until the stability window closes.

#### Scenario: Pi host cleanup
- **WHEN** provisioning finishes on the Pi host
- **THEN** `fcserver` is not running or enabled, no wordclock cron entry remains, and Ruby is not required by the clock

#### Scenario: ESP32-C3 host cutover
- **WHEN** the ESP32-C3 host is installed
- **THEN** the legacy Pi and Fadecandy clock remain available but are not in the signal path, and are retired only after the stability window

### Requirement: Version pinning and checksum
The deployment SHALL install a pinned release and verify the published checksum before activating it.

#### Scenario: Pi checksum mismatch
- **WHEN** the downloaded artifact does not match the expected checksum on the Pi
- **THEN** provisioning fails without replacing the running binary

#### Scenario: ESP32-C3 checksum mismatch
- **WHEN** an OTA or flash image does not match the published checksum
- **THEN** the update is rejected and the running firmware remains active

### Requirement: Shutdown stop budget on the Pi
When the selected host is the Raspberry Pi, the systemd unit SHALL keep the default shutdown dependencies and SHALL set a stop timeout of at least 10 seconds, so the daemon can write the shutdown pattern and exit before an orderly halt, poweroff, or reboot removes power.

#### Scenario: Orderly Pi poweroff
- **WHEN** the operator runs `systemctl poweroff` or `systemctl reboot` on the Pi
- **THEN** the unit is stopped with SIGTERM, the shutdown pattern is visible, and the daemon is not killed by SIGKILL

### Requirement: Non-root service on the Pi
When the selected host is the Raspberry Pi, the daemon SHALL run as a dedicated non-root `wordclock` user with access to the SPI, GPIO, and I2C device groups.

#### Scenario: Pi service identity
- **WHEN** the service is running on the Pi
- **THEN** its main process is owned by `wordclock`, not root, and it can open the SPI device

### Requirement: Configuration
The selected host SHALL be configurable for logical pixel count, physical strip length, output transport and device, gamma, whitepoint, timezone, birthday list, refresh interval, and time-source gate. The Pi stores this in a TOML file; the ESP32-C3 stores it in NVS or LittleFS, or compiles defaults into the image.

#### Scenario: Pixel-count change on the Pi
- **WHEN** the physical strip length is changed in the configuration and the service restarts
- **THEN** the daemon writes the new number of pixels and pads or truncates the word area as configured

#### Scenario: Pixel-count change on the ESP32-C3
- **WHEN** the strip length is changed in the firmware configuration and the device is reflashed or updated over the air
- **THEN** the firmware writes the new number of pixels and pads or truncates the word area as configured

### Requirement: Rollback
The legacy Pi and Fadecandy clock SHALL remain available until the selected host has proven itself, and the selected host SHALL have a documented rollback.

#### Scenario: Pi rollback drill
- **WHEN** the operator swaps the old SD card back in and reconnects Fadecandy
- **THEN** the legacy Ruby cron and `fcserver` render the clock again without code changes

#### Scenario: ESP32-C3 rollback
- **WHEN** the ESP32-C3 firmware fails
- **THEN** the operator restores the previous OTA image or reflashes over USB and, if necessary, restores the Pi and Fadecandy clock, without code changes

#### Scenario: Retire the rollback board
- **WHEN** the selected host has run for the agreed stability window
- **THEN** the operator removes the Fadecandy board and USB cable from the signal path, and the selected host's wiring remains the only data path

### Requirement: Hardware guide
The repository SHALL document the selected host's wiring, bill of materials, output enablement, test-pattern procedure, power and grounding, and rollback. Until the host decision is recorded, it SHALL document both the Pi SPI and ESP32-C3 RMT paths.

#### Scenario: New builder
- **WHEN** a builder follows the guide from a bare host and strip
- **THEN** they can wire the level shifter, run the test pattern, and identify the physical pixel order before installing production firmware
