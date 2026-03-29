# Gateway Test Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a gateway-specific fast/slow/full test workflow, align docs and CI to that workflow, and migrate the oversized gateway API integration tests to the repository-standard `suite + child case files` structure.

**Architecture:** Introduce a single `scripts/test/gateway.sh` orchestration script as the source of truth for all gateway test entry points. Makefile, README, and CI become thin wrappers around that script. In parallel, replace `openjax-gateway/tests/gateway_api.rs` with `gateway_api_suite.rs` plus focused domain files under `openjax-gateway/tests/gateway_api/`, keeping behavior and assertions equivalent.

**Tech Stack:** Rust 2024, Cargo test targets, Bash scripting, GitHub Actions, repository Makefile conventions

---

## File Map

### New files

- `scripts/test/gateway.sh`
  Gateway test orchestration entry point for `gateway-smoke`, `gateway-fast`, `gateway-doc`, `gateway-full`, and `gateway-baseline`.
- `openjax-gateway/tests/gateway_api_suite.rs`
  Suite entry point that re-exports child gateway API case modules with `#[path = "..."] mod ...;`.
- `openjax-gateway/tests/gateway_api/mod.rs`
  Shared helper module declarations if the final test layout needs internal module grouping.
- `openjax-gateway/tests/gateway_api/helpers.rs`
  Shared test setup helpers reused by gateway API child files.
- `openjax-gateway/tests/gateway_api/m1_auth.rs`
  Authentication and login/logout/refresh/revoke API behavior tests.
- `openjax-gateway/tests/gateway_api/m2_session_lifecycle.rs`
  Session create, shutdown, persistence rehydrate, and messages lifecycle tests.
- `openjax-gateway/tests/gateway_api/m3_slash_and_compact.rs`
  Slash commands, clear, compact, and slash command catalog tests.
- `openjax-gateway/tests/gateway_api/m4_approval.rs`
  Approval resolution behavior tests.
- `openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs`
  SSE replay/resume, timeline, and persisted event behavior tests.
- `openjax-gateway/tests/gateway_api/m6_provider.rs`
  Provider CRUD endpoint tests.
- `openjax-gateway/tests/gateway_api/m7_policy_level.rs`
  Session policy level endpoint tests.

### Modified files

- `Makefile`
  Add gateway test targets and help text.
- `openjax-gateway/README.md`
  Replace the default gateway test command guidance with layered commands.
- `.github/workflows/ci.yml`
  Add gateway-specific test jobs or steps for fast/doc/full execution without redefining workspace governance.
- `openjax-gateway/tests/policy_api_suite.rs`
  Optional cleanup to align helper naming or future-proof for a later `policy_api/` split without changing behavior.

### Deleted files

- `openjax-gateway/tests/gateway_api.rs`
  Remove only after all cases are migrated into `gateway_api_suite.rs` and child files.

## Task 1: Build Gateway Test Script

**Files:**
- Create: `scripts/test/gateway.sh`
- Reference: `scripts/test/core.sh`
- Reference: `openjax-gateway/Cargo.toml`

- [ ] **Step 1: Write the script skeleton with explicit subcommands**

Create `scripts/test/gateway.sh` with:

```bash
#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

cargo_bin="${CARGO:-cargo}"
time_bin="${TIME_BIN:-/usr/bin/time}"
package="openjax-gateway"
```

Include a `usage()` function that supports:

```text
gateway-smoke
gateway-fast
gateway-doc
gateway-full
gateway-baseline
```

- [ ] **Step 2: Add suite discovery helpers**

Implement helpers modeled after `scripts/test/core.sh`:

```bash
discover_gateway_suites() {
  find openjax-gateway/tests -maxdepth 1 -name '*_suite.rs' -print \
    | sed 's|.*/||; s|\.rs$||' \
    | sort
}
```

Also add a fixed list for standalone non-suite targets that must still be included in `gateway-fast`, initially:

```bash
standalone_targets=(
  m1_assistant_message_compat_only
)
```

- [ ] **Step 3: Implement `gateway-fast`**

Run:

```bash
"$cargo_bin" test -p "$package" --lib --locked --quiet
```

Then execute all discovered suite targets and standalone targets:

