# Design

Decision numbers here are local to this change; `replace-fadecandy-with-rust` uses its own D-series. `docs/decisions.md` gives both series stable, change-scoped IDs (`WCF-*`, `WCR-*`).

## Context

The selected host is the ESP32-C3-DevKit-RUST-1 over SPI2 with DMA, Wi-Fi, and SNTP (WCR-D16 and WCR-D19 in the archived `replace-fadecandy-with-rust` change). Its firmware already serves OTA and a plain-text status document on port 3232. That server accepts one connection at a time, owns a single 4 KiB RX/TX buffer pair, and closes each connection after the response, which is the right shape for `curl` and `espflash` but not for a long-lived event stream.

`clock_task` renders at 1 Hz from a crystal time base, recomputes the phrase and colours only when the local minute key changes, applies the gamma table, and writes the corrected physical frame. It owns the static `CLOCK_RAW` and `CLOCK_OUT` buffers and blanks the display after `BLANK_AFTER_SECS` without a fresh synchronization. The host simulator already produces the authoritative 18×16 output and corrected colours, and the firmware's frame check is byte-identical to `sim` for a fixed colour and seed.

The canonical front plate is the public `WordClock24h-Frontplatte-800x800.png` from mikrocontroller.net: an 800×800 black square, 16 rows of 18 uppercase letters in a wide-tracked sans-serif, the grid occupying roughly the central 65 % in both axes, and three small dots centred near the bottom.

WCR-D21 in the archived `replace-fadecandy-with-rust` change fixes the network posture: plaintext HTTP on the LAN, no TLS in the firmware, remote access by VPN. WCR-D22 fixes the semantic state that consumers read: the shown minute and colour, not physical pixels. This change inherits both.

The constraints that shape the design are the C3's roughly 50 KiB heap headroom after Wi-Fi, `StackResources<4>`, one core, and an OTA path that must remain reachable even when a browser misbehaves.

## Goals / Non-Goals

**Goals:**

- The page shows exactly the minute, phrase, and corrected colours the strip is showing, including per-pixel occasion frames and the dark state.
- The page updates without polling or reload and stays aligned across browser clock skew, reconnects, DST, and reboots.
- The page is recognizable as the canonical front plate, readable on a phone and a tablet, and self-contained.
- The web path never delays the LED frame, the status document, or the OTA recovery path.
- The same contract can be exercised on macOS without hardware.

**Non-Goals:**

- TLS, authentication, remote exposure, or a public URL.
- A command surface, MQTT, Prometheus, a log browser, or streaming physical pixels to third parties.
- Animations, fades, transitions, or a browser-side clock.
- Changing the words, colours, cadence, or physical output.
- Many concurrent viewers or a general-purpose web server.

## Decisions

### D1: Serve from the device, with a host twin

Production is the ESP32-C3. The page must show the colours the strip actually emits and the firmware that is actually running; only the device has both. A separate server would need to duplicate the colour picker and would drift, especially on per-pixel occasion days. The host `serve` command exists to develop and test the contract on macOS, not as a second production path. Both use the same `wordclock_core::web` renderer and event encoder, so the page cannot fork.

### D2: A separate web port, not the OTA port

The existing server accepts one connection at a time and owns one buffer pair. An SSE connection held open there would block status and OTA, and OTA is the only way back into a sealed clock. Serve the face on a new port, 3233, with its own listener, buffers, and client slots. Keep port 3232 and `GET /` exactly as documented, including the plain-text status document and `PUT /ota`.

The cost is one more port and one more listener's worth of memory. The benefit is that an open browser tab cannot strand the device. This is a deliberate duplication of the HTTP parsing rather than a refactor of the OTA server: the OTA path is proven and must not be disturbed for a read-only page.

### D3: Recreate the plate in HTML and CSS

The page is an 18×16 CSS grid of 288 spans generated from `clock::STRIPE`, uppercase, centred on a black square, with a wide-tracked local sans-serif and three decorative dots below the grid. Inline CSS and a small inline script only; no image, no web font, no CDN, no framework.

Alternatives: an SVG would work but adds markup for no gain and makes per-cell text updates less direct; a raster copy of the canonical image would blur, cannot recolour cells, and costs more flash than the markup; a `<pre>` block with spaces cannot reproduce the proportional sans-serif tracking of the plate.

