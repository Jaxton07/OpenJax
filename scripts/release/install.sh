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
WEB_SRC_DIR="${SCRIPT_DIR}/web"

if [[ ! -d "${BIN_SRC_DIR}" ]]; then
  echo "[install] error: missing bin directory beside install.sh"
  exit 1
fi
if [[ ! -d "${WEB_SRC_DIR}" ]]; then
  echo "[install] error: missing web directory beside install.sh"
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
mkdir -p "${PREFIX}/web"
cp "${BIN_SRC_DIR}/tui_next" "${PREFIX}/bin/tui_next"
cp "${BIN_SRC_DIR}/openjaxd" "${PREFIX}/bin/openjaxd"
cp "${BIN_SRC_DIR}/openjax-gateway" "${PREFIX}/bin/openjax-gateway"
cp -R "${WEB_SRC_DIR}/." "${PREFIX}/web/"
chmod +x "${PREFIX}/bin/tui_next" "${PREFIX}/bin/openjaxd" "${PREFIX}/bin/openjax-gateway"

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

echo "[install] run checks:"
echo "  ${PREFIX}/bin/tui_next --help"
echo "  ${PREFIX}/bin/openjaxd --help"
echo "  ${PREFIX}/bin/openjax-gateway --help"
echo "[install] gateway serves web from: ${PREFIX}/web"
