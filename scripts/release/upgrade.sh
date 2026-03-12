#!/usr/bin/env bash
set -euo pipefail

PREFIX="${HOME}/.local/openjax"
VERSION="latest"
REPO="${OPENJAX_GITHUB_REPO:-Jaxton07/OpenJax}"
FROM_PACKAGE=""
ASSUME_YES=0
SKIP_STOP=0

usage() {
  cat <<USAGE
Usage: upgrade.sh [--prefix <path>] [--version <version|latest>] [--repo <owner/name>] [--from-package <tar.gz>] [--skip-stop] [-y]

Options:
  --prefix <path>       Install prefix (default: ~/.local/openjax)
  --version <value>     Target version for online upgrade (default: latest)
  --repo <owner/name>   GitHub repository for online upgrade (default: Jaxton07/OpenJax)
  --from-package <tar>  Upgrade from a local package tar.gz instead of GitHub download
  --skip-stop           Do not stop running openjax processes before upgrade
  -y, --yes             Skip confirmation prompt
  -h, --help            Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$2"
      shift 2
      ;;
    --version)
      VERSION="$2"
      shift 2
      ;;
    --repo)
      REPO="$2"
      shift 2
      ;;
    --from-package)
      FROM_PACKAGE="$2"
      shift 2
      ;;
    --skip-stop)
      SKIP_STOP=1
      shift
      ;;
    -y|--yes)
      ASSUME_YES=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[upgrade] unknown argument: $1"
      usage
      exit 1
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "[upgrade] missing required command: $cmd"
    exit 1
  fi
}

stop_processes() {
  local found=0
  for name in openjax-gateway openjaxd tui_next; do
    if pgrep -f "$name" >/dev/null 2>&1; then
      found=1
      break
    fi
  done

  if [[ "$found" -eq 0 ]]; then
    return 0
  fi

  if [[ "$ASSUME_YES" -ne 1 ]]; then
    echo "[upgrade] running OpenJax processes detected. stop them now? [Y/n]"
    read -r reply
    if [[ "$reply" =~ ^[Nn]$ ]]; then
      echo "[upgrade] aborted: please stop services before upgrading"
      exit 0
    fi
  fi

  for name in openjax-gateway openjaxd tui_next; do
    pkill -f "$name" 2>/dev/null || true
  done
}

run_checks() {
  local base="$1"
  test -x "$base/bin/tui_next"
  test -x "$base/bin/openjaxd"
  test -x "$base/bin/openjax-gateway"
  test -f "$base/web/index.html"

  "$base/bin/tui_next" --help >/dev/null
  "$base/bin/openjaxd" --help >/dev/null
  "$base/bin/openjax-gateway" --help >/dev/null
}

if [[ "$ASSUME_YES" -ne 1 ]]; then
  echo "[upgrade] upgrade OpenJax under: ${PREFIX} ? [y/N]"
  read -r reply
  if [[ ! "$reply" =~ ^[Yy]$ ]]; then
    echo "[upgrade] aborted"
    exit 0
  fi
fi

if [[ "$SKIP_STOP" -ne 1 ]]; then
  stop_processes
fi

if [[ -n "$FROM_PACKAGE" ]]; then
  require_cmd tar
  if [[ ! -f "$FROM_PACKAGE" ]]; then
    echo "[upgrade] package not found: $FROM_PACKAGE"
    exit 1
  fi

  TMP_DIR="$(mktemp -d)"
  cleanup() {
    rm -rf "$TMP_DIR"
  }
  trap cleanup EXIT

  tar -xzf "$FROM_PACKAGE" -C "$TMP_DIR"
  PKG_DIR="$(find "$TMP_DIR" -maxdepth 1 -type d -name "openjax-v*" | head -n1)"
  if [[ -z "$PKG_DIR" ]]; then
    echo "[upgrade] failed to extract package from: $FROM_PACKAGE"
    exit 1
  fi
  bash "$PKG_DIR/install.sh" --prefix "$PREFIX" --yes
else
  bash "$SCRIPT_DIR/install_from_github.sh" --prefix "$PREFIX" --version "$VERSION" --repo "$REPO" --yes
fi

run_checks "$PREFIX"

echo "[upgrade] upgrade completed successfully"
echo "[upgrade] start services as needed:"
echo "  openjax-gateway"
echo "  tui_next"