The geometry follows the canonical image: a square plate, a grid that uses about 65 % of the width and height, and dots near 91 % of the height. `aspect-ratio: 1` plus `clamp()` sizing keeps the face square from a 360 px phone to a desktop, and a two-column layout puts the metadata beside the plate on wide screens and below it on narrow ones.

### D4: Colours match the simulator, not the plating

The physical plate is black with clear letter cutouts; the canonical image shows the cutouts as white because it is a mask. The page uses the simulator's convention: unlit letters are `#2b2b2b`, the dim grey `TerminalDisplay` uses (43, 43, 43), and lit letters use the gamma-corrected RGB the strip emits. The image is the authority for geometry and typography, not for lit colour. A user comparing the page to an unpowered clock therefore sees dim letters rather than white, which is the truthful off appearance and matches `cargo run -- sim`.

### D5: Server-owned time; the browser only renders

The initial HTML embeds the current state, and every SSE event carries the displayed local minute. The client never calls `new Date()` to decide what to show. Phones and tablets drift; the device is the clock. A page opened mid-minute paints that minute; a page opened during an unsynchronized or blanked period shows the dark state. The browser may use its own clock only for UI labels such as "last update 3 s ago", never to choose the minute.

### D6: Compact, semantic SSE events

`GET /events` responds with `Content-Type: text/event-stream; charset=utf-8` and `Cache-Control: no-store`. On connect it sends `retry: 2000`, then an immediate event:

```text
retry: 2000

id: 1762295820
event: frame
data: {"unix":1762295820,"local":"2026-11-04T22:37","dark":false,"phrase":"ESISTHALBSIEBEN","uniform":{"hex":"#aabbcc","name":"pale blue"},"cells":[[3,"#aabbcc"],[4,"#aabbcc"]]}

```

Every displayed-minute change sends one `frame` event; between changes the server sends `: keepalive` every 15 seconds.

`cells` contains only the lit logical indices in ascending order (11 to 39 entries) with their corrected `#rrggbb` values. This is the shown state, not a physical frame: no padding, no strip length, no byte order. Per-pixel occasion frames carry several colours and `uniform: null`. `unix` disambiguates the repeated local hour at the DST fall-back.

The face cannot be reconstructed from the phrase alone, because the stripe contains repeated words and the legacy lookup chooses among them; the lit indices are the smallest faithful representation. Sending all 288 logical colours would be about 2 KiB per minute and would reveal more structure than the face needs. This stays within D22's semantic-state principle: consumers read logical cells and a minute, not a transport frame.

### D7: A snapshot published by the clock task

`clock_task` already computes the local minute, the phrase, and the corrected frame. After it computes a new minute, and whenever the dark state changes, it publishes a `Shown` snapshot: the UTC instant, the local date and minute, a dark flag, the uniform colour and XKCD name when all lit cells share one, and up to 64 lit cells of `(logical_index, corrected_rgb)`.

The snapshot lives in a `critical_section::Mutex<RefCell<Shown>>`. The clock task builds the new snapshot in a local and swaps it under a short lock; it never formats JSON or allocates while holding the lock. The web task copies the snapshot out under the same lock and formats the event outside it. A compile-time test walks all 1,440 minutes and asserts that no frame exceeds the cell capacity.

Do not run a second colour picker in the web task: a second RNG would show different colours from the strip, which is exactly the failure this requirement exists to prevent. Reading `CLOCK_OUT` directly is not an alternative because it is owned by the clock task and covers the physical strip, not the logical face.

### D8: Fixed client pool and bounded work

The web server has one listener, at most three SSE clients, and transient short requests. Each SSE client owns a socket with small buffers, about 512 bytes RX and 1 KiB TX, so three clients cost roughly 4.5 KiB plus smoltcp socket state. `StackResources` rises from 4 to 8 to cover SNTP, OTA, the web listener, the clients, and a transient slot.

A fourth SSE connection receives `503` with `Retry-After: 15`; `EventSource` retries. Writes are bounded by a 10 second socket timeout, and an event that cannot be drained in that time drops the client. Short requests (`/`, `/status`, the `/events` handshake) are handled inline. The clock task never waits on the web task; the snapshot lock is held only for a copy. OTA is on a different socket and port and cannot be blocked by SSE.

