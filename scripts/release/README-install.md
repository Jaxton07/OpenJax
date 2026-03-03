# OpenJax Install (macOS ARM)

This package is prebuilt for **macOS Apple Silicon (arm64)** only.

## Install

```bash
./install.sh
```

Custom prefix:

```bash
./install.sh --prefix "$HOME/.local/openjax"
```

## Verify

```bash
test -x "$HOME/.local/openjax/bin/tui_next"
openjax-cli --help
openjaxd --help
```

## Uninstall

```bash
./uninstall.sh
```

Keep future user data directory (if present):

```bash
./uninstall.sh --keep-user-data
```
