# OpenJax TUI

A modern terminal UI for OpenJax built with Textual.

## Overview

OpenJax TUI provides a rich, interactive terminal interface for the OpenJax AI agent framework. It replaces the previous prompt_toolkit-based implementation with a modern Textual-based UI.

## Features

- Modern terminal UI with Textual framework
- Screen-based navigation
- Command palette with fuzzy search
- Rich markdown rendering
- Responsive layout

## Installation

```bash
pip install -e .
```

## Usage

```bash
# Run as module
python -m openjax_tui

# Or use the command
openjax-tui
```

## Logging

- Default log file: `.openjax/logs/openjax_tui.log`
- Rotation: keep 5 backups, single file default max `2 MiB`
- Env vars:
  - `OPENJAX_TUI_LOG_DIR` override log directory
  - `OPENJAX_TUI_LOG_MAX_BYTES` override single-file max bytes
  - `OPENJAX_TUI_DEBUG=1` enable debug-level logs

## Development

```bash
# Install with dev dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Run linting
ruff check .
mypy src/openjax_tui
```

## License

MIT