The memory budget is measured, not assumed. The current firmware peaks at 54,432 bytes of a fixed 103,184-byte heap, leaving 48,752 bytes free at the peak. The web path adds static sockets, a listener, shared page strings in flash, and one shared static event buffer; the event is formatted once per minute and written to every client, so there is no per-client or per-request heap allocation. If the measured headroom is insufficient, reduce to two SSE clients before touching the OTA buffers or the clock task.

### D9: Metadata set and version identity

The page shows a compact metadata block beside or below the plate:

| Field | Source |
| :--- | :--- |
| Version | Release tag from `WORDCLOCK_VERSION`, otherwise `git describe --always --dirty` |
| Revision | The numeric `REVISION` the OTA workflow already bumps |
| Build | Target triple and build timestamp, from build-time environment |
| Uptime | Monotonic seconds since boot |
| Address | The IPv4 lease or static address |
| Wi-Fi | RSSI when the driver exposes it, omitted otherwise |
| Time source | SNTP synchronized or not, age of the last accepted reply, and the resulting dark state |
| OTA | Running slot and image state, read once at boot |
| Strip | Configured physical length and whether the SPI+DMA driver came up |
| Flash | Running app image bytes, app partition bytes, free bytes, and percent used |
| RAM | Heap used, maximum, and total; static `.data` + `.bss`; main-stack high-water and total |
| CPU | Busy percentage over the last 60 seconds, maximum since boot, and core frequency |
| Now | Local minute, zone (`Europe/Berlin`), phrase, colour hex, and XKCD name or "per-pixel" |
| Repository | `https://github.com/suhlig/wordclock-rs`, opened with `rel="noopener noreferrer"` |
| License | `MIT OR Apache-2.0` |

Fields that do not apply on the host are omitted rather than filled with a fake value. Resource fields that only the firmware can measure are marked not applicable on the host. `/status` on the web port returns the same fields as `text/plain; charset=utf-8` for scripts; the existing 3232 status document keeps its current fields and format, with the resource fields added from the same snapshot.

Version identity is what the running image was built from, not "latest". A page must not claim an update that did not boot. The release workflow passes the tag so a release image reports it; a local build reports the commit and a dirty marker.

### D10: Host preview server

`wordclock serve --bind 127.0.0.1:3233 [--color RRGGBB] [--seed N] [--frozen-time "DD.MM.YYYY HH:MM"]` serves the same routes and event format on macOS and Linux. Defaults are the wall clock, random colours, and a loopback bind; exposing it on the LAN requires an explicit `--bind 0.0.0.0:3233`, so a development server is not exposed by accident.

The implementation is `std::net::TcpListener` with one thread per connection, a shared snapshot behind a mutex, and a bounded channel per SSE client. No async runtime and no new dependency. A ticker thread renders at each minute boundary with the same `sim::render`, `ColorPicker`, and `GammaLut` the simulator uses. `--frozen-time` keeps the state fixed so tests can assert exact HTML and event payloads.

The parity harness is a test: for a fixed time, colour, and seed, the page's cells and the first event must equal the lit logical pixels and corrected colours from `sim`.

### D11: Accessibility and presentation details

`<html lang="de">`. The plate is a `role="img"` with an `aria-label` containing the phrase, or a grid whose cells are `aria-hidden` and whose phrase is exposed once as text. Metadata is a `<dl>` with a visible heading. Metadata text keeps at least 4.5:1 contrast against black. The page is dark-only, like the clock, and has no motion, so `prefers-reduced-motion` needs no special case.

The page uses `viewport-fit=cover`, `100dvh`, `aspect-ratio: 1`, and `clamp()` so the face fits without scrolling on a phone while remaining large on a desktop. Grid rows and columns are fixed, so a colour update cannot shift the layout. The three dots are decorative and `aria-hidden`.

The response sends a restrictive `Content-Security-Policy` (`default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`), `X-Content-Type-Options: nosniff`, and `Referrer-Policy: no-referrer`. This enforces self-containment rather than trusting the markup to stay asset-free.

### D12: Failure and reconnect behaviour

- SSE disconnect: `EventSource` retries after the advertised delay; the first event on the new connection replaces the whole state, so no delta is lost.
- Device reboot or OTA: the page shows a reconnecting state until the server returns, then catches up to the current minute. It never silently keeps the old minute.
- Network loss: the page cannot reach the device; it keeps retrying and shows the disconnected state.
- Device dark: the event has `dark: true` and no cells; the metadata says unsynchronized or stale, not a time.
- Pool full: the browser retries and the page reports that it is waiting, rather than showing an error dialog.
- No cookies, local storage, or service worker; there is no client-side state to go stale.

