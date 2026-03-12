# OpenJax Deployment Guide (Short-Term)

Chinese version: [deployment.zh-CN.md](deployment.zh-CN.md)

This guide defines the current deployment standard:

1. Prebuilt packages for **macOS ARM / Linux x86_64 / Windows x86_64**
2. Source install for **macOS / Linux / Windows**
3. One-command uninstall with optional `--keep-user-data`

## Constraints and Decisions

- Prebuilt targets:
  - `macOS arm64 (Apple Silicon)`
  - `Linux x86_64`
  - `Windows x86_64`
- Default install prefix: `~/.local/openjax`
- Distribution mode: manual packaging and manual upload
- Uninstall default: remove all files under `~/.local/openjax`
- Forward-compatible flag: `--keep-user-data`

## A. Prebuilt Install (macOS ARM)

`install.sh` is inside the package and performs the actual installation.

### Step A: Get package

Option 1: build locally from repo

```bash
make doctor
make build-release-mac
make package-mac
```

Output artifacts:

- `dist/openjax-v<version>-macos-aarch64.tar.gz`
- `dist/SHA256SUMS`

Option 2: download `openjax-v<version>-macos-aarch64.tar.gz` from your release channel.

### Step B: Extract and enter package directory

```bash
cd dist
TAR_FILE=$(ls openjax-v*-macos-aarch64.tar.gz | head -n1)
tar -xzf "$TAR_FILE"
DIR_NAME=$(basename "$TAR_FILE" .tar.gz)
cd "$DIR_NAME"
```

### Step C: Run installer script

```bash
./install.sh
```

Optional custom prefix:

```bash
./install.sh --prefix "$HOME/.local/openjax"
```

### Step D: Add PATH and run

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
tui_next
```

For persistent PATH, add the `export` line to your shell profile (for example `~/.zshrc`).

### Verify binaries

```bash
test -x "$HOME/.local/openjax/bin/tui_next"
openjaxd --help
```

## B. Prebuilt Install (Linux x86_64)

Build package locally:

```bash
make build-release-linux
make package-linux
```

Install from package directory:

```bash
cd dist
TAR_FILE=$(ls openjax-v*-linux-x86_64.tar.gz | head -n1)
tar -xzf "$TAR_FILE"
DIR_NAME=$(basename "$TAR_FILE" .tar.gz)
cd "$DIR_NAME"
./install.sh
```

## C. Prebuilt Install (Windows x86_64)

Build package in PowerShell:

```powershell
cargo build --release --locked -p tui_next -p openjaxd
powershell -ExecutionPolicy Bypass -File scripts/release/package_windows.ps1
```

Install from package directory:

```powershell
Expand-Archive .\dist\openjax-v<version>-windows-x86_64.zip -DestinationPath .\dist -Force
cd .\dist\openjax-v<version>-windows-x86_64
.\install.ps1
```

## D. Source Install (Local Repo, One Command)

Use this when you are already in a cloned OpenJax repository:

```bash
make install-source
```

## E. Source Install (Clone + Manual Steps)

### macOS / Linux (bash/zsh)

```bash
git clone <your-repo-url> openJax
cd openJax
cargo build --release --locked -p tui_next -p openjaxd
mkdir -p "$HOME/.local/openjax/bin"
cp target/release/tui_next "$HOME/.local/openjax/bin/tui_next"
cp target/release/openjaxd "$HOME/.local/openjax/bin/openjaxd"
chmod +x "$HOME/.local/openjax/bin/tui_next" "$HOME/.local/openjax/bin/openjaxd"
```

### Windows (PowerShell)

```powershell
git clone <your-repo-url> openJax
cd openJax
cargo build --release --locked -p tui_next -p openjaxd
$prefix = Join-Path $HOME ".local/openjax/bin"
New-Item -ItemType Directory -Force -Path $prefix | Out-Null
Copy-Item "target/release/tui_next.exe" (Join-Path $prefix "tui_next.exe") -Force
Copy-Item "target/release/openjaxd.exe" (Join-Path $prefix "openjaxd.exe") -Force
```

## F. Uninstall

### Default full cleanup

```bash
./uninstall.sh
```

Or from repo:

```bash
make uninstall-local
```

Keep userdata via Makefile:

```bash
make uninstall-local KEEP_USER_DATA=1
```

### Keep user data directory (future-compatible)

```bash
./uninstall.sh --keep-user-data
```

Behavior today:

- If `<prefix>/userdata` exists, it is preserved.
- If it does not exist, result is equivalent to full cleanup.

## G. Weak-Network Suggestions

```bash
export CARGO_NET_RETRY=5
export CARGO_HTTP_MULTIPLEXING=false
```

Optional first step before build:

```bash
cargo fetch --locked
```

## H. Release SOP (Manual)

1. `make doctor`
2. build one target:
- macOS ARM: `make build-release-mac && make package-mac`
- Linux x86_64: `make build-release-linux && make package-linux`
- Windows x86_64: `cargo build --release --locked -p tui_next -p openjaxd` then `powershell -ExecutionPolicy Bypass -File scripts/release/package_windows.ps1`
3. Fresh-folder validation:
- unpack package
- run install script (`install.sh` or `install.ps1`)
- verify `tui_next` exists and `--help` for `openjaxd`
- run uninstall script (`uninstall.sh` or `uninstall.ps1`)
4. Upload package (`.tar.gz`/`.zip`) + `SHA256SUMS`
