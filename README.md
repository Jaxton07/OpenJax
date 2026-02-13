# OpenJax

A CLI agent framework based on Rust, enabling AI models to interact with codebases. Built as a simplified reference implementation inspired by [Codex](https://github.com/anthropics/codex).

## Features

- **Agent Loop**: Model-driven tool calling loop (up to 5 tool calls per turn)
- **Tool System**: Read files, list directories, grep search, execute commands, apply patches
- **Sandbox**: Two modes - `workspace_write` (restricted) and `danger_full_access`
- **Approval Policy**: Three levels - `always_ask`, `on_request`, `never`
- **Multi-Agent**: Protocol reserved for future sub-agent support

## Quick Start

```bash
# Build
cargo build

# Run CLI (uses environment variables or defaults)
cargo run -p openjax-cli

# Or with specific settings
cargo run -p openjax-cli -- --model echo --approval never
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENJAX_MODEL` | Model backend | `gpt-4.1-mini` or `codex-MiniMax-M2.1` |
| `OPENJAX_MINIMAX_API_KEY` | MiniMax API key | - |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `OPENJAX_APPROVAL_POLICY` | Approval level | `on_request` |
| `OPENJAX_SANDBOX_MODE` | Sandbox mode | `workspace_write` |

## CLI Options

```
OpenJax - A CLI agent framework

Usage: openjax-cli [OPTIONS]

Options:
      --model <MODEL>        模型后端: minimax | openai | echo
      --approval <APPROVAL>  审批策略: always_ask | on_request | never
      --sandbox <SANDBOX>   沙箱模式: workspace_write | danger_full_access
      --config <CONFIG>     配置文件路径
  -h, --help                Print help
  -V, --version            Print version
```

## Tool Syntax

```bash
# Read file
tool:read_file path=src/lib.rs

# List directory
tool:list_dir path=.

# Search files
tool:grep_files pattern=fn main path=.

# Execute command
tool:exec_command cmd='ls -la' require_escalated=true timeout_ms=60000

# Apply patch
tool:apply_patch patch='*** Begin Patch
*** Add File: hello.txt
+hello
*** End Patch'
```

## Patch Format

```
*** Begin Patch
*** Add File: <path>
+<line>
+<line>
*** Update File: <path>
@@
 <context line>
-<old line>
+<new line>
*** Delete File: <path>
*** Move File: <from> -> <to>
*** Rename File: <old> -> <new>
*** End Patch
```

## Security

See [docs/security.md](docs/security.md) for security model details.

## Architecture

See [docs/openjax-vs-codex-comparison.md](docs/openjax-vs-codex-comparison.md) for architecture comparison with Codex.

## License

MIT
