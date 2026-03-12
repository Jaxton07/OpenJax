#!/usr/bin/env bash
set -euo pipefail

PREFIX="${HOME}/.local/openjax"
VERSION="latest"
ASSUME_YES=0
REPO="${OPENJAX_GITHUB_REPO:-Jaxton07/OpenJax}"

usage() {
  cat <<USAGE
Usage: install_from_github.sh [--version <version|latest>] [--prefix <path>] [-y] [--repo <owner/name>]

Options:
  --version <value>  Release version without leading v (default: latest)
  --prefix <path>    Install prefix (default: ~/.local/openjax)
  --repo <owner/name>
                     GitHub repository (default: Jaxton07/OpenJax)
  -y, --yes          Skip confirmation prompt
  -h, --help         Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --prefix)
      PREFIX="$2"
      shift 2
      ;;
    --repo)
      REPO="$2"
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
      echo "[install-online] unknown argument: $1"
      usage
      exit 1
      ;;
  esac
done

if [[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]]; then
  ARTIFACT_SUFFIX="macos-aarch64"
elif [[ "$(uname -s)" == "Linux" && "$(uname -m)" == "x86_64" ]]; then
  ARTIFACT_SUFFIX="linux-x86_64"
else
  echo "[install-online] unsupported platform: $(uname -s) $(uname -m)"
  echo "[install-online] supported: macOS arm64, Linux x86_64"
  exit 1
fi

if command -v curl >/dev/null 2>&1; then
  downloader="curl"
elif command -v wget >/dev/null 2>&1; then
  downloader="wget"
else
  echo "[install-online] missing downloader: install curl or wget"
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  sha_cmd="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
  sha_cmd="shasum -a 256"
else
  echo "[install-online] missing checksum tool: install sha256sum or shasum"
  exit 1
fi

if [[ "$VERSION" == "latest" ]]; then
  release_base="https://github.com/${REPO}/releases/latest/download"
else
  release_base="https://github.com/${REPO}/releases/download/v${VERSION}"
fi

artifact_name() {
  local version="$1"
  printf "openjax-v%s-%s.tar.gz" "$version" "$ARTIFACT_SUFFIX"
}

download_file() {
  local url="$1"
  local output="$2"
  if [[ "$downloader" == "curl" ]]; then
    curl --fail --location --silent --show-error --output "$output" "$url"
  else
    wget -q -O "$output" "$url"
  fi
}

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if [[ "$VERSION" == "latest" ]]; then
  if [[ "$downloader" == "curl" ]]; then
    final_url="$(curl --head --location --silent --show-error --output /dev/null --write-out '%{url_effective}' "https://github.com/${REPO}/releases/latest")"
  else
    final_url="$(wget -qO- --server-response "https://github.com/${REPO}/releases/latest" 2>&1 | sed -n 's/.*Location: //p' | tr -d '\r' | tail -n1)"
  fi
  tag="$(basename "$final_url")"
  if [[ -z "$tag" || "$tag" == "latest" ]]; then
    echo "[install-online] failed to resolve latest release tag"
    echo "[install-online] manual page: https://github.com/${REPO}/releases/latest"
    exit 1
  fi
  resolved_version="${tag#v}"
else
  resolved_version="$VERSION"
fi

artifact="$(artifact_name "$resolved_version")"
artifact_url="${release_base}/${artifact}"
checksums_url="${release_base}/SHA256SUMS"

echo "[install-online] downloading ${artifact} from GitHub..."
if ! download_file "$artifact_url" "$TMP_DIR/$artifact"; then
  echo "[install-online] download failed: $artifact_url"
  echo "[install-online] fallback: download manually and run ./install.sh from extracted package"
  exit 1
fi

if ! download_file "$checksums_url" "$TMP_DIR/SHA256SUMS"; then
  echo "[install-online] failed to download checksums: $checksums_url"
  exit 1
fi

expected_hash="$(awk -v file="$artifact" '$2==file { print $1 }' "$TMP_DIR/SHA256SUMS")"
if [[ -z "$expected_hash" ]]; then
  echo "[install-online] missing checksum for artifact: $artifact"
  exit 1
fi

actual_hash="$($sha_cmd "$TMP_DIR/$artifact" | awk '{print $1}')"
if [[ "$expected_hash" != "$actual_hash" ]]; then
  echo "[install-online] checksum mismatch for $artifact"
  echo "[install-online] expected: $expected_hash"
  echo "[install-online] actual:   $actual_hash"
  exit 1
fi

echo "[install-online] checksum verified: $artifact"

tar -xzf "$TMP_DIR/$artifact" -C "$TMP_DIR"
pkg_dir="$(find "$TMP_DIR" -maxdepth 1 -type d -name "openjax-v*" | head -n1)"
if [[ -z "$pkg_dir" ]]; then
  echo "[install-online] failed to extract package directory"
  exit 1
fi

install_args=("--prefix" "$PREFIX")
if [[ "$ASSUME_YES" -eq 1 ]]; then
  install_args+=("--yes")
fi

if [[ "$ASSUME_YES" -ne 1 ]]; then
  echo "[install-online] install ${artifact} to ${PREFIX}/bin ? [y/N]"
  read -r reply
  if [[ ! "$reply" =~ ^[Yy]$ ]]; then
    echo "[install-online] aborted"
    exit 0
  fi
fi

"$pkg_dir/install.sh" "${install_args[@]}"
