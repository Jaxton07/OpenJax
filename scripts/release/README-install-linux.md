# OpenJax Install (Linux x86_64)

This package is prebuilt for **Linux x86_64** only.

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
