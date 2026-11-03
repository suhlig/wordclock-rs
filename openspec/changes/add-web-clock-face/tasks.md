# Tasks

## 1. Shared web renderer in wordclock-core

- [ ] 1.1 Add `wordclock-core/src/web.rs` with the `Shown`, `Cell`, `ResourceMetrics`, and `Meta` types and the page and event renderers, and verify `cargo test -p wordclock-core web` passes the new module tests.
- [ ] 1.2 Render the 288-cell plate from `clock::STRIPE` as an 18×16 grid with an accessible phrase and no hard-coded letter list, and verify a test counts 288 cells and finds `Ü` and `Ö`.
- [ ] 1.3 Implement the inline CSS for a square black plate, the canonical grid proportions, the three dots, the dim `#2b2b2b` unlit colour, and the two-column-to-stacked metadata layout, and verify it renders correctly at 800×800 and 360×640 in a browser.
- [ ] 1.4 Implement the inline script that applies the embedded initial state, opens `EventSource("/events")`, applies `frame` and `health` events without `innerHTML`, and shows connected, reconnecting, and waiting states, and verify it passes `node --check` and the three states in a browser.
- [ ] 1.5 Encode `frame` events as the JSON document in design D6, including `unix`, `local`, `dark`, `phrase`, `uniform`, and `cells`, with string escaping, and verify tests cover a uniform minute, a per-pixel minute, a dark minute, and a colour name containing a quote.
- [ ] 1.6 Add a test that walks all 1,440 minutes, asserts that no frame exceeds the snapshot cell capacity, and asserts that each cell list is ascending and duplicate-free; verify the test passes.
- [ ] 1.7 Add the page's resource block for flash, heap, static RAM, stack, and CPU with explicit units and a not-applicable marker, embed the initial health snapshot in the page, update it from `health` events, and verify it appears beside or below the plate without overlapping the grid at 800×800 and 360×640.

## 2. Resource metrics

- [ ] 2.1 Define `ResourceMetrics` with the fields and units in design D14 and formatters for the page, `/status`, and the health event, and verify a test compares every field name and unit against `docs/metrics.md`.
- [ ] 2.2 Implement the running-image length parser for the ESP-IDF header, segment table, checksum padding, and optional appended hash, combine it with the running partition size, and verify it against a real image and a deliberately truncated header, reporting unknown on failure.
- [ ] 2.3 Populate the heap and static RAM fields from `esp_alloc::HEAP.stats()` and the `_data_start`/`_data_end`/`_bss_start`/`_bss_end` linker symbols, and verify the static figure against the linked map and that heap maximum is at least heap used.
- [ ] 2.4 Implement the main-stack high-water mark by painting the unused stack at boot and scanning a snapshot at most once per 60 seconds, and verify it rises under a page load and resets on reboot while being labelled approximate.
- [ ] 2.5 Replace `esp_rtos::start` with `esp_rtos::start_with_idle_hook`, accumulate idle microseconds around `wait_for_interrupt`, and verify an idle device reports below the documented threshold while a sustained busy loop raises both the 60-second value and the maximum.
- [ ] 2.6 Add the low-priority metrics task that samples flash once at boot and heap, stack, and CPU every 60 seconds into a shared snapshot, and verify the clock task's frame cadence and watchdog are unaffected.
- [ ] 2.7 Add the `health` event to the web stream on connect and at least every 60 seconds, and verify with `curl -N` that it carries every metric, stays under 1 KiB, and does not change the `frame` event.
- [ ] 2.8 Add the resource fields to the web `/status` and the existing 3232 status document from the same snapshot, and verify both agree with each other and with the page.
- [ ] 2.9 Measure the metrics build against the previous build and verify under 1% average CPU, under 1 KiB of heap, a health payload under 1 KiB, and no change in frame cadence; record the numbers.
- [ ] 2.10 Keep `docs/metrics.md` in step with the implementation, including the names, units, meanings, cadence, and the MQTT/Prometheus mapping, and verify the page, `/status`, and the document agree.

## 3. Shown-state snapshot

- [ ] 3.1 Define the fixed-size `Shown` snapshot with UTC and local minute, dark flag, uniform colour and name, and at most 64 cells, and verify it fits the documented static budget with a compile-time size assertion.
- [ ] 3.2 Publish the snapshot from `clock_task` when the minute key changes and when the dark state changes, building it outside the lock and swapping it under a short critical section, and verify a host test observes one publication per minute and none on the 1 Hz refresh.
- [ ] 3.3 Verify each published cell colour equals the corresponding `CLOCK_OUT` pixel for a fixed-colour frame and for a per-pixel occasion frame, and verify the test passes.
- [ ] 3.4 Verify the unsynchronized and post-bound dark states publish `dark: true` with no cells, and verify the test covers both transitions.

## 4. Host preview server

