# OpenJax

<p align="center">
  <strong>A Rust-first, general-purpose AI assistant runtime for local and cloud workflows.</strong><br/>
  Built around tool calling, sandboxing, and approval control.
</p>

<p align="center">
  <a href="https://github.com/Jaxton07/OpenJax"><img alt="GitHub Repo" src="https://img.shields.io/badge/GitHub-Repo-181717?logo=github"></a>
  <a href="https://github.com/Jaxton07/OpenJax/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/License-MIT-green.svg"></a>
  <a href="https://github.com/Jaxton07/OpenJax/commits/main"><img alt="Last Commit" src="https://img.shields.io/github/last-commit/Jaxton07/OpenJax"></a>
  <a href="https://github.com/Jaxton07/OpenJax/stargazers"><img alt="Stars" src="https://img.shields.io/github/stars/Jaxton07/OpenJax?style=social"></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="OVERVIEW.md">Overview</a> |
  <a href="CONTRIBUTING.md">Contributing</a> |
  <a href="SECURITY.md">Security</a> |
  <a href="docs/deployment.md">Deployment</a>
</p>

## Contents

- [Highlights](#highlights)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Configuration](#configuration)
- [Architecture](#architecture)
- [Repository Layout](#repository-layout)
- [Development](#development)
- [Documentation](#documentation)
- [Security](#security)
- [Contributing](#contributing)

## Highlights

- General-purpose assistant loop for coding, automation, and everyday workflows
- Tooling for file read/search, shell execution, and patch application
- Sandbox modes and approval policies
- Rust TUI runtime (`tui_next`) as the primary interactive interface
- Multi-model support through pluggable provider configuration

## Quick Start

### Prerequisites

- Rust toolchain (`cargo`, `rustup`)
- `zsh`
- `OPENAI_API_KEY` (or another provider key supported by your configuration)

### Run from source

```bash
make doctor
make run-tui
```

Equivalent direct command:

```bash
cargo run -q -p tui_next
```

## Installation

### Option A: Source install (local repo, one command)

```bash
make install-source
```

Use this if you are already in a cloned OpenJax repository.

### Option B: Prebuilt package (macOS ARM / Linux x86_64 / Windows x86_64)

Build package locally (example: macOS ARM):

```bash
make doctor
make build-release-mac
make package-mac
```

Then install from package directory:

```bash
./install.sh --prefix "$HOME/.local/openjax"
```

Add to `PATH` and launch:

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
tui_next
```

For Linux/Windows package commands and full deployment flow, see [docs/deployment.md](docs/deployment.md).

## Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENJAX_MODEL` | Model backend | `gpt-4.1-mini` |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `OPENJAX_KIMI_API_KEY` | Kimi API key | - |
| `OPENJAX_GLM_API_KEY` | GLM API key | - |
| `OPENJAX_ANTHROPIC_API_KEY` | Claude API key | - |
| `OPENJAX_APPROVAL_POLICY` | Approval level | `on_request` |
| `OPENJAX_SANDBOX_MODE` | Sandbox mode | `workspace_write` |

If no config file exists, OpenJax auto-generates a template at:
- `~/.openjax/config.toml`

## Architecture

```text
User (Rust TUI / Web UI)
        |
        v
openjaxd (daemon)
        |
        v
openjax-core (agent loop, tools, sandbox, approval)
        |
        v
openjax-protocol (shared types/events)
```

## Repository Layout

- `openjax-core/`: agent loop, tools, sandbox, approvals
- `openjax-protocol/`: protocol/event/data types
- `openjaxd/`: daemon runtime
- `ui/tui/`: Rust TUI (`tui_next`)
- `openjax-gateway/`: HTTP/SSE gateway for Web clients
- `ui/web/`: React Web UI
- `python/openjax_sdk/`: async Python SDK
- `smoke_test/`: smoke scripts

## Development

Run from repository root:

```bash
make run-web-dev
zsh -lc "cargo build"
zsh -lc "cargo fmt -- --check"
zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"
zsh -lc "cargo test --workspace"
```

`make run-web-dev` starts `openjax-gateway` and `ui/web` together for local web development.

For integration tests in `tests/`, use explicit test target form:

```bash
zsh -lc "cargo test -p openjax-core --test m3_sandbox"
```

## Documentation

- Overview: [OVERVIEW.md](OVERVIEW.md)
- Deployment: [docs/deployment.md](docs/deployment.md)
- Chinese Deployment Guide: [docs/deployment.zh-CN.md](docs/deployment.zh-CN.md)
- Security model: [docs/security.md](docs/security.md)

## Security

Please read [SECURITY.md](SECURITY.md) before reporting vulnerabilities.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
