# Spec Delta

## Purpose

Presents the clock's currently shown state in a web browser on the local network: an HTML recreation of the WordClock24h front plate, updated at every displayed-minute change by Server-Sent Events, with the firmware version and operational metadata. It reads the existing rendering and colour contracts without changing them.

## ADDED Requirements

### Requirement: Face at the web root

The web server SHALL serve a complete HTML document at `GET /` with `Content-Type: text/html; charset=utf-8` and `Cache-Control: no-store`. The document SHALL render the canonical 18-column by 16-row front plate with all 288 stripe characters, including the German umlauts, in uppercase with wide letter tracking on a black square, with three decorative dots below the grid.

#### Scenario: Complete grid
- **WHEN** a client requests `GET /`
- **THEN** the response is HTML containing 18 columns and 16 rows of letters that match `clock::STRIPE` exactly, including `Ü` and `Ö`

#### Scenario: Canonical proportions
- **WHEN** the page is rendered at 800×800 CSS pixels
- **THEN** the plate is square, the letter grid is centred and legible, and the three dots appear below the grid as in the canonical front plate

#### Scenario: Narrow viewport
- **WHEN** the page is rendered at 360×640 CSS pixels
- **THEN** the whole face is visible without horizontal scrolling or clipping, and the metadata stacks below it

#### Scenario: No stale HTML
- **WHEN** a browser reloads the page after a firmware update
- **THEN** it receives the current document rather than a cached copy

### Requirement: Metadata outside the face

The metadata block and repository link SHALL be placed outside the letter grid at every supported viewport so the front-plate layout stays unobstructed.

#### Scenario: Metadata beside the plate
- **WHEN** the page is rendered on a viewport at least 900 CSS pixels wide
- **THEN** the metadata appears beside the plate and does not overlap any letter cell

#### Scenario: Metadata below the plate
- **WHEN** the page is rendered on a narrow viewport
- **THEN** the metadata appears below the plate and does not overlap any letter cell

### Requirement: Same frame as the simulator

For the same local time, fixed colour or seed, birthday list, and gamma configuration, the lit cells and their RGB values on the page SHALL be identical to the logical 288-pixel frame that `cargo run -- sim` renders. The page SHALL show only the 288 logical cells and SHALL NOT show physical padding pixels.

#### Scenario: Fixed colour parity
- **WHEN** the simulator and the page render `12.11.2017 17:23` with colour `ff0000`
- **THEN** both light the same indices and the page uses `#ff0000` for every lit cell

#### Scenario: Seeded colour parity
- **WHEN** the simulator and the page render the same time with the same seed
- **THEN** both use the same gamma-corrected colour for every lit cell

#### Scenario: Padded strip
- **WHEN** the physical strip is configured longer than 288 pixels
- **THEN** the page renders exactly 288 logical cells and no padding pixels

### Requirement: Wall-clock synchronization

The page SHALL show the local minute that the device is currently displaying and SHALL change at the same minute boundary as the LED frame, without a page reload and without polling. The client SHALL NOT use the browser's clock to choose the displayed minute.

#### Scenario: Minute change
- **WHEN** the displayed local time advances from 17:23 to 17:24 while the page is open
- **THEN** the page shows the 17:24 phrase within 2 seconds and does not reload

#### Scenario: Wrong browser clock
- **WHEN** the browser's clock is set hours away from the device's clock
- **THEN** the page still shows the device's displayed minute

#### Scenario: Mid-minute load
- **WHEN** a client requests the page at 17:23:47
- **THEN** the initial response already shows the 17:23 phrase and colour

#### Scenario: Daylight-saving fall-back
- **WHEN** the local zone repeats an hour at the daylight-saving fall-back
- **THEN** both occurrences are shown, and each update carries the UTC instant so a client can distinguish them

### Requirement: Server-Sent Events updates

`GET /events` SHALL respond with `Content-Type: text/event-stream; charset=utf-8` and `Cache-Control: no-store`, SHALL send a `retry` hint and an initial `frame` event immediately on connect, SHALL send exactly one `frame` event per displayed-minute change, and SHALL send a comment heartbeat at least every 20 seconds between changes. The stream SHALL NOT require the client to poll.

#### Scenario: Initial event
- **WHEN** a client connects to `/events`
- **THEN** it receives an initial `frame` event describing the current minute without waiting for the next boundary

#### Scenario: One event per minute
- **WHEN** the displayed minute changes
- **THEN** exactly one `frame` event is sent for that minute, and none is sent again until the next change

#### Scenario: Idle keepalive
- **WHEN** no minute change occurs for 60 seconds
- **THEN** the stream still carries comment heartbeats and the connection stays open

#### Scenario: Reconnect
- **WHEN** the connection drops and the browser reconnects
- **THEN** the first event on the new connection fully replaces the page state with the current minute, without user action

#### Scenario: No polling
- **WHEN** the page is idle for several minutes
- **THEN** it issues no repeated HTTP requests beyond the event stream and its heartbeats

### Requirement: Event content

Each `frame` event SHALL identify the displayed local date and minute, the UTC instant, whether the display is dark, the rendered phrase, and every lit logical cell with its corrected `#rrggbb` colour. On a uniform-colour minute it SHALL also carry the colour's hex value and XKCD name; on a per-pixel occasion minute it SHALL carry every per-pixel colour and no uniform colour.

#### Scenario: Uniform minute
- **WHEN** the minute uses one sampled colour
- **THEN** the event has one colour for all cells plus a matching hex value and XKCD name

#### Scenario: Per-pixel occasion
- **WHEN** the minute is carnival, a birthday, or the New Year party window
- **THEN** the event may contain several distinct corrected colours, the uniform field is absent or null, and the page shows each cell with its own colour

