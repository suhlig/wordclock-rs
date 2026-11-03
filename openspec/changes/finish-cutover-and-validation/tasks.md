# Tasks

Carried over from `openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/tasks.md`. The original task number is in parentheses; nothing here adds behavior beyond what the archived specs already require.

## 1. Bench verification of the accepted behavior

- [ ] 1.1 (was 4.12) Verify D20 on the installed image with the access point switched off across a power cycle, and with a short outage below the 24 h bound; record the observed dark and recovery times.

## 2. Physical mapping and measurement on the real chain

- [ ] 2.1 (was 5B.4) Run walking-pixel, row-color, primary-color, and all-on patterns on the real strip; identify the physical order, orientation, and whether the LEDs beyond 288 are connected.
- [ ] 2.2 (was 5B.5) Record the verified mapping in the configuration and hardware guide.
- [ ] 2.3 (was 5B.6) Measure the SPI frame time and bitstream with the Pico analyzer at the 5 V side; confirm the four-bit cell, 288 pixels, GRB order, and a gap after a `1` of at least 800 ns.
- [ ] 2.4 (was 5B.7) If SPI is unreliable, lower the clock only within the datasheet window or check the shifter and wiring; do not reintroduce RMT for the strip (WCR-D19). Document the result before rejecting the C3 path.

## 3. Shared daemon logic and runtime configuration

- [ ] 3.1 (was 6A.1) Implement the minute scheduler and per-minute color cadence behind the clock and display traits; test them on the host with a fake time source.
- [ ] 3.2 (was 6A.2) Implement the time-source gate behavior with a fake synchronized/unsynchronized source; test the neutral frame and the first synchronized frame.
- [ ] 3.3 (was 6C.2) Move strip length, birthday list, and the D20 bound out of compile-time constants into the runtime configuration, keeping Wi-Fi credentials in the existing flash record.
- [ ] 3.4 (was 6C.8) Add the daemon state-machine tests that are still missing; host tests and the on-device smoke test at revision 28 already pass.

## 4. Provisioning

- [ ] 4.1 (was 8B.3) Run the blank-board USB serial provisioning against the second board when it arrives; the NVS load path is already verified at revision 32.

## 5. Cutover and validation

- [ ] 5.1 (was 9.1) Capture fixed-color reference photos of the legacy clock at 00:00, 00:06, 01:05, 11:15, 12:15, 13:15, and 23:55.
- [ ] 5.2 (was 9.2) Render the same times and colors with the selected host and compare the words, pixel positions, and gamma-corrected colors side by side.
- [ ] 5.3 (was 9.3) Run the selected host for 24 hours and confirm every minute updates, no minute is skipped, and the color changes once per minute.
- [ ] 5.4 (was 9.4) Reboot the selected host and confirm the neutral/NTP gate, then service start and the correct first frame.
- [ ] 5.5 (was 9.5) Disconnect the network after startup and confirm the clock keeps rendering within the D20 bound.
- [ ] 5.6 (was 9.6) Power-cycle the C3 and confirm the neutral gate, then recovery to the current time.
- [ ] 5.7 (was 9.7) Run the rollback drill: previous OTA image or USB reflash, plus the legacy Pi clock.
- [ ] 5.8 (was 9.8) After at least two weeks of stable operation, remove Fadecandy from the signal path and retire the board; keep the previous firmware as the last-resort image.

## 6. Documentation

- [ ] 6.1 (was 10.1) Add the configuration, deployment, and troubleshooting sections to the README, and verify the documented commands run as written.
- [ ] 6.2 (was 10.4) Record the verified physical mapping and pixel counts in the hardware guide.
- [ ] 6.3 (was 10.6) Create the remaining follow-up proposals for the deferred ideas: smooth fade, night dimming, MQTT/Prometheus observability, and sensors.

## Workflow follow-up

- Archive with `openspec archive finish-cutover-and-validation -y --skip-specs` once every task is done. No capability spec changes on archive.
