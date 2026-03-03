# OpenJax

A Rust-first CLI/TUI agent framework for AI-assisted coding workflows, inspired by Codex-style tool calling, sandboxing, and approval control.

## Features

- Agent loop with tool-calling orchestration
- Tooling for file read/search, shell execution, and patch application
- Sandbox modes and approval policies
- Rust TUI runtime (`tui_next`) as the primary interactive interface

## Install

### 1) Prebuilt install (macOS ARM only)

`install.sh` is included in the prebuilt package. Follow these steps.

#### Step A: Get the prebuilt package

Option 1 (build package locally):

```bash
make doctor
make build-release-mac
make package-mac
```

After packaging, the artifact is at `dist/openjax-v<version>-macos-aarch64.tar.gz`.

Option 2 (download from release page):
- Download `openjax-v<version>-macos-aarch64.tar.gz` to a local directory.

#### Step B: Extract and enter package directory

```bash
cd dist
TAR_FILE=$(ls openjax-v*-macos-aarch64.tar.gz | head -n1)
tar -xzf "$TAR_FILE"
DIR_NAME=$(basename "$TAR_FILE" .tar.gz)
cd "$DIR_NAME"
```

#### Step C: Run installer script

```bash
./install.sh
```

Optional: custom install prefix

```bash
./install.sh --prefix "$HOME/.local/openjax"
```

#### Step D: Add PATH and launch

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
tui_next
```

For persistent PATH, add the same `export PATH=...` line to your shell profile (for example `~/.zshrc`).

### 2) Source install (macOS / Linux / Windows)

```bash
make install-source
```

For platform-specific source-install commands, see [Deployment Guide](docs/deployment.md).
For Chinese instructions, see [Chinese Deployment Guide](docs/deployment.zh-CN.md).

## Quick Run

```bash
make run-tui
```

Or run directly:

```bash
cargo run -q -p tui_next
```

## Uninstall

```bash
# Run inside the extracted prebuilt package directory
./uninstall.sh

# Or run from the repository root
make uninstall-local
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENJAX_MODEL` | Model backend | `gpt-4.1-mini` |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `OPENJAX_KIMI_API_KEY` | Kimi API key | - |
| `OPENJAX_GLM_API_KEY` | GLM API key | - |
| `OPENJAX_ANTHROPIC_API_KEY` | Claude API key | - |
| `OPENJAX_APPROVAL_POLICY` | Approval level | `on_request` |
| `OPENJAX_SANDBOX_MODE` | Sandbox mode | `workspace_write` |

If no config file exists, OpenJax auto-generates a default template on startup at `./.openjax/config/config.toml` (fallback: `~/.openjax/config.toml`).

## Deployment and Security

- Deployment details: [docs/deployment.md](docs/deployment.md)
- Chinese deployment guide: [docs/deployment.zh-CN.md](docs/deployment.zh-CN.md)
- Security model: [docs/security.md](docs/security.md)
- Architecture comparison: [docs/openjax-vs-codex-comparison.md](docs/openjax-vs-codex-comparison.md)

## License

MIT