### D13: Measure resources at runtime

Flash is not consumed dynamically at runtime, but the honest usage figure still comes from the running image rather than a build constant. The firmware parses the ESP-IDF image header and segment table of the running app partition to get the image length, and reads the partition size from the partition table. This mirrors the private `get_image_metadata` in `esp-bootloader-esp-idf`, kept small and tested against a real image. A build-time constant would be stale after an OTA that changes the image size, and a local build has no release artifact to derive it from.

RAM has three separate answers, and the page shows all three:

- Heap: `esp_alloc::HEAP.stats()` for used, maximum, and total.
- Static: `_data_start`/`_data_end` plus `_bss_start`/`_bss_end` from the linker, which is `.data` plus `.bss` including the heap's backing arrays.
- Stack: `_stack_start`/`_stack_end` from the linker for the total, plus a high-water mark obtained by painting the unused main stack at boot and scanning it at most once per 60 seconds. The value is approximate and labelled as such; it must never be used for a safety decision.

CPU comes from idle accounting. Replace `esp_rtos::start` with `esp_rtos::start_with_idle_hook`, accumulate idle microseconds around `esp_hal::interrupt::wait_for_interrupt`, and compute `busy = 1 - idle / wall` over a 60-second window, keeping the maximum window since boot. Task and interrupt time count as busy; time waiting for an interrupt counts as idle. This is the definition the page and later publishers use, even though it is not a cycle-exact profile.

A low-priority metrics task samples heap and stack every 60 seconds, the flash figures once at boot because they cannot change without a reboot, and publishes a `ResourceMetrics` snapshot. The page, `/status`, and the event stream read that snapshot; they do not each parse flash or scan the stack.

