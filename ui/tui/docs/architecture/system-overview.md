# System Overview

## Purpose
Define the high-level structure of the new TUI runtime.

## Scope
Modules under `ui/tui/src` and their runtime responsibilities.

## Decisions (Locked)
- `custom_terminal` is the only viewport/cursor source of truth.
- `insert_history` is the only scrollback insertion entrypoint.
- `app` produces separated history and live content.

## Open Questions
- None.

## Validation
- Module boundaries are reflected in `ui/tui/src/lib.rs`.

## Last Updated
2026-03-01

## Runtime Layers
1. App/state: owns message model and history commit boundaries.
2. TUI orchestrator: applies draw lifecycle and queues history insertion.
3. Terminal core: tracks viewport area and cursor snapshots.
4. Insert engine: writes committed history cells into scrollback.
