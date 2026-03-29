#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

cargo_bin="${CARGO:-cargo}"
time_bin="${TIME_BIN:-/usr/bin/time}"
package="openjax-gateway"

smoke_targets=(
  gateway_api_suite::clear_command_submit_and_polling_flow
  policy_api_suite::policy_rule_create_update_publish_affects_submit_turn
  m1_assistant_message_compat_only::response_completed_overrides_legacy_assistant_message
)

standalone_targets=(
  m1_assistant_message_compat_only
)

usage() {
  cat <<'EOF'
Usage: bash scripts/test/gateway.sh <gateway-smoke|gateway-fast|gateway-doc|gateway-full|gateway-baseline>
EOF
}

discover_suite_targets() {
  find openjax-gateway/tests -maxdepth 1 -name '*_suite.rs' -print \
    | sed 's|.*/||; s|\.rs$||' \
    | sort
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
  echo "[gateway-smoke] running ${#smoke_targets[@]} high-value cases"
  local entry
  for entry in "${smoke_targets[@]}"; do
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

run_baseline() {
  local cold_real warm_real fast_real doc_real

  echo "[gateway-baseline] cleaning openjax-gateway build artifacts"
  "$cargo_bin" clean -p "$package"

  echo "[gateway-baseline] cold full run"
  cold_real="$(measure_command "gateway-baseline cold" "$cargo_bin" test -p "$package" --locked)"
  printf '[gateway-baseline] cold real: %s s\n' "$cold_real"

  echo "[gateway-baseline] warm full run"
  warm_real="$(measure_command "gateway-baseline warm" "$cargo_bin" test -p "$package" --locked)"
  printf '[gateway-baseline] warm real: %s s\n' "$warm_real"

  echo "[gateway-baseline] warm fast run"
  fast_real="$(measure_command "gateway-baseline fast" bash "$0" gateway-fast)"
  printf '[gateway-baseline] fast real: %s s\n' "$fast_real"

  echo "[gateway-baseline] warm doc run"
  doc_real="$(measure_command "gateway-baseline doc" bash "$0" gateway-doc)"
  printf '[gateway-baseline] doc real: %s s\n' "$doc_real"

  echo "[gateway-baseline] warm per-target timing"
  local real
  real="$(measure_command "gateway-baseline --lib --bins" "$cargo_bin" test -p "$package" --lib --bins --locked --quiet)"
  printf '[gateway-baseline] --lib --bins real: %s s\n' "$real"

  if target_exists "gateway_api_suite"; then
    real="$(measure_command "gateway-baseline --test gateway_api_suite" "$cargo_bin" test -p "$package" --test gateway_api_suite --locked --quiet)"
    printf '[gateway-baseline] --test gateway_api_suite real: %s s\n' "$real"
  else
    echo "[gateway-baseline] --test gateway_api_suite unavailable; skipped"
  fi

  if target_exists "policy_api_suite"; then
    real="$(measure_command "gateway-baseline --test policy_api_suite" "$cargo_bin" test -p "$package" --test policy_api_suite --locked --quiet)"
    printf '[gateway-baseline] --test policy_api_suite real: %s s\n' "$real"
  else
    echo "[gateway-baseline] --test policy_api_suite unavailable; skipped"
  fi

  if target_exists "m1_assistant_message_compat_only"; then
    real="$(measure_command "gateway-baseline --test m1_assistant_message_compat_only" "$cargo_bin" test -p "$package" --test m1_assistant_message_compat_only --locked --quiet)"
    printf '[gateway-baseline] --test m1_assistant_message_compat_only real: %s s\n' "$real"
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
