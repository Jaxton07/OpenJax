# Contributing to OpenJax

Thanks for contributing. This repository is Rust-first with Python MVP components.

## Development Setup
- Use Rust stable (workspace edition is 2024).
- Use Node.js + `pnpm` for Web UI related changes.
- Use Python 3.10+ for SDK related changes.
- Run commands from repository root.

### Prerequisites Checklist
- Rust toolchain (`rustup`, `cargo`)
- `bash` (`zsh` recommended in this repo)
- Node.js + `pnpm` (for `ui/web`)
- Python 3.10+ (for `python/openjax_sdk`)
- Model provider credentials (for example `OPENAI_API_KEY`)

## Build
```bash
zsh -lc "cargo build"
```

## Run (Contributor Workflows)
```bash
zsh -lc "make run-web-dev"
zsh -lc "make run-tui"
```

- `make run-web-dev` starts `openjax-gateway` and `ui/web` together.
- `make run-tui` starts the Rust TUI directly.

## Format and Lint
```bash
zsh -lc "cargo fmt -- --check"
zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"
```

## Test
```bash
zsh -lc "cargo test --workspace"
```

For integration tests under `tests/`, prefer explicit test targets, for example:
```bash
zsh -lc "cargo test -p openjax-core --test m3_sandbox"
```

## Coding Guidelines
- Follow existing module boundaries and naming patterns.
- Prefer typed structures/enums over stringly-typed state.
- Avoid `unwrap()` in production paths.
- Add or update tests for behavior changes.

## Pull Requests
- Keep changes focused and atomic.
- Include test evidence (commands and results).
- Describe behavior changes and migration impact (if any).
