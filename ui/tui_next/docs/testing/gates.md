# Gates

## Purpose
Define mandatory validation gates before switching default runtime to `next`.

## Scope
Automated and manual checks for `ui/tui_next`.

## Decisions (Locked)
- Switch requires passing build/test and manual scenario gates.
- `tui_next` is the active runtime; gates prevent regressions during ongoing refactors.

## Open Questions
- Final threshold for long-session stress runs.

## Validation
- Automated: `cargo test -p tui_next`
- Manual:
  - 20-turn conversation, no duplicate entries.
  - Long Chinese response, no visual corruption.
  - Tool call + approval flow renders and resolves correctly.

## Last Updated
2026-03-01
