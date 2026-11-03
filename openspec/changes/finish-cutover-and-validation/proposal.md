# Proposal

## Why

`replace-fadecandy-with-rust` was archived on 2026-11-03 with its accepted capabilities merged into `openspec/specs/`, but 21 of its tasks were still open. They are not new behavior: they are the bench checks, the recorded physical mapping, the runtime configuration that is still compiled in, and the cutover validation that proves the installed clock over 24 hours and a rollback drill. Leaving them only inside `openspec/changes/archive/2026-11-03-replace-fadecandy-with-rust/tasks.md` hides real planned work from `openspec list`.

## What Changes

- Carry the open items from the archived change into one active change so `openspec list` shows the remaining work, with the original task numbers preserved for traceability.
- Add no requirements. The behavior was accepted with the archived change; this change only completes and verifies it. `skip_specs: true` records that.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. `clock-daemon`, `deployment`, and `led-output` already describe the target behavior; the open tasks verify or complete the implementation described there.

## Impact

- Firmware and host code: the shared daemon logic behind the clock and display traits (`wordclock-host/src/daemon.rs` is still a stub), the runtime configuration fields that are currently compiled in, and the daemon state-machine tests.
- Bench and installation: the D20 power-cycle check, the physical mapping on the real 335-pixel chain, the bitstream measurement at 5 V, and the blank-board serial provisioning run once the second board arrives.
- Documentation: the README configuration, deployment, and troubleshooting sections, and the recorded mapping in the hardware guide.
- No change to the words, colours, cadence, gamma, network posture, or the selected host.
