# Proposal

## Why

The clock can only be read by standing in front of it, and the device's existing HTTP root is a plain-text status document aimed at `curl`. A browser on the LAN should show the same face the strip is showing right now, at the same minute and in the same colours, so the clock can be checked from a phone or a wall tablet without adding a second renderer that could drift. The device already owns the state, the HTTP server, and a stable simulator to compare against; the missing piece is a small, push-based web representation.

## What Changes

- Add a web presentation of the clock on the selected ESP32-C3: an HTML page at the root of a dedicated web port that recreates the WordClock24h front plate as an 18×16 letter grid, with lit cells in the corrected colour the strip is currently emitting and unlit cells dim.
- Push the displayed state over Server-Sent Events: one event on connect and one at each displayed-minute change, with heartbeat comments between; the page never polls and never derives the time from the browser clock.
- Serve the frame from a snapshot published by the clock task, so the page shows the same minute, phrase, per-pixel occasion colours, and dark or unsynchronized state as the LEDs.
- Show firmware version, build revision, uptime, IP address, SNTP sync age, OTA slot, strip length, current phrase and colour, and a link to the GitHub repository, without obscuring the face.
- Collect runtime resource usage on the device: the running app image against its partition, heap used and maximum, static RAM, main-stack high-water, and CPU busy time, and expose it with stable names and units.
- Show the resource metrics in the page's metadata block and deliver them on the existing SSE stream as a `health` event on connect and every 60 seconds, so the page still does not poll.
- Fix the metric names, units, and meanings in a repository document so the later MQTT health document and Prometheus gauges reuse them unchanged.
- Keep the existing plain-text status and OTA endpoints on the current port unchanged, and serve the web face on a separate port so an open SSE connection cannot block an update.
- Add `wordclock serve` to the host binary: the same HTML, event format, and metadata served from a laptop, with fixed colour, seed, and frozen time for deterministic parity tests against `sim`.
- Recreate the canonical front plate with inline HTML and CSS only: no image, no web font, no CDN, and no JavaScript framework, so the page works offline on the LAN and fits in firmware flash.

## Capabilities

### New Capabilities

- `web-clock-face`: HTML and SSE presentation of the shown clock state, including the front-plate layout, wall-clock synchronization, metadata, client limits, and the host preview server.
- `resource-metrics`: runtime flash, RAM, and CPU usage with stable names, units, and meanings, displayed by the web page now and reused by the later MQTT and Prometheus publishers.

### Modified Capabilities

None. The new capabilities read the existing `clock-rendering`, `color-pipeline`, `display-simulator`, and `deployment` contracts without changing them; the ESP32-C3 OTA and plain-text status endpoints keep their behavior.

## Impact

- `wordclock-core` gains a `no_std` web module that renders the page and the SSE event from a shown-state snapshot, and metric types and formatters shared by the page, the plain-text status, and the future MQTT and Prometheus publishers.
- `wordclock-host` gains a `serve` subcommand and a small thread-per-connection HTTP server; no new runtime or web framework.
- `wordclock-esp32` gains a snapshot publication from `clock_task`, a second HTTP server with a fixed SSE client pool, additional static buffers, and a larger `StackResources`, without touching the OTA path. It also gains the runtime samplers for the app-image length, heap, static RAM, stack high-water, and CPU idle time.
- Documentation: README, `docs/esp32-c3.md`, a page screenshot, and `docs/metrics.md` with the stable metric contract; the release workflow gains a version identity for the page.
- No TLS, authentication, external assets, MQTT or Prometheus transport, or command surface; the page is LAN-only, consistent with WCR-D21 of the archived `replace-fadecandy-with-rust` change. This change fixes the resource metric contract those later transports will use but does not implement them.
- No change to the words, colours, cadence, or physical output.
