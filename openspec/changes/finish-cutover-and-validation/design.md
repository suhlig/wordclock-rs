# Design

## Context

This change is the continuation of `replace-fadecandy-with-rust`, which was archived on 2026-11-03 with 21 tasks still open. Its capabilities and requirements are already accepted and live under `openspec/specs/`; nothing here changes them.

## Goals / Non-Goals

Goals: finish and verify the cutover, and keep the remaining work visible in `openspec list` instead of only in the archive.

Non-goals: new behavior, new capabilities, or reopening any accepted decision.

## Decisions

None. Every decision this work depends on is already recorded, with stable IDs, in [`docs/decisions.md`](../../../docs/decisions.md): the selected host (WCR-D16), the SPI-with-DMA transport (WCR-D19), the sync gate (WCR-D20), and the network posture (WCR-D21).

## Risks / Trade-offs

- The work spans hardware, firmware, and installation, so tasks stay open for reasons outside the repository (a second board on order, a 24-hour soak, a two-week stability window). The task list keeps those reasons inline.
- `skip_specs: true` means archiving this change does not touch `openspec/specs/`. If an implementation step reveals a requirement that the accepted specs get wrong, that belongs in a separate change with a real delta, not in this tracking change.
