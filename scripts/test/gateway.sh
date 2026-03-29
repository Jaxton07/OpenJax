#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

cargo_bin="${CARGO:-cargo}"
time_bin="${TIME_BIN:-/usr/bin/time}"
package="openjax-gateway"
smoke_manifest="openjax-gateway/tests/.smoke-targets"

standalone_targets=(
  m1_assistant_message_compat_only
)

usage() {
  cat <<'EOF'
Usage: bash scripts/test/gateway.sh <gateway-smoke|gateway-fast|gateway-doc|gateway-full|gateway-baseline>
EOF
}

die() {
  echo "[gateway-test] $*" >&2
  exit 1
}

discover_suite_targets() {
  find openjax-gateway/tests -maxdepth 1 -name '*_suite.rs' -print \
    | sed 's|.*/||; s|\.rs$||' \
    | sort
}

discover_smoke_targets() {
  [[ -f "$smoke_manifest" ]] || die "missing smoke manifest: ${smoke_manifest}"

  awk '
    {
      sub(/\r$/, "", $0)
      if ($0 ~ /^[[:space:]]*#/ || $0 ~ /^[[:space:]]*$/) {
        next
      }
      sub(/^[[:space:]]+/, "", $0)
      sub(/[[:space:]]+$/, "", $0)
      print
    }
  ' "$smoke_manifest"
}

target_exists() {
  local target="$1"
  set +e
  "$cargo_bin" test -p "$package" --test "$target" --locked --quiet -- --list >/dev/null 2>&1
  local status=$?
  set -e
  [[ $status -eq 0 ]]
}

run_suite_target() {
  local suite="$1"
  local target="$2"

  echo "[${suite}] cargo test -p ${package} --test ${target} --locked --quiet"
  "$cargo_bin" test -p "$package" --test "$target" --locked --quiet
}

run_test_case() {
  local suite="$1"
  local case_spec="$2"
  local target="${case_spec%%::*}"
  local filter="${case_spec#*::}"

  echo "[${suite}] cargo test -p ${package} --test ${target} ${filter} --locked --quiet"
  "$cargo_bin" test -p "$package" --test "$target" "$filter" --locked --quiet
}

run_smoke() {
  local -a smoke_targets=()
  while IFS= read -r target; do
    [[ -z "$target" ]] && continue
    smoke_targets+=("$target")
  done < <(discover_smoke_targets)

  [[ ${#smoke_targets[@]} -gt 0 ]] || die "no smoke targets discovered from ${smoke_manifest}"

  echo "[gateway-smoke] discovered ${#smoke_targets[@]} smoke target(s) from ${smoke_manifest}"
  local entry
  for entry in "${smoke_targets[@]}"; do
    echo "[gateway-smoke] selected ${entry}"
    if [[ "$entry" == *"::"* ]]; then
      run_test_case "gateway-smoke" "$entry"
    else
      run_suite_target "gateway-smoke" "$entry"
    fi
  done
  echo "[gateway-smoke] completed"
}

run_fast() {
  echo "[gateway-fast] cargo test -p ${package} --lib --bins --locked --quiet"
  "$cargo_bin" test -p "$package" --lib --bins --locked --quiet

  local -a suite_targets=()
  while IFS= read -r suite; do
    [[ -z "$suite" ]] && continue
    suite_targets+=("$suite")
  done < <(discover_suite_targets)

  echo "[gateway-fast] running ${#suite_targets[@]} discovered suite target(s)"
  local target
  for target in "${suite_targets[@]}"; do
    run_suite_target "gateway-fast" "$target"
  done

  echo "[gateway-fast] running ${#standalone_targets[@]} standalone target(s)"
  for target in "${standalone_targets[@]}"; do
    run_suite_target "gateway-fast" "$target"
  done
  echo "[gateway-fast] completed"
}

run_doc() {
  echo "[gateway-doc] cargo test -p ${package} --doc --locked --quiet"
  "$cargo_bin" test -p "$package" --doc --locked --quiet
  echo "[gateway-doc] completed"
}

run_full() {
  echo "[gateway-full] start"
  run_fast
  run_doc
  echo "[gateway-full] completed"
}

measure_command() {
  local label="$1"
  shift

  local tmp_file cmd_out
  tmp_file="$(mktemp)"
  cmd_out="$(mktemp)"

  set +e
  "$time_bin" -p "$@" >"$cmd_out" 2>"$tmp_file"
  local status=$?
  set -e

  if [[ $status -ne 0 ]]; then
    echo "[${label}] command failed; time output follows:" >&2
    cat "$cmd_out" >&2
    cat "$tmp_file" >&2
    rm -f "$cmd_out"
    rm -f "$tmp_file"
    return "$status"
  fi

  local real
  real="$(awk '/^real / { print $2; exit }' "$tmp_file")"
  rm -f "$cmd_out"
  rm -f "$tmp_file"

  if [[ -z "$real" ]]; then
    echo "[${label}] failed to capture elapsed time" >&2
    return 1
  fi

  printf '%s\n' "$real"
}

print_measurement() {
  local label="$1"
  local real="$2"
  printf '  %-36s %s s\n' "${label}:" "$real"
}

measure_and_print() {
  local display_label="$1"
  shift
  local real
  real="$(measure_command "gateway-baseline ${display_label}" "$@")"
  print_measurement "$display_label" "$real"
}

run_baseline() {
  echo "[gateway-baseline] cleaning openjax-gateway build artifacts"
  "$cargo_bin" clean -p "$package"

  echo "[gateway-baseline] measurements"
  measure_and_print "cold/full" "$cargo_bin" test -p "$package" --locked
  measure_and_print "warm/full" "$cargo_bin" test -p "$package" --locked
  measure_and_print "warm/fast" bash "$0" gateway-fast
  measure_and_print "warm/doc" bash "$0" gateway-doc

  echo "[gateway-baseline] per-target"
  measure_and_print "--lib --bins" "$cargo_bin" test -p "$package" --lib --bins --locked --quiet
  if target_exists "gateway_api_suite"; then
    measure_and_print "gateway_api_suite" "$cargo_bin" test -p "$package" --test gateway_api_suite --locked --quiet
  else
    echo "[gateway-baseline] --test gateway_api_suite unavailable; skipped"
  fi

  if target_exists "policy_api_suite"; then
    measure_and_print "policy_api_suite" "$cargo_bin" test -p "$package" --test policy_api_suite --locked --quiet
  else
    echo "[gateway-baseline] --test policy_api_suite unavailable; skipped"
  fi

  if target_exists "m1_assistant_message_compat_only"; then
    measure_and_print "m1_assistant_message_compat_only" "$cargo_bin" test -p "$package" --test m1_assistant_message_compat_only --locked --quiet
  else
    echo "[gateway-baseline] --test m1_assistant_message_compat_only unavailable; skipped"
  fi
}

main() {
  case "${1:-}" in
    gateway-smoke)
      run_smoke
      ;;
    gateway-fast)
      run_fast
      ;;
    gateway-doc)
      run_doc
      ;;
    gateway-full)
      run_full
      ;;
    gateway-baseline)
      run_baseline
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    "")
      usage >&2
      exit 1
      ;;
    *)
      echo "[gateway-test] unknown command: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"