Alternatives: build-time constants for flash (rejected, stale after OTA and absent for local builds); FreeRTOS run-time statistics (not available in the `esp-rtos` scheduler); sampling the program counter (unreliable and more intrusive); a single combined RAM percentage (rejected, because the heap's backing arrays are inside `.bss` and a naive sum double-counts them).

### D14: A separate health event with a stable contract

The event stream carries `event: health` immediately on connect and at least every 60 seconds:

```text
event: health
data: {"uptime_s":3600,"flash_app_used_bytes":615808,"flash_app_total_bytes":1966080,"flash_app_free_bytes":1350272,"flash_app_used_pct":31.3,"ram_heap_used_bytes":54312,"ram_heap_max_bytes":54432,"ram_heap_total_bytes":103184,"ram_heap_used_pct":52.7,"ram_static_bytes":132464,"ram_stack_used_bytes":6120,"ram_stack_total_bytes":188048,"ram_stack_used_pct":3.3,"cpu_busy_pct":1.4,"cpu_busy_max_pct":12.0,"cpu_freq_mhz":160}

```

`health` is separate from `frame`. The face event stays small and changes once per minute; health changes slowly and is useful to a scraper or a dashboard on its own schedule. The page embeds the current health snapshot in the initial HTML, so the metadata block is correct before the first event, and the immediate `health` event confirms it. On the page, the `health` handler updates only the metadata block, so the face is never re-rendered for a metric change.

The metric names, units, and meanings are fixed in `docs/metrics.md`:

| Name | Unit | Meaning |
| :--- | :--- | :--- |
| `flash_app_used_bytes` | bytes | Running app image length |
| `flash_app_total_bytes` | bytes | Running app partition size |
| `flash_app_free_bytes` | bytes | Partition size minus image length |
| `flash_app_used_pct` | percent | Image length divided by partition size |
| `ram_heap_used_bytes` | bytes | Current heap usage |
| `ram_heap_max_bytes` | bytes | Peak heap usage since boot |
| `ram_heap_total_bytes` | bytes | Total heap regions |
| `ram_heap_used_pct` | percent | Heap used divided by heap total |
| `ram_static_bytes` | bytes | `.data` plus `.bss` |
| `ram_stack_used_bytes` | bytes | Approximate main-stack high-water |
| `ram_stack_total_bytes` | bytes | Main-stack region |
| `ram_stack_used_pct` | percent | High-water divided by stack total |
| `cpu_busy_pct` | percent | Busy time over the last 60 seconds |
| `cpu_busy_max_pct` | percent | Highest 60-second value since boot |
| `cpu_freq_mhz` | MHz | Core clock |
| `uptime_s` | seconds | Seconds since boot |

The later MQTT publisher uses the same JSON keys and units in a retained health document; the topic belongs to the MQTT change. The later Prometheus endpoint exposes each name as a `wordclock_`-prefixed gauge with the same unit. This change does not implement either transport; it fixes the contract so the later work cannot silently redefine a metric.

Why `docs/metrics.md` and not just the page: a dashboard, an alert, and a Home Assistant sensor will outlive this page. Field names are cheap to fix now and expensive to change after a panel or an automation depends on them.

### D15: Keep diagnostics out of the shown-state document

WCR-D22 in the archived `replace-fadecandy-with-rust` change defines a retained shown-state document for Home Assistant and mirrors: the local minute, the phrase, the colour, the mode, and the dark flag. Runtime resource metrics are not shown state. They go in the separate health payload above, with a separate MQTT document and Prometheus gauges, so a Home Assistant entity does not gain a dozen diagnostic attributes and a mirror does not receive them as part of the clock's appearance.

The shared `ResourceMetrics` type and the names in `docs/metrics.md` are the single source; the page, the status document, the future MQTT health document, and the future Prometheus endpoint all serialize that type rather than re-deriving the numbers.

This is consistent with D22's rule that consumers read semantic state rather than frames: diagnostics are their own semantic document.

## Resource budget

The baseline is the current release firmware on this machine and the 2026-10-25 bench measurements in `docs/esp32-c3-spike.md`. Where the two differ, the larger current image is used, so the estimate is conservative.

| Resource | Measured baseline | Web addition | Headroom | Verdict |
| :--- | ---: | ---: | ---: | :--- |
| Flash image | 615,808 B of a 1,966,080 B OTA slot (31.3 %) | 8–15 KiB of page, CSS, JS, and event code in `.rodata`/`.text` | 1,350,272 B free | Non-issue; about 32–33 % of the slot used |
| DRAM regions | 320,512 B `DRAM` plus 66,320 B `dram2_seg` | 10–14 KiB of `.bss` for four socket slots, `StackResources<8>`, one shared event buffer, one status buffer, the metrics snapshot, and task state | `.stack` is the remainder of `DRAM` after statics; it is 188,048 B today and about 174–178 KiB after the addition | Fits; the linker's minimum main-stack assertion still has a large margin |
| Heap | 103,184 B fixed in two regions (66,320 B reclaimed + 36,864 B in `.bss`); 54,432 B peak, 50,308 B after Wi-Fi | 0 B if page, event, and status use static buffers; allow up to 4 KiB for Wi-Fi/TCP per-connection internals | 48,752 B free at the baseline peak | About 44–48 KiB free; comfortable for three clients |
| CPU | 160 MHz single core; 11.5 ms/s SPI wire time for 288 pixels (1.15 %), 13.3 ms/s for 335, DMA-driven | Under 1 ms per minute for the event, 12 B every 15 s per heartbeat, a one-time ~10 KB page stream, and a health event of under 1 KiB per minute | One core | Average well under 1.5 %; DMA keeps LED timing independent of web load |
| Metrics sampling | 0 B and 0 tasks before this change | One 60 s metrics task, O(1) heap stats, one 188 KiB stack scan per minute, and two timer reads per idle wake in the idle hook | Stated bound: under 1 % average CPU and under 1 KiB heap | Within the bound; the stack scan is the largest single operation and runs at most once per minute |
| Sockets | `StackResources<4>` | Four more slots, about 200 B of smoltcp state each | `StackResources<8>` for SNTP, OTA, the listener, three clients, and a transient slot | One spare slot |

The heap figure is fixed by the two `esp_alloc::heap_allocator!` calls, so new static buffers come out of the `.stack` remainder rather than shrinking the heap. Conversely, new per-connection allocations in the Wi-Fi or TCP stack come out of the heap. The static addition is the more predictable of the two; the per-connection heap is the item to measure. The health payload is under 1 KiB and is sent once a minute plus once per connection, so it adds well under 1 % to the event stream.

What remains to be measured on the board, not inferred from the map:

- Heap used and maximum with three event streams, one page load, and an OTA upload in progress.
- The main-stack high-water mark with the web task active. The event and status formatters write into static buffers rather than large stack arrays, so the expected growth is under 1 KiB.
- The actual `.bss`, `.stack`, and heap sizes from the linked map, and whether the Wi-Fi driver allocates per connection.
- A one-hour soak for heap growth and frame cadence.

## Risks / Trade-offs

| Risk | Mitigation |
| :--- | :--- |
| The C3 lacks RAM or sockets for a listener plus three clients | Measure heap and `StackResources`; drop to two clients first; the OTA server keeps its own buffers and port. |
| An SSE client blocks the network stack | Bounded writes with a 10 s timeout, small per-client buffers, a fixed pool, and a drop policy for stalled clients. |
| The clock task stalls on the snapshot lock | Copy-only critical section, no formatting or allocation while locked, and a fixed-size snapshot. |
| The page drifts from the strip | The snapshot is published by the task that drives the strip, and a host test compares the page with `sim` for fixed inputs. |
| Per-pixel occasion colours make events large | At most 39 cells, roughly 1 KiB; the heartbeat is a comment. |
| DST repeats a local hour | Every event carries `unix`; the page shows the local minute and a consumer can disambiguate. |
| Fonts differ between devices and the plate | Use a local sans-serif stack, keep the grid geometry fixed, and accept small metric differences. |
| Browsers cache an old page across a firmware update | `Cache-Control: no-store` on HTML and status; `no-store` on the stream. |
| The page exposes operational data on the LAN | No secrets, read-only methods only, and D21's trusted-LAN posture; the same data is already on the status document. |
| OTA and the web server compete for the single core | The web path is event-driven and idle between minutes and heartbeats; OTA remains a separate task. |
| The stack high-water is approximate and racy | Paint at boot, scan a snapshot at most once per minute, label it approximate, and never use it for a safety decision. |
| The image-header parser drifts from the ESP-IDF format | Test against a real image and a deliberately truncated one; report unknown rather than a wrong number. |
| The custom idle hook interferes with low-power behaviour | The current build does not enable automatic light sleep; keep the hook to a timer read plus `wait_for_interrupt`, and revisit if light sleep is enabled. |
| The CPU figure is read as application CPU | Define it as total busy time including interrupts and document that meaning on the page and in `docs/metrics.md`. |
| Metric names drift between the page, MQTT, and Prometheus | Fix the contract in `docs/metrics.md`, serialize one shared `ResourceMetrics` type, and test that `/status` and the page agree. |
| The flash figure is mistaken for whole-chip free space | Label it as the running app partition and show the slot total next to it. |

## Migration Plan

This change is additive. The new port and routes appear after an OTA update; the existing 3232 status and OTA endpoints keep their responses. Roll back by flashing the previous image; there is no configuration, data, or wiring migration. Update the README, `docs/esp32-c3.md`, and `docs/metrics.md`, and add `WORDCLOCK_VERSION` to the release build so the page identifies the tag.

## References

- Canonical front plate: <https://www.mikrocontroller.net/wikifiles/1/12/WordClock24h-Frontplatte-800x800.png>
- `wordclock-core/src/clock.rs` for `STRIPE`, `pixels`, and `words`.
- `wordclock-core/src/color.rs` for `GammaLut`, `ColorPicker`, and the XKCD names.
- `wordclock-host/src/display.rs` for the terminal grid and the dim grey 43,43,43.
- `wordclock-host/src/sim.rs` for `render`, `parse_time`, and the demo picker behaviour.
- `wordclock-esp32/src/main.rs` for `clock_task`, `serve`, `status_document`, and `REVISION`.
- `esp_alloc::HEAP.stats()` for heap used, maximum, and total.
- `esp-hal` linker symbols `_data_start`, `_data_end`, `_bss_start`, `_bss_end`, `_stack_start`, and `_stack_end` in `ld/sections/rwdata.x` and `ld/sections/stack.x`.
- `esp_hal::interrupt::wait_for_interrupt` and `esp_rtos::start_with_idle_hook` for idle accounting.
- `esp-bootloader-esp-idf`'s image metadata parser for the ESP-IDF header and segment table that this change reimplements in miniature.
- `docs/esp32-c3-spike.md` for the baseline heap, flash, frame-time, and current measurements.
- `openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md` WCR-D21 and WCR-D22 for the network posture and semantic state.
