# Contributing to OpenJax

Thanks for your interest in contributing! OpenJax is a Rust-first agent framework. This guide covers everything you need to get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Report Issues](#how-to-report-issues)
- [Development Setup](#development-setup)
- [Build & Run](#build--run)
- [Format, Lint & Test](#format-lint--test)
- [Coding Guidelines](#coding-guidelines)
- [Branch & Commit Conventions](#branch--commit-conventions)
- [Pull Request Process](#pull-request-process)
- [License](#license)

---

## Code of Conduct

Be respectful and constructive. We welcome contributors of all backgrounds. Harassment, discrimination, or personal attacks will not be tolerated. When in doubt, assume good intent and communicate openly.

---

## How to Report Issues

### Bug Reports

Open a GitHub Issue and include:

- A clear, descriptive title
- Steps to reproduce the problem
- Expected vs. actual behavior
- Environment details (OS, Rust version, relevant env vars)
- Relevant logs or error output

### Feature Requests

Open a GitHub Issue with the `enhancement` label and describe:

- The problem you are trying to solve
- Your proposed solution or behavior
- Any alternatives you considered

> **Tip:** For significant changes, open an issue to discuss the approach before writing code. This avoids wasted effort if the direction does not fit the project.

---

## Development Setup

### Prerequisites

| Tool | Purpose |
|------|---------|
| Rust stable (`rustup`, `cargo`) | Core codebase |
| `zsh` | Recommended shell for running project commands |
| Node.js + `pnpm` | Web UI (`ui/web`) |
| Python 3.10+ | SDK (`python/openjax_sdk`) |
| Model provider credentials | e.g. `OPENAI_API_KEY` |

### Environment Variables

Copy any needed credentials into your shell environment. Never commit secrets.

```bash
export OPENAI_API_KEY=sk-...
export OPENJAX_MODEL=gpt-4o          # optional
export OPENJAX_SANDBOX_MODE=...      # optional
export OPENJAX_GATEWAY_BIND=...      # optional
```

---

## Build & Run

Run all commands from the **repository root**.

```bash
# Build all crates
cargo build

# Build a specific crate
cargo build -p openjax-core
cargo build -p openjax-gateway

# Start gateway + web UI together (development)
make run-web-dev

# Start TUI
make run-tui
```

> When previewing the web UI locally, use `http://127.0.0.1:<port>` instead of `localhost`.

---

## Format, Lint & Test

All checks must pass before a PR can be merged.

```bash
# Format check
cargo fmt -- --check

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Run all tests
cargo test --workspace

# Run a specific integration suite (preferred for tests/ directory)
cargo test -p openjax-core --test tools_sandbox_suite
cargo test -p openjax-core --test approval_suite

# Run with output for debugging
cargo test -p openjax-core -- --nocapture

# Web UI tests
cd ui/web && pnpm test

# Python SDK tests
PYTHONPATH=python/openjax_sdk/src python3 -m unittest discover -s python/openjax_sdk/tests -v
```

---

## Coding Guidelines

- Follow existing module boundaries and naming conventions.
- Rust: `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Prefer typed structs and enums over stringly-typed state.
- Avoid `unwrap()` in production paths; use `Result<T, E>` with proper context.
- Use `anyhow::Result` at service boundaries, `thiserror` for structured domain errors.
- Keep files focused: aim for under 500 lines, avoid exceeding 800 lines.
- Add or update tests for every behavior change. Cover happy path and failure/edge cases.
- Python: follow PEP 8, include type annotations on all public and internal functions.

---

## Branch & Commit Conventions

### Branching

- Base all work off `main`.
- Use descriptive branch names with a type prefix:
  - `feat/short-description`
  - `fix/short-description`
  - `docs/short-description`
  - `refactor/short-description`
  - `chore/short-description`

### Commit Messages

Follow the **emoji + Conventional Commits** style used in this project's history:

```
<emoji> <type>(<scope>): <Chinese or English summary>
```

Common types and their emoji:

| Type | Emoji | When to use |
|------|-------|-------------|
| `feat` | ✨ | New feature |
| `fix` | 🐛 | Bug fix |
| `docs` | 📝 | Documentation only |
| `refactor` | ♻️ | Code restructure without behavior change |
| `chore` | 🔧 | Build, tooling, CI |
| `test` | ✅ | Adding or updating tests |

Examples:

```
✨ feat(gateway): 支持 SSE 事件断线重连
🐛 fix(store): 修复会话删除未落盘问题
📝 docs(contributing): 补充 PR 流程说明
```

Rules:
- Keep commits atomic — one logical change per commit.
- Use `git add <specific-file>` — never `git add .` or `git add -A`.
- Append the following trailer to every commit:
  ```
  Co-Authored-By: <your-name> <your-email>
  ```

---

## Pull Request Process

### Before Opening a PR

1. Ensure you are not on `main`.
2. Sync with latest main:
   ```bash
   git fetch origin main
   git rebase origin/main
   ```
3. Run format, lint, and tests locally and confirm they pass.

### Opening the PR

- **Base branch:** always `main`
- **Title:** match your commit message style, keep it under 70 characters
- **Body:** must include two sections:

```
## Summary
- What changed and why (1–3 bullet points)

## Test Plan
- [ ] Step 1: command or action
- [ ] Step 2: expected result
```

### After Opening

- A maintainer will review your PR. Please respond to feedback promptly.
- Keep the PR updated if `main` moves forward — rebase, do not merge.
- Squash commits if requested during review.

---

## Getting Help

- **Questions / Discussion:** open a GitHub Issue or Discussion
- **Stuck on a review comment?** Leave a question in the PR thread

---

## License

By contributing to OpenJax, you agree that your contributions will be licensed under the [MIT License](LICENSE). No CLA is required.