```bash
"$cargo_bin" test -p "$package" --test "$target" --locked --quiet
```

Expected outcome:
- unit tests pass
- gateway suite targets pass
- no doc tests run

- [ ] **Step 4: Implement `gateway-doc`**

Run:

```bash
"$cargo_bin" test -p "$package" --doc --locked --quiet
```

Expected outcome:
- doc phase is isolated
- command remains valid even when current doctest count is zero

- [ ] **Step 5: Implement `gateway-full`**

Compose the command instead of duplicating logic:

```bash
run_fast
run_doc
```

Expected outcome:
- `gateway-full` is the authoritative complete gateway validation path

- [ ] **Step 6: Implement `gateway-smoke`**

Choose a minimal fixed list of high-value cases after the test migration lands. Initial shape:

```bash
smoke_cases=(
  "gateway_api_suite::create_session_requires_auth"
  "gateway_api_suite::slash_commands_endpoint_returns_aliases_and_replaces_input"
  "policy_api_suite::publish_returns_incremented_policy_version"
)
```

Execute each with:

```bash
"$cargo_bin" test -p "$package" --test "$suite" "$filter" --locked --quiet
```

Expected outcome:
- smoke stays intentionally tiny
- failures indicate obvious gateway regressions quickly

- [ ] **Step 7: Implement `gateway-baseline`**

Mirror the `core-baseline` style:

```bash
"$cargo_bin" clean -p "$package"
```

Then measure:

```bash
"$time_bin" -p "$cargo_bin" test -p "$package" --lib --tests --locked --quiet
"$time_bin" -p "$cargo_bin" test -p "$package" --doc --locked --quiet
```

Also add optional warm per-target timing for:

```bash
--lib
--test gateway_api_suite
--test policy_api_suite
--test m1_assistant_message_compat_only
```

Expected outcome:
- separate cold/warm output
- fast vs doc cost is visible
- output is useful for future regression tracking

- [ ] **Step 8: Verify the script manually**

Run:

```bash
bash scripts/test/gateway.sh gateway-fast
bash scripts/test/gateway.sh gateway-doc
```

Expected:
- both commands exit 0
- logs clearly show which targets ran

- [ ] **Step 9: Commit the script-only change**

Run:

```bash
git add scripts/test/gateway.sh
git commit -m "test(gateway): add layered gateway test script"
```

## Task 2: Add Makefile Entry Points

**Files:**
- Modify: `Makefile`
- Reference: `scripts/test/gateway.sh`

- [ ] **Step 1: Extend `.PHONY`**

Add:

```make
gateway-smoke gateway-fast gateway-doc gateway-full gateway-baseline
```

- [ ] **Step 2: Add help text**

Under the testing section, add:

```make
@echo "  openjax-gateway 测试入口:"
@echo "    make gateway-smoke      - 运行 openjax-gateway 烟雾测试"
@echo "    make gateway-fast       - 运行 openjax-gateway 快线测试"
@echo "    make gateway-doc        - 运行 openjax-gateway 文档测试"
@echo "    make gateway-full       - 运行 openjax-gateway 完整测试"
@echo "    make gateway-baseline   - 运行 openjax-gateway 冷/热耗时统计"
```

- [ ] **Step 3: Add target wrappers**

Implement:

```make
gateway-smoke:
	bash scripts/test/gateway.sh gateway-smoke

gateway-fast:
	bash scripts/test/gateway.sh gateway-fast

gateway-doc:
	bash scripts/test/gateway.sh gateway-doc

gateway-full:
	bash scripts/test/gateway.sh gateway-full

gateway-baseline:
	bash scripts/test/gateway.sh gateway-baseline
```

- [ ] **Step 4: Verify the Makefile wiring**

Run:

```bash
make gateway-fast
make gateway-doc
```

Expected:
- both commands delegate to the script and exit 0

- [ ] **Step 5: Commit the Makefile change**

Run:

```bash
git add Makefile
git commit -m "build(test): add gateway make targets"
```

## Task 3: Update Gateway README

**Files:**
- Modify: `openjax-gateway/README.md`

- [ ] **Step 1: Replace the current default test command block**

Change the local development section so it no longer presents:

