#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

cargo_bin="${CARGO:-cargo}"
time_bin="${TIME_BIN:-/usr/bin/time}"
package="openjax-core"

smoke_suites=(
  tools_sandbox_suite::system_tools_are_registered_in_specs
  streaming_suite::submit_with_sink_emits_events_in_same_order_as_submit_result
)

feature_skills_suites=(
  skills_suite
)

feature_tools_suites=(
  tools_sandbox_suite
)

feature_streaming_suites=(
  streaming_suite
)

feature_approval_suites=(
  approval_suite
  approval_events_suite
)

feature_history_suites=(
  core_history_suite
)

usage() {
  cat <<'EOF'
Usage: bash scripts/test/core.sh <core-smoke|core-feature-skills|core-feature-tools|core-feature-streaming|core-feature-approval|core-feature-history|core-full|core-baseline>
EOF
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

run_suite() {
  local suite="$1"
  shift

  local targets=("$@")
  echo "[${suite}] running ${#targets[@]} openjax-core entries"
  for target in "${targets[@]}"; do
    if [[ "$target" == *"::"* ]]; then
      run_test_case "$suite" "$target"
    else
      run_suite_target "$suite" "$target"
    fi
  done
  echo "[${suite}] completed"
}

discover_all_suites() {
  find openjax-core/tests -maxdepth 1 -name '*_suite.rs' -print \
    | sed 's|.*/||; s|\.rs$||' \
    | sort
}

discover_all_test_cases() {
  while IFS= read -r suite; do
    [[ -z "$suite" ]] && continue
    "$cargo_bin" test -p "$package" --test "$suite" --quiet -- --list \
      | awk -v suite="$suite" '/: test$/ { sub(": test$", "", $1); print suite "\t" $1 }'
  done < <(discover_all_suites)
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
  local cold_real warm_real
  local -a rank_rows=()

  echo "[core-baseline] cleaning openjax-core build artifacts"
  "$cargo_bin" clean -p "$package"

  echo "[core-baseline] cold full run"
  cold_real="$(measure_command "core-baseline cold" "$cargo_bin" test -p "$package" --locked)"
  printf '[core-baseline] cold real: %s s\n' "$cold_real"

  echo "[core-baseline] warm full run"
  warm_real="$(measure_command "core-baseline warm" "$cargo_bin" test -p "$package" --locked)"
  printf '[core-baseline] warm real: %s s\n' "$warm_real"

  echo "[core-baseline] slow test ranking (warm single test case)"
  while IFS=$'\t' read -r suite test_name; do
    [[ -z "$suite" || -z "$test_name" ]] && continue

    local real
    real="$(measure_command "core-baseline ${suite}::${test_name}" "$cargo_bin" test -p "$package" --test "$suite" "$test_name" --locked --quiet)"
    rank_rows+=("${real}"$'\t'"${suite}::${test_name}")
  done < <(discover_all_test_cases)

  echo "[core-baseline] slowest tests:"
  printf '%s\n' "${rank_rows[@]}" \
    | LC_ALL=C sort -nr -k1,1 \
    | awk -F '\t' 'NR <= 20 { printf "[core-baseline] %2d. %-64s %8.2fs\n", NR, $2, $1 + 0 }'
}

main() {
  case "${1:-}" in
    core-smoke)
      run_suite "core-smoke" "${smoke_suites[@]}"
      ;;
    core-feature-skills)
      run_suite "core-feature-skills" "${feature_skills_suites[@]}"
      ;;
    core-feature-tools)
      run_suite "core-feature-tools" "${feature_tools_suites[@]}"
      ;;
    core-feature-streaming)
      run_suite "core-feature-streaming" "${feature_streaming_suites[@]}"
      ;;
    core-feature-approval)
      run_suite "core-feature-approval" "${feature_approval_suites[@]}"
      ;;
    core-feature-history)
      run_suite "core-feature-history" "${feature_history_suites[@]}"
      ;;
    core-full)
      echo "[core-full] cargo test -p ${package} --tests --locked"
      "$cargo_bin" test -p "$package" --tests --locked
      ;;
    core-baseline)
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
      echo "[core-test] unknown command: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"