#### Scenario: Dark display
- **WHEN** the device is not synchronized or has blanked after the drift bound
- **THEN** the event reports a dark display, contains no lit cells, and the page shows the dim plate without a stale minute

### Requirement: Live connection state

The page SHALL indicate whether the event stream is connected, reconnecting, or refused because the server's client pool is full, and SHALL recover automatically when the stream becomes available again.

#### Scenario: Stream lost
- **WHEN** the network drops or the device reboots
- **THEN** the page marks itself disconnected and keeps retrying

#### Scenario: Stream restored
- **WHEN** the server returns
- **THEN** the page reconnects, clears the disconnected state, and shows the current minute

#### Scenario: Pool full
- **WHEN** the server has no free event-stream slot
- **THEN** the client receives a refusal and retries later while the page reports that it is waiting

### Requirement: Operational metadata

The page SHALL show the version of the code serving it, the build revision or commit, uptime, IPv4 address, SNTP synchronization state and age, OTA slot and state where applicable, strip length and driver state where applicable, the current local time and zone, the current phrase, the current colour or a per-pixel marker, and the runtime resource metrics. It SHALL link to `https://github.com/suhlig/wordclock-rs` with `rel="noopener noreferrer"`.

#### Scenario: Firmware identity
- **WHEN** the page is served by an ESP32-C3 release image
- **THEN** it shows the release version and numeric revision that identify that image, and the repository link opens the project page

#### Scenario: Unsynced state
- **WHEN** the device has never synchronized or has exceeded the drift bound
- **THEN** the metadata says the clock is not synchronized or dark instead of showing a fabricated age or time

#### Scenario: Host preview
- **WHEN** the page is served by `wordclock serve` on a laptop
- **THEN** firmware-only fields are omitted or marked not applicable, and the host version and commit are shown

#### Scenario: Plain-text status
- **WHEN** a script requests `/status` on the web port
- **THEN** it receives the same metadata as `text/plain; charset=utf-8`, and the existing plain-text status on the OTA port is unchanged

#### Scenario: Resource metadata
- **WHEN** the page is rendered on the device
- **THEN** flash, RAM, and CPU usage are visible with units and are refreshed at least every 60 seconds

### Requirement: Self-contained presentation

The page SHALL load no external images, fonts, stylesheets, scripts, or analytics, and SHALL render correctly when the device has no internet access. The HTML response SHALL carry a Content-Security-Policy that forbids external resources and a nosniff content-type header.

#### Scenario: LAN-only device
- **WHEN** the page is opened on a network with no internet access
- **THEN** the face and metadata render completely, and the only requests go to the device

#### Scenario: Offline fonts
- **WHEN** the browser has no web fonts available
- **THEN** the page uses a local sans-serif stack and the grid keeps its 18×16 geometry

#### Scenario: External asset blocked
- **WHEN** a response would reference an external host
- **THEN** the Content-Security-Policy blocks it and the page still renders

### Requirement: Client concurrency and isolation

The firmware SHALL support at least two concurrent event-stream clients, SHALL refuse additional clients with `503` and a `Retry-After` header when its pool is full, and SHALL NOT let a slow or stalled client delay the LED frame, another client, the status endpoint, or an OTA update.

#### Scenario: Two viewers
- **WHEN** two browsers hold event streams open
- **THEN** both receive each minute's update

#### Scenario: Third viewer
- **WHEN** a third client requests an event stream while the pool is full
- **THEN** it receives `503` with `Retry-After` and can retry later

#### Scenario: Stalled client
- **WHEN** a client stops reading its event stream
- **THEN** the server drops that client within a bounded time and the clock and other clients continue

#### Scenario: OTA during a stream
- **WHEN** an operator uploads firmware while two event streams are open
- **THEN** the update succeeds, and the clients reconnect after the reboot

### Requirement: Separate web port

The web face and event stream SHALL be served on a dedicated port, separate from the OTA and plain-text status port, so that holding an event stream open cannot block an update or the existing status document.

#### Scenario: Existing endpoints unchanged
- **WHEN** an operator requests the existing status document or uploads an image on the OTA port while a browser holds the web stream
- **THEN** both continue to work with their current responses

#### Scenario: Documented URLs
- **WHEN** an operator reads the ESP32-C3 guide
- **THEN** the web face URL and the OTA URL are both documented with their ports

### Requirement: Host preview server

The host binary SHALL provide `wordclock serve` that serves the same HTML, event-stream format, and metadata contract without hardware on macOS and Linux, and that accepts a fixed colour, a random seed, and a frozen time for deterministic tests.

#### Scenario: Deterministic preview
- **WHEN** `wordclock serve` runs with a fixed colour and frozen time
- **THEN** `GET /` and the first `frame` event show the same lit cells and colours as `sim` for that time and colour

#### Scenario: No hardware
- **WHEN** `wordclock serve` runs on macOS without SPI, GPIO, or network credentials
- **THEN** it starts and serves the page without opening a hardware device

#### Scenario: Loopback default
- **WHEN** `wordclock serve` is started without an explicit bind address
- **THEN** it listens only on loopback

### Requirement: Read-only and secret-free

The web presentation SHALL be read-only and SHALL NOT expose Wi-Fi credentials, OTA credentials, or any secret, and SHALL NOT accept commands that change the clock.

#### Scenario: Read-only surface
- **WHEN** a client sends a method other than `GET` to the web port
- **THEN** the server refuses it without changing the display or configuration

#### Scenario: No credentials in the page
- **WHEN** the page and event stream are inspected
- **THEN** no SSID, Wi-Fi password, or other credential appears
