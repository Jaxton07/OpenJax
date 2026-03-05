# Contributing to OpenJax

Thanks for contributing. This repository is Rust-first with Python MVP components.

## Development Setup
- Use Rust stable (workspace edition is 2024).
- Use Python 3.10+ for SDK/TUI related changes.
- Run commands from repository root.

## Build
```bash
zsh -lc "cargo build"
```

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
