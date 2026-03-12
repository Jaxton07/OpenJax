# OpenJax Deployment Guide (Runtime First)

Chinese version: [deployment.zh-CN.md](deployment.zh-CN.md)

This guide targets runtime users on:

- macOS arm64 (Apple Silicon)
- Linux x86_64

Runtime install does not require Rust, Node, or Python on the user machine.

## Package Contents

Each release tarball includes:

- `bin/tui_next`
- `bin/openjaxd`
- `bin/openjax-gateway`
- `web/` (prebuilt static UI)
- `install.sh`
- `uninstall.sh`
- `README-install.md`

`openjax-gateway` serves `web/` by default from `<install_prefix>/web`.

## A. Offline Install (Recommended)

1. Download release package from GitHub Releases:
- `openjax-v<version>-macos-aarch64.tar.gz`
- `openjax-v<version>-linux-x86_64.tar.gz`

2. Verify checksum (optional but recommended):

```bash
shasum -a 256 openjax-v<version>-<platform>.tar.gz
```

3. Extract and install:

```bash
tar -xzf openjax-v<version>-<platform>.tar.gz
cd openjax-v<version>-<platform>
./install.sh --prefix "$HOME/.local/openjax"
```

4. Add PATH and run:

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
openjax-gateway
```

Open `http://127.0.0.1:8765/login` and enter the owner key shown in the gateway terminal output.
Web will call `/api/v1/auth/login` to obtain access/refresh tokens. Refresh token is stored in HttpOnly cookie.

## B. Online Install from GitHub Release (Optional)

One-liner:

```bash
curl -fsSL https://raw.githubusercontent.com/Jaxton07/OpenJax/main/scripts/release/install_from_github.sh | bash -s -- --yes
```

From repository checkout:

```bash
bash scripts/release/install_from_github.sh --yes
```

Options:

- `--version 0.2.6` install a specific tag (`v0.2.6`)
- `--prefix <path>` set install prefix
- `--repo owner/name` install from another repository

The script downloads artifact + `SHA256SUMS`, verifies checksum, then runs package `install.sh`.

## C. Uninstall

From package directory:

```bash
./uninstall.sh
```

Keep userdata if present:

```bash
./uninstall.sh --keep-user-data
```

## D. Upgrade

Online upgrade (latest release):

```bash
bash scripts/release/upgrade.sh --yes
```

Upgrade from a local package tarball:

```bash
bash scripts/release/upgrade.sh --from-package /path/to/openjax-v<version>-<platform>.tar.gz --yes
```

The upgrade script stops running `openjax-gateway` / `openjaxd` / `tui_next` by default, then installs and validates the new binaries.

## E. Build and Package Locally (Maintainers)

macOS arm64:

```bash
make doctor
make build-release-mac
make package-mac
```

Linux x86_64:

```bash
make doctor
make build-release-linux
make package-linux
```

## F. CI/CD Release Flow

- CI (`.github/workflows/ci.yml`) validates Rust, Web, and Linux install/uninstall package smoke tests.
- Release (`.github/workflows/release.yml`) runs on tags `v*`, builds macOS/Linux packages, verifies install, and uploads assets to GitHub Release.

## G. Development Environment Notes (Not Required for Runtime Users)

Only contributors need Rust/Node. Optional mirror settings for faster dependency fetch in mainland China:

```bash
# cargo
export CARGO_NET_RETRY=5
export CARGO_HTTP_MULTIPLEXING=false

# npm/pnpm (example)
npm config set registry https://registry.npmmirror.com
pnpm config set registry https://registry.npmmirror.com
```
