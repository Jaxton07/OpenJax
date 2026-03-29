# Gateway Smoke/Baseline Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `scripts/test/gateway.sh` choose smoke coverage by explicit metadata instead of hard-coded test names, and make `gateway-baseline` produce stable, readable timing output for ongoing comparison.

**Architecture:** Keep all behavior inside `scripts/test/gateway.sh` and companion gateway test files. Smoke selection should be driven by a small, explicit convention that lives next to the test modules, so test renames do not silently break the smoke lane. Baseline output should stay shell-native and human-readable, but structured enough to compare cold/warm/full/fast/doc/per-target timing without reading raw `time` output.

**Tech Stack:** Bash, Cargo test targets, Rust integration test suites under `openjax-gateway/tests`, Makefile passthrough

---

## File Map

- Modify: `scripts/test/gateway.sh`
  - Replace hard-coded smoke target list with convention-driven discovery or a checked-in manifest.
  - Normalize baseline output into stable labeled sections.
- Modify: `openjax-gateway/tests/gateway_api/mod.rs`
  - Add smoke metadata hook if the chosen approach is module-driven.
- Modify: `openjax-gateway/tests/policy_api/mod.rs`
  - Add smoke metadata hook if the chosen approach is module-driven.
- Optional Create: `openjax-gateway/tests/.smoke-targets`
  - Only if manifest-based selection is chosen over module comments.
- Modify: `openjax-gateway/README.md`
  - Document the smoke selection convention and baseline usage/output expectations.
- Verify: `Makefile`
  - No behavior change expected, but re-run `gateway-smoke` and `gateway-baseline` through existing targets if touched indirectly.

## Design Decisions

- Smoke lane should remain a tiny curated slice of high-value checks.
- Selection must be explicit and reviewable; do not infer smoke tests from fragile name patterns like `m1_` or `*_smoke`.
- The shortest path is a checked-in smoke manifest or inline suite metadata parsed by the script. Prefer whichever keeps ownership closest to the tests and is easiest to review.
- Baseline output should show:
  - cold full
  - warm full
  - warm fast
  - warm doc
  - warm per-target timings
  - a final summary block with aligned labels
- Do not add JSON, jq, or external dependencies for baseline formatting.

### Task 1: Lock Smoke Selection Contract

**Files:**
- Modify: `scripts/test/gateway.sh`
- Modify: `openjax-gateway/tests/gateway_api/mod.rs`
- Modify: `openjax-gateway/tests/policy_api/mod.rs`
- Optional Create: `openjax-gateway/tests/.smoke-targets`

- [ ] **Step 1: Choose one explicit smoke contract**

Pick exactly one of these and document it in the code before implementing:

```text
A. checked-in manifest file with one <target>::<test_name> per line
B. inline metadata comments in suite module files, parsed by gateway.sh
```

Expected: one source of truth, no fallback to the current hard-coded bash array.

- [ ] **Step 2: Replace the hard-coded `smoke_targets` array**

Implement the chosen contract in `scripts/test/gateway.sh`.

Expected behavior:
- `gateway-smoke` fails loudly if no smoke targets are discovered
- blank lines/comments are ignored if manifest/comments are used
- every discovered entry is echoed before execution

- [ ] **Step 3: Add or move smoke declarations next to the tests**

Examples, depending on the chosen contract:

```text
gateway_api_suite::clear_command_submit_and_polling_flow
policy_api_suite::policy_rule_create_update_publish_affects_submit_turn
m1_assistant_message_compat_only::response_completed_overrides_legacy_assistant_message
```

Expected: renaming a smoke test requires updating only the local declaration source, not bash logic.

- [ ] **Step 4: Verify smoke discovery**

Run:

```bash
zsh -lc "cd /Users/ericw/work/code/ai/openJax && bash scripts/test/gateway.sh gateway-smoke"
```

Expected: PASS, and output shows discovered smoke entries before running them.

- [ ] **Step 5: Commit**

```bash
git add scripts/test/gateway.sh openjax-gateway/tests/gateway_api/mod.rs openjax-gateway/tests/policy_api/mod.rs openjax-gateway/tests/.smoke-targets
git commit -m "test(gateway): 收紧 smoke 用例选择规则"
```

### Task 2: Normalize Baseline Output

**Files:**
- Modify: `scripts/test/gateway.sh`

- [ ] **Step 1: Define the target output shape**

Add a comment block or helper describing the intended report layout:

```text
[gateway-baseline] measurements
  cold/full: X.XX s
  warm/full: X.XX s
  warm/fast: X.XX s
  warm/doc:  X.XX s
[gateway-baseline] per-target
  --lib --bins: X.XX s
  gateway_api_suite: X.XX s
  policy_api_suite: X.XX s
  m1_assistant_message_compat_only: X.XX s
```

Expected: one stable output format that humans can compare line-by-line across runs.

- [ ] **Step 2: Refactor timing capture helpers**

Move repeated `printf` formatting into one helper so each measurement path only supplies:
- display label
- command to measure

Expected: adding/removing one measured target later is a one-line change.

- [ ] **Step 3: Print a final summary block**

Keep the existing timing work, but add a final compact summary after all measurements complete.

Expected:
- no raw temporary-file noise
- failures still stop the script with context
- success output ends with a compact report block

- [ ] **Step 4: Verify baseline output**

Run:

```bash
zsh -lc "cd /Users/ericw/work/code/ai/openJax && bash scripts/test/gateway.sh gateway-baseline"
```

Expected: PASS with labeled cold/warm/per-target sections and no stale target references.

- [ ] **Step 5: Commit**

```bash
git add scripts/test/gateway.sh
git commit -m "test(gateway): 规范 baseline 统计输出"
```

### Task 3: Sync Documentation

**Files:**
- Modify: `openjax-gateway/README.md`

- [ ] **Step 1: Document smoke contract**

Explain where smoke selections live and how to update them when a smoke test changes.

Expected: developers no longer need to read bash internals to maintain `gateway-smoke`.

- [ ] **Step 2: Document baseline intent**

Add a short note covering:
- when to use `gateway-baseline`
- what the summary block means
- why `gateway-doc` is separated from the fast lane

- [ ] **Step 3: Verify docs against commands**

Run:

```bash
zsh -lc "cd /Users/ericw/work/code/ai/openJax && bash scripts/test/gateway.sh --help"
```

Expected: README wording matches the actual command names and intent.

- [ ] **Step 4: Commit**

```bash
git add openjax-gateway/README.md
git commit -m "docs(gateway): 补充 smoke 与 baseline 约定"
```

### Task 4: Final Verification

**Files:**
- Verify only

- [ ] **Step 1: Run targeted verification**

```bash
zsh -lc "cd /Users/ericw/work/code/ai/openJax && bash scripts/test/gateway.sh gateway-smoke"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && bash scripts/test/gateway.sh gateway-fast"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && bash scripts/test/gateway.sh gateway-baseline"
zsh -lc "cd /Users/ericw/work/code/ai/openJax && make gateway-smoke"
```

Expected:
- smoke passes with discovered entries
- fast still covers lib/bins/suites/standalone targets
- baseline report is readable and complete

- [ ] **Step 2: Review diff shape**

Run:

```bash
zsh -lc "cd /Users/ericw/work/code/ai/openJax && git diff --stat"
```

Expected: changes remain limited to gateway test infra/docs, with no production-code drift.

- [ ] **Step 3: Final commit or squash decision**

If implemented as multiple commits, decide whether to keep them split or squash before push.

```bash
git log --oneline -n 5
```

Expected: clean local history that reflects smoke selection hardening and baseline output cleanup.