```bash
zsh -lc "cargo test -p openjax-gateway"
```

as the daily default.

- [ ] **Step 2: Add layered command guidance**

Add a compact command section like:

```bash
zsh -lc "make gateway-fast"
zsh -lc "make gateway-doc"
zsh -lc "make gateway-full"
```

And explain:
- daily development uses `gateway-fast`
- pre-merge uses `gateway-full`
- doc validation is isolated in `gateway-doc`

- [ ] **Step 3: Verify doc consistency**

Check that README terminology exactly matches:
- `scripts/test/gateway.sh`
- `Makefile`
- CI job names

Expected:
- no naming drift

- [ ] **Step 4: Commit the README change**

Run:

```bash
git add openjax-gateway/README.md
git commit -m "docs(gateway): document layered test workflow"
```

## Task 4: Add Gateway CI Fast/Doc Jobs

**Files:**
- Modify: `.github/workflows/ci.yml`
- Reference: `Makefile`
- Reference: `scripts/test/gateway.sh`

- [ ] **Step 1: Decide the job shape**

Keep the existing workspace Rust job intact.

Add gateway-specific jobs or steps with this intent:
- PR fast feedback path uses `make gateway-fast`
- doc path uses `make gateway-doc`

Do not redefine the entire workspace gating strategy.

- [ ] **Step 2: Implement the fast job**

Add a dedicated gateway test job with standard Rust setup:

```yaml
- name: Gateway fast tests
  run: make gateway-fast
```

Expected:
- gateway changes have a short, targeted validation path

- [ ] **Step 3: Implement the doc job**

Add a separate job or step:

```yaml
- name: Gateway doc tests
  run: make gateway-doc
```

Expected:
- doctest overhead is isolated from the fast path

- [ ] **Step 4: Optionally add `gateway-full` for heavier validation**

If the workflow structure benefits from it, add a non-default or later-stage job:

```yaml
- name: Gateway full tests
  run: make gateway-full
```

Only add this if it does not create redundant runtime without a clear purpose.

- [ ] **Step 5: Validate workflow syntax locally**

Run:

```bash
sed -n '1,260p' .github/workflows/ci.yml
```

Then verify that:
- job names are clear
- no YAML indentation issues exist
- commands match Makefile target names exactly

- [ ] **Step 6: Commit the CI change**

Run:

```bash
git add .github/workflows/ci.yml
git commit -m "ci(gateway): add fast and doc gateway test jobs"
```

## Task 5: Create Gateway API Suite Helpers

**Files:**
- Create: `openjax-gateway/tests/gateway_api_suite.rs`
- Create: `openjax-gateway/tests/gateway_api/helpers.rs`
- Create: `openjax-gateway/tests/gateway_api/mod.rs`
- Reference: `openjax-gateway/tests/gateway_api.rs`

- [ ] **Step 1: Create the suite entry point**

Start `openjax-gateway/tests/gateway_api_suite.rs` with:

```rust
#[path = "gateway_api/m1_auth.rs"]
mod m1_auth;
#[path = "gateway_api/m2_session_lifecycle.rs"]
mod m2_session_lifecycle;
#[path = "gateway_api/m3_slash_and_compact.rs"]
mod m3_slash_and_compact;
#[path = "gateway_api/m4_approval.rs"]
mod m4_approval;
#[path = "gateway_api/m5_stream_and_timeline.rs"]
mod m5_stream_and_timeline;
#[path = "gateway_api/m6_provider.rs"]
mod m6_provider;
#[path = "gateway_api/m7_policy_level.rs"]
mod m7_policy_level;
```

- [ ] **Step 2: Move common helpers into `helpers.rs`**

Create shared helpers extracted from the old file:

```rust
pub fn app_with_api_key(api_key: &str) -> (axum::Router, AppState) { ... }
pub fn auth_header(token: &str) -> String { ... }
pub async fn response_json(response: axum::response::Response) -> Value { ... }
pub async fn login(app: &axum::Router, owner_key: &str) -> (String, String, String) { ... }
pub async fn create_session_for_test(app: &axum::Router, access_token: &str) -> String { ... }
```

Only extract helpers that remove duplication cleanly.

- [ ] **Step 3: Wire helpers for child modules**