- [ ] 4.1 Add `wordclock serve` with `--bind`, `--color`, `--seed`, and `--frozen-time`, and verify `cargo run -p wordclock-host -- serve --help` lists them.
- [ ] 4.2 Implement the `std` HTTP listener with `GET /`, `GET /status`, `GET /events`, `404` for unknown paths, and `405` for other methods, and verify an integration test drives each route over TCP.
- [ ] 4.3 Run a ticker that renders at minute boundaries with `sim::render`, `ColorPicker`, and `GammaLut`, and verify a frozen-time run keeps the state constant and a wall-clock run changes at the next boundary.
- [ ] 4.4 Implement the SSE fan-out with one thread per client, a bounded channel per client, `retry`, an immediate first event, one event per minute, a health event every 60 seconds, and a 15 s heartbeat, and verify an integration test reads the first frame, a health event, and a heartbeat.
- [ ] 4.5 Add a parity test asserting that the page's cells and the first event equal the lit logical pixels and corrected colours from `sim` for a fixed time, colour, and seed, and verify the test passes.
- [ ] 4.6 Verify the default bind is loopback, an explicit LAN bind works, and a port already in use exits with a clear error.
- [ ] 4.7 Verify the host page marks flash, stack, and other firmware-only metrics not applicable rather than showing invented values.
- [ ] 4.8 Document `wordclock serve` in the README with the URL, options, and the parity command, and verify the documented commands run as written.

## 5. ESP32-C3 web server

- [ ] 5.1 Add the web port and its static buffers, raise `StackResources` to cover SNTP, OTA, the listener, and the client pool, and verify the firmware builds for `riscv32imc-unknown-none-elf` and boots with the new port logged.
- [ ] 5.2 Serve `GET /` from the shared renderer and verify with `curl` that the response is HTML with 18×16 letters and `Cache-Control: no-store`.
- [ ] 5.3 Serve `GET /events` with the immediate first event, one event per displayed-minute change, a health event every 60 seconds, and a 15 s heartbeat, formatting the events once into shared static buffers, and verify with `curl -N` across two minute boundaries.
- [ ] 5.4 Serve `GET /status` as plain text with the page's metadata, and verify its version, revision, uptime, address, sync age, slot, strip, and resource fields against the 3232 status document.
- [ ] 5.5 Implement the three-client pool with `503` and `Retry-After: 15` when full, and verify with four concurrent `curl -N` clients that the fourth gets the refusal.
- [ ] 5.6 Drop a client that stops reading within 10 seconds without delaying the clock or another client, and verify with a non-reading socket while the strip keeps updating.
- [ ] 5.7 Add a build-time version identity from `WORDCLOCK_VERSION` or `git describe --always --dirty`, pass the release tag from the release workflow, and verify a local build and a tagged build report different, correct strings.
- [ ] 5.8 Publish the remaining metadata from atomics and the boot-time OTA read, and verify the page and `/status` agree on every field that both expose.
- [ ] 5.9 Confirm the 3232 status document and `PUT /ota` are unchanged apart from the new resource fields, and verify an OTA upload succeeds while two event streams are open and the clients reconnect after the reboot.
- [ ] 5.10 Measure heap used and maximum, the main-stack high-water mark, `.bss` and `.stack` from the linked map, flash growth, and the static socket cost with three clients attached, record the numbers in `docs/esp32-c3.md`, and reduce the client pool to two if the measured heap or stack margin is insufficient.
- [ ] 5.11 Run a one-hour soak with a browser attached, and verify the clock keeps its 1 Hz cadence, the watchdog does not reset, no heap growth appears, and the resource metrics stay stable.

## 6. Visual and synchronization verification

- [ ] 6.1 Capture the page at 800×800 and compare it with the canonical front plate for grid position, tracking, and the three dots, and verify the screenshot is stored in `docs/images/web-clock-face.png`.
- [ ] 6.2 Verify a 360×640 phone viewport shows the whole face with no horizontal scroll, the metadata stacked below it, and the resource block readable.
- [ ] 6.3 Keep two browsers open across at least ten minute boundaries and verify both change within 2 seconds of the strip and of each other.
- [ ] 6.4 Verify the DST fall-back with a frozen time in the repeated hour, and confirm the two occurrences are distinguishable by `unix` while the page shows the correct local minute.
- [ ] 6.5 Verify the dark state by booting without a reachable access point and by exceeding the drift bound, and confirm the page shows no stale minute and explains why it is dark.
- [ ] 6.6 Check `lang="de"`, the accessible phrase, heading structure, keyboard focus, and metadata contrast against black, and record the result.
- [ ] 6.7 Watch the browser network panel for several minutes and verify there are no external requests and no polling requests beyond the event stream.
- [ ] 6.8 Watch the resource block for three health events and verify it updates without re-rendering the face and that the values change plausibly for load and idle.

## 7. Documentation and integration

- [ ] 7.1 Add a README section for the web face with the device URL, the host preview command, the parity test, and the health metrics, and verify the documented commands run as written.
- [ ] 7.2 Document the web port, routes, metadata, resource metrics, client limit, memory cost, and LAN-only security posture in `docs/esp32-c3.md`, and verify the guide matches the firmware behavior and the status table.
- [x] 7.3 Update the deferred observability item in `openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md` to include runtime flash, RAM, and CPU metrics and to point at the `resource-metrics` capability and `docs/metrics.md`.
- [ ] 7.4 Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, the firmware target build, `openspec validate add-web-clock-face --strict`, and `openspec status --change add-web-clock-face`, and verify all pass.
- [ ] 7.5 Verify the rendered page, event stream, and resource metrics against the simulator and the status document on a second machine on the LAN, not only on the development host.

## Workflow follow-up

- Archive the change after the implementation, the soak, and the documentation are complete.
- Verify the archived capabilities appear under `openspec/specs/web-clock-face/` and `openspec/specs/resource-metrics/`.
- When the later MQTT and Prometheus change is written, consume `docs/metrics.md` and do not redefine the metric names or units.
