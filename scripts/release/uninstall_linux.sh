#!/usr/bin/env bash
set -euo pipefail

PREFIX="${HOME}/.local/openjax"
KEEP_USER_DATA=0
ASSUME_YES=0

usage() {
  cat <<USAGE
Usage: ./uninstall.sh [--prefix <path>] [--keep-user-data] [-y]

Options:
  --prefix <path>      Install prefix (default: ~/.local/openjax)
  --keep-user-data     Keep <prefix>/userdata if present
  -y, --yes            Skip confirmation prompt
  -h, --help           Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$2"
      shift 2
      ;;
    --keep-user-data)
      KEEP_USER_DATA=1
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
      echo "[uninstall] unknown argument: $1"
      usage
      exit 1
      ;;
  esac
done

if [[ ! -e "${PREFIX}" ]]; then
  echo "[uninstall] nothing to remove at ${PREFIX}"
  exit 0
fi

if [[ ${ASSUME_YES} -ne 1 ]]; then
  if [[ ${KEEP_USER_DATA} -eq 1 ]]; then
    echo "[uninstall] remove OpenJax under ${PREFIX} but keep userdata? [y/N]"
  else
    echo "[uninstall] remove ALL OpenJax files under ${PREFIX}? [y/N]"
  fi
  read -r reply
  if [[ ! "${reply}" =~ ^[Yy]$ ]]; then
    echo "[uninstall] aborted"
    exit 0
  fi
fi

if [[ ${KEEP_USER_DATA} -eq 1 && -d "${PREFIX}/userdata" ]]; then
  TMP_DIR="$(mktemp -d)"
  mv "${PREFIX}/userdata" "${TMP_DIR}/userdata"
  rm -rf "${PREFIX}"
  mkdir -p "${PREFIX}"
  mv "${TMP_DIR}/userdata" "${PREFIX}/userdata"
  rmdir "${TMP_DIR}" || true
  echo "[uninstall] removed binaries/configs and kept ${PREFIX}/userdata"
else
  rm -rf "${PREFIX}"
  if [[ ${KEEP_USER_DATA} -eq 1 ]]; then
    echo "[uninstall] removed ${PREFIX} (no userdata to keep)"
  else
    echo "[uninstall] removed ${PREFIX}"
  fi
fi