Expose helpers either via:

```rust
mod helpers;
```

inside the suite entry file, or via a `mod.rs` that keeps imports predictable.

Expected:
- child test files can reuse setup without large copy/paste blocks

- [ ] **Step 4: Run the empty suite target to confirm module wiring**

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite -- --list
```

Expected:
- the new suite target compiles
- child test names are visible

- [ ] **Step 5: Commit the skeleton before migration**

Run:

```bash
git add openjax-gateway/tests/gateway_api_suite.rs openjax-gateway/tests/gateway_api
git commit -m "test(gateway): scaffold gateway api suite structure"
```

## Task 6: Migrate Auth and Session Tests

**Files:**
- Create: `openjax-gateway/tests/gateway_api/m1_auth.rs`
- Create: `openjax-gateway/tests/gateway_api/m2_session_lifecycle.rs`
- Modify: `openjax-gateway/tests/gateway_api_suite.rs`
- Reference: `openjax-gateway/tests/gateway_api.rs`

- [ ] **Step 1: Move auth tests into `m1_auth.rs`**

Migrate:
- `create_session_requires_auth`
- `login_refresh_logout_flow`
- `logout_without_access_token_returns_401`
- `refresh_reuse_conflict_returns_conflict`
- `revoke_session_invalidates_access_token`

Each test should use shared helpers rather than duplicate setup.

- [ ] **Step 2: Run only the auth tests**

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite m1_auth --quiet
```

Expected:
- moved auth tests pass

- [ ] **Step 3: Move session lifecycle tests into `m2_session_lifecycle.rs`**

Migrate session-related tests such as:
- create session success helpers
- shutdown flow
- persistence rehydrate
- session messages endpoint behavior

Keep behavior identical to the original assertions.

- [ ] **Step 4: Run the session lifecycle slice**

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite m2_session_lifecycle --quiet
```

Expected:
- moved session tests pass

- [ ] **Step 5: Commit the migrated auth/session slice**

Run:

```bash
git add openjax-gateway/tests/gateway_api_suite.rs openjax-gateway/tests/gateway_api
git commit -m "test(gateway): split auth and session lifecycle cases"
```

## Task 7: Migrate Slash, Approval, Stream, Provider, and Policy Level Tests

**Files:**
- Create: `openjax-gateway/tests/gateway_api/m3_slash_and_compact.rs`
- Create: `openjax-gateway/tests/gateway_api/m4_approval.rs`
- Create: `openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs`
- Create: `openjax-gateway/tests/gateway_api/m6_provider.rs`
- Create: `openjax-gateway/tests/gateway_api/m7_policy_level.rs`
- Reference: `openjax-gateway/tests/gateway_api.rs`

- [ ] **Step 1: Move slash and compact tests**

Migrate:
- `clear_command_submit_and_polling_flow`
- `slash_commands_endpoint_returns_aliases_and_replaces_input`
- `compact_endpoint_succeeds`

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite m3_slash_and_compact --quiet
```

- [ ] **Step 2: Move approval tests**

Migrate:
- `approval_resolve_second_call_returns_conflict`

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite m4_approval --quiet
```

- [ ] **Step 3: Move stream and timeline tests**

Migrate:
- `sse_replay_out_of_window_returns_invalid_argument`
- `sse_resume_query_takes_precedence_over_last_event_id`
- timeline endpoint cases
- persisted events and messages behavior cases

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline --quiet
```

- [ ] **Step 4: Move provider tests**

Migrate provider CRUD tests into `m6_provider.rs`.

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite m6_provider --quiet
```

- [ ] **Step 5: Move policy level tests**

Migrate:
- `get_policy_level_returns_200_with_default_level`
- `put_policy_valid_level_returns_200`
- `put_policy_invalid_level_returns_400`
- `get_policy_level_reflects_put_change`

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite m7_policy_level --quiet
```

- [ ] **Step 6: Remove the legacy monolithic test file**

Delete:

```text
openjax-gateway/tests/gateway_api.rs
```

Only after every migrated case passes under the suite target.

- [ ] **Step 7: Verify the full gateway suite target**

Run:

```bash
cargo test -p openjax-gateway --test gateway_api_suite --quiet
```

