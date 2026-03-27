#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_DIR="${REPO_ROOT}/target/release"
DIST_DIR="${REPO_ROOT}/dist"
WEB_DIST_DIR="${REPO_ROOT}/ui/web/dist"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "[package] error: Linux x86_64 is required for this package target."
  exit 1
fi

TAG_VERSION="$(git -C "${REPO_ROOT}" describe --tags --exact-match 2>/dev/null || true)"
if [[ -n "${TAG_VERSION}" ]]; then
  VERSION="${TAG_VERSION#v}"
else
  VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${REPO_ROOT}/Cargo.toml" | head -n1)"
fi

if [[ -z "${VERSION}" ]]; then
  echo "[package] error: unable to resolve version."
  exit 1
fi

for bin in tui_next openjaxd openjax-gateway openjax; do
  if [[ ! -f "${TARGET_DIR}/${bin}" ]]; then
    echo "[package] error: missing binary ${TARGET_DIR}/${bin}. run: make build-release-linux"
    exit 1
  fi
done
if [[ ! -f "${WEB_DIST_DIR}/index.html" ]]; then
  echo "[package] error: missing web bundle ${WEB_DIST_DIR}/index.html. run: make build-web-release"
  exit 1
fi

PACKAGE_NAME="openjax-v${VERSION}-linux-x86_64"
STAGE_DIR="${DIST_DIR}/${PACKAGE_NAME}"
ARCHIVE_PATH="${DIST_DIR}/${PACKAGE_NAME}.tar.gz"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin"
mkdir -p "${STAGE_DIR}/web"

cp "${TARGET_DIR}/tui_next" "${STAGE_DIR}/bin/tui_next"
cp "${TARGET_DIR}/openjaxd" "${STAGE_DIR}/bin/openjaxd"
cp "${TARGET_DIR}/openjax-gateway" "${STAGE_DIR}/bin/openjax-gateway"
cp "${TARGET_DIR}/openjax" "${STAGE_DIR}/bin/openjax"
cp -R "${WEB_DIST_DIR}/." "${STAGE_DIR}/web/"
cp "${SCRIPT_DIR}/install_linux.sh" "${STAGE_DIR}/install.sh"
cp "${SCRIPT_DIR}/uninstall_linux.sh" "${STAGE_DIR}/uninstall.sh"
cp "${SCRIPT_DIR}/README-install-linux.md" "${STAGE_DIR}/README-install.md"

chmod +x "${STAGE_DIR}/bin/tui_next" "${STAGE_DIR}/bin/openjaxd" "${STAGE_DIR}/bin/openjax-gateway" "${STAGE_DIR}/bin/openjax"
chmod +x "${STAGE_DIR}/install.sh" "${STAGE_DIR}/uninstall.sh"

mkdir -p "${DIST_DIR}"
rm -f "${ARCHIVE_PATH}"
tar -C "${DIST_DIR}" -czf "${ARCHIVE_PATH}" "${PACKAGE_NAME}"

(
  cd "${DIST_DIR}"
  shasum -a 256 "${PACKAGE_NAME}.tar.gz" > "SHA256SUMS"
)

echo "[package] archive: ${ARCHIVE_PATH}"
echo "[package] checksums: ${DIST_DIR}/SHA256SUMS"
