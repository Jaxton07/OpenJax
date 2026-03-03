#!/usr/bin/env bash
set -euo pipefail

PREFIX="${HOME}/.local/openjax"
ASSUME_YES=0

usage() {
  cat <<USAGE
Usage: ./install.sh [--prefix <path>] [-y]

Options:
  --prefix <path>   Install prefix (default: ~/.local/openjax)
  -y, --yes         Skip confirmation prompt
  -h, --help        Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$2"
      shift 2
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
      echo "[install] unknown argument: $1"
      usage
      exit 1
      ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "[install] error: this prebuilt package supports only macOS ARM (Darwin arm64)."
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_SRC_DIR="${SCRIPT_DIR}/bin"

if [[ ! -d "${BIN_SRC_DIR}" ]]; then
  echo "[install] error: missing bin directory beside install.sh"
  exit 1
fi

if [[ ${ASSUME_YES} -ne 1 ]]; then
  echo "[install] install OpenJax binaries to: ${PREFIX}/bin ? [y/N]"
  read -r reply
  if [[ ! "${reply}" =~ ^[Yy]$ ]]; then
    echo "[install] aborted"
    exit 0
  fi
fi

mkdir -p "${PREFIX}/bin"
cp "${BIN_SRC_DIR}/tui_next" "${PREFIX}/bin/tui_next"
cp "${BIN_SRC_DIR}/openjax-cli" "${PREFIX}/bin/openjax-cli"
cp "${BIN_SRC_DIR}/openjaxd" "${PREFIX}/bin/openjaxd"
chmod +x "${PREFIX}/bin/tui_next" "${PREFIX}/bin/openjax-cli" "${PREFIX}/bin/openjaxd"

echo "[install] done: ${PREFIX}/bin"

case ":${PATH}:" in
  *":${PREFIX}/bin:"*)
    echo "[install] PATH already includes ${PREFIX}/bin"
    ;;
  *)
    echo "[install] add to PATH if needed:"
    echo "  export PATH=\"${PREFIX}/bin:\$PATH\""
    ;;
esac