Expected:
- migrated target passes cleanly

- [ ] **Step 8: Commit the remainder of the migration**

Run:

```bash
git add openjax-gateway/tests
git commit -m "test(gateway): migrate gateway api cases into suite files"
```

## Task 8: Optional Policy API Internal Cleanup

**Files:**
- Modify: `openjax-gateway/tests/policy_api_suite.rs`
- Optional create: `openjax-gateway/tests/policy_api/`

- [ ] **Step 1: Assess whether `policy_api_suite.rs` needs immediate splitting**

Check:
- file size
- helper duplication
- domain clarity

If the current file remains readable and within reasonable size, do not split in this pass.

- [ ] **Step 2: Only split if it clearly improves maintainability**

If splitting is justified, create:

```text
openjax-gateway/tests/policy_api/
```

with focused modules for publish, CRUD, validation, and overlay.

- [ ] **Step 3: Verify behavior stays identical**

Run:

```bash
cargo test -p openjax-gateway --test policy_api_suite --quiet
```

Expected:
- policy API target remains green

- [ ] **Step 4: Commit only if a real cleanup was performed**

Run:

```bash
git add openjax-gateway/tests/policy_api_suite.rs openjax-gateway/tests/policy_api
git commit -m "test(gateway): organize policy api suite helpers"
```

Skip this commit entirely if no meaningful cleanup was needed.

## Task 9: Verify End-to-End Gateway Test Workflow

**Files:**
- Reference: `scripts/test/gateway.sh`
- Reference: `Makefile`
- Reference: `openjax-gateway/tests/`
- Reference: `.github/workflows/ci.yml`

- [ ] **Step 1: Run the fast path**

Run:

```bash
bash scripts/test/gateway.sh gateway-fast
```

Expected:
- unit tests pass
- suite targets pass
- standalone compatibility target passes

- [ ] **Step 2: Run the doc path**

Run:

```bash
bash scripts/test/gateway.sh gateway-doc
```

Expected:
- doc path exits 0

- [ ] **Step 3: Run the full path**

Run:

```bash
bash scripts/test/gateway.sh gateway-full
```

Expected:
- fast and doc paths both succeed

- [ ] **Step 4: Run the smoke path**

Run:

```bash
bash scripts/test/gateway.sh gateway-smoke
```

Expected:
- selected high-value cases pass

- [ ] **Step 5: Run the baseline path**

Run:

```bash
bash scripts/test/gateway.sh gateway-baseline
```

Expected:
- output includes cold/warm data
- output separates fast and doc cost

- [ ] **Step 6: Verify Makefile wrappers**

Run:

```bash
make gateway-fast
make gateway-doc
make gateway-full
```

Expected:
- wrappers invoke the script correctly

- [ ] **Step 7: Commit the verified workflow state**

Run:

```bash
git add scripts/test/gateway.sh Makefile openjax-gateway/README.md .github/workflows/ci.yml openjax-gateway/tests
git commit -m "test(gateway): add layered workflow and suite-based api tests"
```

## Task 10: Final Validation and Handoff

**Files:**
- Reference: `docs/superpowers/specs/2026-03-28-gateway-test-optimization-design.md`
- Reference: `docs/superpowers/plans/2026-03-28-gateway-test-optimization.md`

- [ ] **Step 1: Capture final evidence**

Record the exact commands and outcomes for:

```bash
bash scripts/test/gateway.sh gateway-fast
bash scripts/test/gateway.sh gateway-doc
bash scripts/test/gateway.sh gateway-full
```

- [ ] **Step 2: Confirm no stale command guidance remains**

Search for outdated guidance:

```bash
rg -n 'cargo test -p openjax-gateway"' openjax-gateway/README.md Makefile .github/workflows/ci.yml
```

Expected:
- no stale default daily command guidance remains

- [ ] **Step 3: Summarize residual risks**

Document:
- whether `policy_api_suite.rs` still needs later decomposition
- whether smoke case selection should evolve with future gateway features
- whether doc phase cost changes once real doctests are introduced

- [ ] **Step 4: Prepare handoff**

Handoff summary must include:
- new gateway command matrix
- migrated gateway API suite structure
- CI changes
- verification commands and results

