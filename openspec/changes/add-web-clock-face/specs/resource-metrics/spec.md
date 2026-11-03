# Spec Delta

## Purpose

Defines the runtime flash, RAM, and CPU measurements the clock exposes, with stable names, units, and meanings, so the web page, the plain-text status, the later MQTT health document, and the later Prometheus endpoint all report the same numbers.

## ADDED Requirements

### Requirement: Runtime flash usage

The firmware SHALL report the running app image length, the running app partition size, the free bytes, and the percentage used. It SHALL read the image length at runtime from the ESP-IDF image header and segment table in the running partition, and the partition size from the partition table, rather than embedding either as a build constant.

#### Scenario: Current slot
- **WHEN** the resource metrics are read
- **THEN** the used bytes match the flashed image length and the total bytes match the running partition's size

#### Scenario: OTA slot swap
- **WHEN** the device boots from the other OTA slot after an update
- **THEN** the values describe the newly running slot, not the slot that was running before

#### Scenario: Unreadable image header
- **WHEN** the image header or segment table cannot be parsed
- **THEN** the flash values are reported as unknown and no number is invented

### Requirement: Runtime RAM usage

The firmware SHALL report heap used, heap maximum, and heap total bytes from the allocator; static `.data` and `.bss` bytes from the linker symbols; and the main-stack high-water mark and total bytes. The stack value MAY be approximate but SHALL be labelled as such. Consumers SHALL present the three components separately rather than a single total that double-counts the heap's backing memory.

#### Scenario: Heap figures
- **WHEN** the resource metrics are read
- **THEN** heap used, heap maximum, and heap total are present, the maximum is at least the current usage, and the maximum does not decrease until reboot

#### Scenario: Static RAM
- **WHEN** the static figure is compared with the linked image
- **THEN** it equals the `.data` plus `.bss` size reported by the linker map for that build

#### Scenario: Stack high-water
- **WHEN** the device has run a heavier path such as a page load or an OTA upload
- **THEN** the reported stack high-water rises and never exceeds the total stack size, and it resets on reboot

#### Scenario: Components stay separate
- **WHEN** a consumer shows RAM
- **THEN** heap, static RAM, and stack are labelled separately, and no single percentage implies that the heap's backing memory is counted twice

### Requirement: Runtime CPU usage

The firmware SHALL report the CPU busy percentage over the last 60 seconds, the maximum 60-second value since boot, and the core frequency in MHz. It SHALL derive busy time from idle time accumulated in the RTOS idle hook, counting task and interrupt time as busy and time spent waiting for an interrupt as idle.

#### Scenario: Idle clock
- **WHEN** the clock is idle with no page load or update in progress
- **THEN** the busy percentage is below the documented idle threshold

#### Scenario: Load
- **WHEN** a sustained busy path runs on the device
- **THEN** the 60-second value rises, and the maximum records the peak even after the load ends

#### Scenario: Window
- **WHEN** the 60-second window elapses
- **THEN** the busy value is recomputed from the idle and wall-clock deltas and does not drift

### Requirement: Stable metric contract

The page, the plain-text status, and the later MQTT and Prometheus publishers SHALL use the metric names, units, and meanings recorded in `docs/metrics.md`. The MQTT health document SHALL use the same flat JSON keys and units, and each Prometheus metric SHALL be a `wordclock_`-prefixed gauge with the same unit.

#### Scenario: Page and status agree
- **WHEN** the same metric is read from the page and from `/status`
- **THEN** both show the same value and unit

#### Scenario: Later MQTT publisher
- **WHEN** the MQTT health document is added
- **THEN** it uses the documented keys and units rather than inventing new ones

#### Scenario: Later Prometheus endpoint
- **WHEN** the Prometheus endpoint is added
- **THEN** each documented metric is exposed as a `wordclock_`-prefixed gauge with the documented unit

#### Scenario: No redefinition
- **WHEN** a new publisher is added
- **THEN** it does not rename or rescale an existing metric

### Requirement: Health event

The web event stream SHALL send a `health` event with the resource metrics immediately on connect and at least every 60 seconds. `health` events SHALL be separate from `frame` events and SHALL NOT require the page to poll.

#### Scenario: Initial health event
- **WHEN** a client connects to the event stream
- **THEN** it receives a `health` event without waiting for the next minute boundary

#### Scenario: Periodic health event
- **WHEN** the page has been open for 60 seconds
- **THEN** it receives an updated `health` event and refreshes the metadata block

#### Scenario: Frame independence
- **WHEN** a minute boundary passes
- **THEN** the `frame` event changes the face and the `health` event does not, and neither duplicates the other

#### Scenario: No polling
- **WHEN** the page is idle
- **THEN** resource updates arrive on the existing stream and the page issues no additional HTTP requests

### Requirement: Host behaviour

`wordclock serve` SHALL mark firmware-only resource metrics as not applicable, or report a process equivalent where one is defined, and SHALL NOT fabricate flash, heap, stack, or CPU values.

#### Scenario: Host page
- **WHEN** the page is served by `wordclock serve` on a laptop
- **THEN** firmware-only resource fields are marked not applicable while the host version and commit are shown

#### Scenario: No fake numbers
- **WHEN** a resource value is unavailable on the host
- **THEN** the page shows a not-applicable marker rather than a zero or a placeholder that reads as a measurement

### Requirement: Measurement overhead

The resource measurement SHALL add less than 1% average CPU load, less than 1 KiB of heap, and SHALL NOT delay the clock task. The stack scan SHALL run at most once per 60 seconds, and the health payload SHALL remain under 1 KiB.

#### Scenario: Overhead
- **WHEN** the metrics build is compared with the previous build on the same workload
- **THEN** average CPU and heap use stay within the stated bounds

#### Scenario: Clock unaffected
- **WHEN** the metrics task and a health event run while the clock is displaying
- **THEN** the frame cadence and the watchdog behave as they did without the metrics

#### Scenario: Payload size
- **WHEN** a `health` event is serialized
- **THEN** it is smaller than 1 KiB
