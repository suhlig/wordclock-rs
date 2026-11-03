# Decisions

One index for the design decisions in this repository. OpenSpec numbers decisions per change (`D1`, `D2`, …), so three changes produce three `D1`s. This page gives every decision a stable, change-scoped ID and a status, so a reference can be followed after its change is archived, and so "decided" is never mistaken for "implemented".

Status vocabulary:

- **accepted** — in force and implemented in the selected build.
- **accepted (fallback)** — in force only for the Raspberry Pi rollback path, which is not executed.
- **superseded** — replaced by another decision; the original text is kept for history.
- **planned** — decided and written down, no implementation yet.
- **planned (partly shipped)** — decided, and only the named part exists today.

OpenSpec's own record stays authoritative: `openspec/specs/` holds the accepted capabilities and their requirements, `openspec/changes/` holds the changes that are still planned, and each change's `design.md` holds the full rationale. This page is the map, not a copy.

## replace-fadecandy-with-rust

Archived 2026-11-03. Rationale: [`design.md`](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md).

| ID | Status | Decision |
| :--- | :--- | :--- |
| WCR-D1 | accepted | [Rewrite in Rust in a new public repository](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d1-rewrite-in-rust-in-a-new-repository): `github.com/suhlig/wordclock-rs`, dual-licensed, legacy repo untouched as the oracle. |
| WCR-D2 | superseded by WCR-D16 | [Treat the Pi 3B as the incumbent host candidate](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d2-treat-the-pi-3b-as-the-incumbent-host-candidate). |
| WCR-D3 | accepted (fallback) | [Drive the strip from SPI0 with an external 74AHCT125 level shifter on the Pi](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d3-drive-the-strip-from-spi0-with-an-external-level-shifter-on-the-pi). |
| WCR-D4 | accepted | [Run as a long-running service, not a per-minute cron job](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d4-run-as-a-long-running-service-on-the-selected-host); on the C3 this is the firmware loop. |
| WCR-D5 | accepted | [Keep the core pure and put hardware behind a display trait](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d5-keep-the-core-pure-and-put-hardware-behind-a-display-trait). |
| WCR-D6 | accepted | [Crate choices](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d6-crate-choices): the small maintained crates, pinned. |
| WCR-D7 | accepted (fallback) | [SPI0 output details on the Pi](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d7-spi-output-details-on-the-pi): mode 0, 2.5 MHz, verify the bit period. |
| WCR-D8 | accepted | [Preserve the colour pipeline, not necessarily Fadecandy's dithering](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d8-preserve-the-color-pipeline-not-necessarily-fadecandys-dithering). |
| WCR-D9 | accepted | [Preserve the per-minute colour cadence and occasion semantics](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d9-preserve-the-per-minute-color-cadence-and-occasion-semantics). |
| WCR-D10 | accepted | [Use the legacy implementation as a golden oracle](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d10-use-the-legacy-implementation-as-a-golden-oracle): 1,440-minute fixture with a pinned SHA-256. |
| WCR-D11 | superseded by WCR-D20 | [Gate the first render on NTP](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d11-gate-the-first-render-on-ntp) on the Pi; the general rule is now D20. |
| WCR-D12 | planned (partly shipped) | [Build in CI and deploy with Ansible](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d12-build-in-ci-and-deploy-with-ansible): CI is in force, the Ansible path is not executed. |
| WCR-D13 | accepted | [Verify the physical order before trusting identity mapping](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d13-verify-the-physical-order-before-trusting-identity-mapping); the recorded mapping is still open (archived task 5B.4). |
| WCR-D14 | accepted | [Rollback is a card swap, not a code revert](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d14-rollback-is-a-card-swap-not-a-code-revert). |
| WCR-D15 | accepted (fallback) | [Show an explicit shutdown pattern instead of clearing to black](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d15-show-an-explicit-shutdown-pattern-instead-of-clearing-to-black); the C3 cannot guarantee it on power loss. |
| WCR-D16 | accepted | [Decide the host with a measured ESP32-C3 spike](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d16-decide-the-host-with-a-measured-esp32-c3-spike): the C3 was selected 2026-11-03. |
| WCR-D17 | accepted | [ESP32-C3 output and time](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d17-esp32-c3-output-and-time): SPI2 with DMA on GPIO3, Wi-Fi/SNTP, Berlin DST rule. |
| WCR-D18 | accepted | [ESP32-C3 lifecycle and deployment](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d18-esp32-c3-lifecycle-and-deployment): `esp-hal` with `esp-rtos`/`embassy`, pinned toolchain, A/B OTA. |
| WCR-D19 | accepted | [Drive the ESP32-C3 strip over SPI with DMA, not RMT](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d19-drive-the-esp32-c3-strip-over-spi-with-dma-not-rmt). |
| WCR-D20 | accepted | [Never show a time that was not synchronized](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d20-never-show-a-time-that-was-not-synchronized): dark at boot and after the drift bound. |
| WCR-D21 | planned | [Plain MQTT on the LAN, and no TLS in the firmware](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d21-plain-mqtt-on-the-lan-and-no-tls-in-the-firmware); no MQTT transport exists yet. |
| WCR-D22 | planned | [Publish the shown state, not the frame](../openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/design.md#d22-publish-the-shown-state-not-the-frame); the snapshot arrives with `add-web-clock-face`, the retained document with the later MQTT change. |

Caveats: the archived change was closed with open tasks, recorded in its archived `tasks.md` and carried into [`finish-cutover-and-validation`](../openspec/changes/finish-cutover-and-validation/tasks.md). They are the cutover and bench-verification items: the physical mapping (5B.4–5B.5), the bitstream measurement at 5 V (5B.6), the blank-board serial provisioning run (8B.3), the D20 power-cycle check (4.12), the daemon state-machine tests (6C.8), and the cutover validation (9.1–9.8). The accepted capabilities describe the system as agreed, including the documented Pi rollback path; they do not all claim a bench-verified installation.

## add-web-clock-face

Planned, not implemented. Rationale: [`design.md`](../openspec/changes/add-web-clock-face/design.md). Contract: `openspec/changes/add-web-clock-face/specs/`.

| ID | Status | Decision |
| :--- | :--- | :--- |
| WCF-D1 | planned | [Serve from the device, with a host twin](../openspec/changes/add-web-clock-face/design.md#d1-serve-from-the-device-with-a-host-twin). |
| WCF-D2 | planned | [A separate web port, not the OTA port](../openspec/changes/add-web-clock-face/design.md#d2-a-separate-web-port-not-the-ota-port), so SSE cannot block an update. |
| WCF-D3 | planned | [Recreate the plate in HTML and CSS](../openspec/changes/add-web-clock-face/design.md#d3-recreate-the-plate-in-html-and-css) from `clock::STRIPE`; no image, font, CDN, or framework. |
| WCF-D4 | planned | [Colours match the simulator, not the plating](../openspec/changes/add-web-clock-face/design.md#d4-colours-match-the-simulator-not-the-plating); unlit is `#2b2b2b`. |
| WCF-D5 | planned | [Server-owned time; the browser only renders](../openspec/changes/add-web-clock-face/design.md#d5-server-owned-time-the-browser-only-renders). |
| WCF-D6 | planned | [Compact, semantic SSE events](../openspec/changes/add-web-clock-face/design.md#d6-compact-semantic-sse-events). |
| WCF-D7 | planned | [A snapshot published by the clock task](../openspec/changes/add-web-clock-face/design.md#d7-a-snapshot-published-by-the-clock-task). |
| WCF-D8 | planned | [Fixed client pool and bounded work](../openspec/changes/add-web-clock-face/design.md#d8-fixed-client-pool-and-bounded-work): three SSE clients, 503 beyond that. |
| WCF-D9 | planned | [Metadata set and version identity](../openspec/changes/add-web-clock-face/design.md#d9-metadata-set-and-version-identity). |
| WCF-D10 | planned | [Host preview server](../openspec/changes/add-web-clock-face/design.md#d10-host-preview-server): `wordclock serve` with fixed colour, seed, and frozen time. |
| WCF-D11 | planned | [Accessibility and presentation details](../openspec/changes/add-web-clock-face/design.md#d11-accessibility-and-presentation-details). |
| WCF-D12 | planned | [Failure and reconnect behaviour](../openspec/changes/add-web-clock-face/design.md#d12-failure-and-reconnect-behaviour). |
| WCF-D13 | planned | [Measure resources at runtime](../openspec/changes/add-web-clock-face/design.md#d13-measure-resources-at-runtime). |
| WCF-D14 | planned | [A separate health event with a stable contract](../openspec/changes/add-web-clock-face/design.md#d14-a-separate-health-event-with-a-stable-contract). |
| WCF-D15 | planned | [Keep diagnostics out of the shown-state document](../openspec/changes/add-web-clock-face/design.md#d15-keep-diagnostics-out-of-the-shown-state-document). |

## add-power-budget-and-current-sensing

Planned. Rationale: [`design.md`](../openspec/changes/add-power-budget-and-current-sensing/design.md). Contract: `openspec/changes/add-power-budget-and-current-sensing/specs/`.

| ID | Status | Decision |
| :--- | :--- | :--- |
| PWR-D1 | planned | [Bound the commanded current at 9 A](../openspec/changes/add-power-budget-and-current-sensing/design.md#d1-bound-the-commanded-current-at-9-a): 75 % of the 12 A supply. |
| PWR-D2 | planned | [Apply one uniform scale after gamma and whitepoint correction](../openspec/changes/add-power-budget-and-current-sensing/design.md#d2-apply-one-uniform-scale-after-gamma-and-whitepoint-correction). |
| PWR-D3 | planned | [Estimate from quiescent plus drive, calibrated rather than assumed](../openspec/changes/add-power-budget-and-current-sensing/design.md#d3-estimate-from-quiescent-plus-drive-calibrated-rather-than-assumed): 11/255 mA per count, not the datasheet's 20/255. |
| PWR-D4 | planned | [Use the existing ACS712-20A rather than buying a sensor](../openspec/changes/add-power-budget-and-current-sensing/design.md#d4-use-the-existing-acs712-20a-rather-than-buying-a-sensor). |
| PWR-D5 | planned | [Divide the sensor output and read it on ADC1](../openspec/changes/add-power-budget-and-current-sensing/design.md#d5-divide-the-sensor-output-and-read-it-on-adc1): 10 k/10 k and 100 nF. |
| PWR-D6 | planned | [Power the sensor from a stable 5 V, not the measured rail](../openspec/changes/add-power-budget-and-current-sensing/design.md#d6-power-the-sensor-from-a-stable-5-v-not-the-measured-rail). |
| PWR-D7 | planned | [Slow outer loop with hysteresis, never per frame](../openspec/changes/add-power-budget-and-current-sensing/design.md#d7-slow-outer-loop-with-hysteresis-never-per-frame). |
| PWR-D8 | planned | [Keep the existing distribution, thicken only the trunk if needed](../openspec/changes/add-power-budget-and-current-sensing/design.md#d8-keep-the-existing-distribution-thicken-only-the-trunk-if-needed). |
| PWR-D9 | planned (partly shipped) | [One 5 V supply for everything, firmware updates over OTA](../openspec/changes/add-power-budget-and-current-sensing/design.md#d9-one-5-v-supply-for-everything-firmware-updates-over-ota): OTA is in production, the single-rail build and its verification are not. |

## Conventions for new decisions

- Reference a decision by its scoped ID (`WCR-D20`, `WCF-D7`, `PWR-D2`) in code comments, documents, and other change artifacts. A bare `D20` is ambiguous across changes.
- Add a row here when a change is proposed, not after it is implemented. Mark it `planned`, then update the status when the work lands.
- Do not copy the rationale here. Link to the `design.md` section; the archived change keeps its `design.md` forever.
