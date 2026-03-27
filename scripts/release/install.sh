#!/usr/bin/env bash
set -euo pipefail

PREFIX="${HOME}/.local/openjax"
ASSUME_YES=0
NO_MODIFY_PATH=0

usage() {
  cat <<USAGE
Usage: ./install.sh [--prefix <path>] [-y] [--no-modify-path]

Options:
  --prefix <path>     Install prefix (default: ~/.local/openjax)
  -y, --yes           Skip confirmation prompt
  --no-modify-path    Do not modify shell rc file (PATH must be set manually)
  -h, --help          Show this help
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
    --no-modify-path)
      NO_MODIFY_PATH=1
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
cp "${BIN_SRC_DIR}/openjax" "${PREFIX}/bin/openjax"
cp -R "${WEB_SRC_DIR}/." "${PREFIX}/web/"
chmod +x "${PREFIX}/bin/tui_next" "${PREFIX}/bin/openjaxd" "${PREFIX}/bin/openjax-gateway" "${PREFIX}/bin/openjax"

echo "[install] done: ${PREFIX}/bin"

_write_path_to_rc() {
  local bin_dir="$1"
  local shell_name
  shell_name="$(basename "${SHELL:-}")"
  local rc_file=""
  case "$shell_name" in
    zsh)  rc_file="${HOME}/.zshrc" ;;
    bash) rc_file="${HOME}/.bashrc" ;;
  esac

  if [[ -z "$rc_file" ]]; then
    echo "[install] shell '${shell_name}' not recognized; add to PATH manually:"
    echo "  export PATH=\"${bin_dir}:\$PATH\""
    return
  fi

  if grep -qF "$bin_dir" "$rc_file" 2>/dev/null; then
    echo "[install] PATH entry already present in ${rc_file}"
    return
  fi

  printf '\n# OpenJax\nexport PATH="%s:$PATH"\n' "$bin_dir" >> "$rc_file"
  echo "[install] added PATH entry to ${rc_file}"
  echo "[install] run: source ${rc_file}  (or restart your terminal)"
}

if [[ "$NO_MODIFY_PATH" -eq 1 ]]; then
  case ":${PATH}:" in
    *":${PREFIX}/bin:"*)
      echo "[install] PATH already includes ${PREFIX}/bin"
      ;;
    *)
      echo "[install] add to PATH manually:"
      echo "  export PATH=\"${PREFIX}/bin:\$PATH\""
      ;;
  esac
else
  _write_path_to_rc "${PREFIX}/bin"
fi

echo "[install] run checks:"
echo "  ${PREFIX}/bin/openjax --version"
echo "  ${PREFIX}/bin/tui_next --help"
echo "  ${PREFIX}/bin/openjaxd --help"
echo "  ${PREFIX}/bin/openjax-gateway --help"
echo "[install] gateway serves web from: ${PREFIX}/web"
echo "[install] to update in future: openjax update"
