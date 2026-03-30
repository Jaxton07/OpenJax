# Transcript-First Gateway P0/P1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate gateway event persistence from SQLite timeline to transcript-first JSONL with append-then-publish consistency, while splitting oversized gateway/core files and removing dead batch path.

**Architecture:** Keep `openjax-core` as the only event semantics source (`event_type` + `payload`). Make `openjax-gateway` a thin transport/runtime adapter with transcript storage (`manifest + segment JSONL`) and strict append-success-before-SSE semantics. Enforce unmapped core-event gate in tests and remove silent drop behavior.

**Tech Stack:** Rust 2024, tokio, serde/serde_json, axum SSE, existing OpenJax test harness (`make gateway-fast`, `make core-feature-*`).

---

### Task 1: Introduce Transcript Store Module Skeleton

**Files:**
- Create: `openjax-gateway/src/transcript/mod.rs`
- Create: `openjax-gateway/src/transcript/types.rs`
- Create: `openjax-gateway/src/transcript/store.rs`
- Modify: `openjax-gateway/src/lib.rs`
- Modify: `openjax-gateway/src/state/mod.rs`
- Test: `openjax-gateway/tests/gateway_api/m8_transcript_store.rs`

- [ ] **Step 1: Write failing transcript-store test**
```rust
#[test]
fn transcript_store_creates_manifest_and_first_segment() {
    // expect manifest + segment files after first append
}
```

- [ ] **Step 2: Run test to verify failure**
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m8_transcript_store -- --nocapture"`
Expected: FAIL with missing module/type errors.

- [ ] **Step 3: Add minimal transcript types/store skeleton**
```rust
pub struct TranscriptStore { /* root path, config */ }
pub struct TranscriptRecord { /* schema + envelope fields */ }
```

- [ ] **Step 4: Run test to verify compile + basic pass**
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m8_transcript_store"`
Expected: PASS for created-manifest/segment case.

- [ ] **Step 5: Commit**
```bash
git add openjax-gateway/src/transcript openjax-gateway/src/lib.rs openjax-gateway/src/state/mod.rs openjax-gateway/tests/gateway_api/m8_transcript_store.rs
git commit -m "feat(gateway): scaffold transcript store module"
```

### Task 2: Implement Append + Manifest + Rotation + 30-Day GC

**Files:**
- Modify: `openjax-gateway/src/transcript/store.rs`
- Modify: `openjax-gateway/src/transcript/types.rs`
- Test: `openjax-gateway/tests/gateway_api/m8_transcript_store.rs`

- [ ] **Step 1: Write failing tests for seq/rotation/gc**
```rust
#[test]
fn append_assigns_monotonic_event_seq_per_session() {}
#[test]
fn rotates_segment_when_size_limit_reached() {}
#[test]
fn gc_deletes_records_older_than_30_days() {}
#[test]
fn recovers_manifest_seq_when_tail_record_is_newer() {}
#[test]
fn switches_segment_and_warns_when_active_segment_is_not_writable() {}
```

- [ ] **Step 2: Run tests to verify failure**
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m8_transcript_store"`
Expected: FAIL with assertion mismatches.

- [ ] **Step 3: Implement minimal passing logic**
```rust
fn append(&self, session_id: &str, rec: NewRecord) -> Result<TranscriptRecord>;
fn replay(&self, session_id: &str, after: Option<u64>) -> Result<Vec<TranscriptRecord>>;
fn gc(&self, retention_days: u32) -> Result<()>;
fn recover_manifest_from_active_segment_tail(&self, session_id: &str) -> Result<()>;
fn rotate_when_active_segment_unwritable(&self, session_id: &str) -> Result<()>;
```

- [ ] **Step 4: Run tests to verify pass**
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m8_transcript_store"`
Expected: PASS including seq monotonicity, manifest-tail recovery warning assertion, and unwritable-segment rotation warning assertion.

- [ ] **Step 5: Commit**
```bash
git add openjax-gateway/src/transcript openjax-gateway/tests/gateway_api/m8_transcript_store.rs
git commit -m "feat(gateway): implement transcript append replay rotation gc"
```

### Task 3: Enforce Append-Then-Publish Pipeline

**Files:**
- Create: `openjax-gateway/src/state/publish_pipeline.rs`
- Modify: `openjax-gateway/src/state/events.rs`
- Modify: `openjax-gateway/src/handlers/stream.rs`
- Test: `openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs`

- [ ] **Step 1: Write failing consistency tests**
```rust
#[tokio::test]
async fn append_failure_does_not_emit_sse_event() {}
#[tokio::test]
async fn sse_and_timeline_are_identical_for_same_turn() {}
#[tokio::test]
async fn key_event_append_failure_marks_turn_failed_and_emits_single_transcript_append_error() {}
#[tokio::test]
async fn when_error_event_append_also_fails_turn_stops_without_recursive_error_emit() {}
#[tokio::test]
async fn last_event_id_resume_replays_exact_missing_events_from_transcript() {}
```

- [ ] **Step 2: Run failing tests**
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`
Expected: FAIL because old path still publishes after append failure.

- [ ] **Step 3: Implement single publish path**
```rust
pub fn append_then_publish(...) -> Result<(), ApiError>;
pub fn handle_key_event_append_failure(...) -> Result<(), ApiError>; // emit one TRANSCRIPT_APPEND_FAILED response_error, then stop on second failure
```

- [ ] **Step 4: Run targeted tests**
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline"`
Expected: PASS for append-failure block + key-failure convergence + Last-Event-ID replay alignment.

- [ ] **Step 5: Commit**
```bash
git add openjax-gateway/src/state/publish_pipeline.rs openjax-gateway/src/state/events.rs openjax-gateway/src/handlers/stream.rs openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs
git commit -m "fix(gateway): enforce append-then-publish consistency"
```

### Task 4: Replace Optional Mapper with Explicit Coverage Gate

**Files:**
- Modify: `openjax-gateway/src/event_mapper/mod.rs`
- Modify: `openjax-gateway/src/state/events.rs`
- Test: `openjax-gateway/src/event_mapper/mod.rs` (unit tests)
- Test: `openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs`

- [ ] **Step 1: Add failing test for unmapped core event**
```rust
#[test]
fn mapping_gate_fails_when_core_event_variant_not_covered() {}
```

- [ ] **Step 2: Run test to verify failure**
Run: `zsh -lc "cargo test -p openjax-gateway event_mapper -- --nocapture"`
Expected: FAIL due uncovered variants.

- [ ] **Step 3: Implement explicit gate**
```rust
pub enum MapResult { Mapped(CoreEventMapping), IgnoredInternal, Unmapped(&'static str) }
```

- [ ] **Step 4: Re-run mapper and gateway tests**
Run: `zsh -lc "cargo test -p openjax-gateway event_mapper"`
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline"`
Expected: PASS; unmapped case now fails tests explicitly and resume flow remains stable.

- [ ] **Step 5: Commit**
```bash
git add openjax-gateway/src/event_mapper/mod.rs openjax-gateway/src/state/events.rs openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs
git commit -m "test(gateway): add strict core-event mapping gate"
```

### Task 5: Split Oversized Gateway/Core Files and Remove Dead Batch Path

**Files:**
- Create: `openjax-gateway/src/state/turn_orchestrator.rs`
- Create: `openjax-gateway/src/state/core_projection.rs`
- Modify: `openjax-gateway/src/state/mod.rs`
- Modify: `openjax-gateway/src/state/events.rs`
- Create: `openjax-core/src/agent/tool_guard.rs` (or reuse existing with migrated logic)
- Create: `openjax-core/src/agent/tool_executor.rs`
- Create: `openjax-core/src/agent/tool_projection.rs`
- Modify: `openjax-core/src/agent/planner_tool_action.rs`
- Delete: `openjax-core/src/agent/planner_tool_batch.rs`
- Modify: `openjax-core/src/agent/mod.rs`
- Test: `openjax-core/tests/tools_sandbox_suite.rs`

- [ ] **Step 1: Add failing compile/test checks after planned module moves**
Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite --no-run"`
Expected: FAIL until moved modules and imports are fixed.

- [ ] **Step 2: Move code by responsibility with no behavior change**
```rust
// keep public behavior, split only ownership boundaries
```

- [ ] **Step 3: Remove dead batch module and references**
Run: `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite --no-run"`
Expected: PASS compile without `planner_tool_batch`.

- [ ] **Step 4: Run focused behavior tests**
Run: `zsh -lc "make core-feature-tools"`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add openjax-core/src/agent openjax-core/tests/tools_sandbox_suite.rs
git commit -m "refactor(core): split planner_tool_action and remove dead batch path"
```

### Task 6: README Sync + Final Regression Gate

**Files:**
- Modify: `openjax-gateway/README.md`
- Modify: `openjax-core/src/agent/README.md`
- Test: `openjax-gateway/tests/gateway_api_suite.rs`
- Test: `openjax-core/tests/streaming_suite.rs`

- [ ] **Step 1: Write doc assertions checklist as test notes**
```text
- gateway storage source == transcript JSONL
- no legacy persistence tree references
- planner_tool_batch removed from agent docs
```

- [ ] **Step 2: Update README files**
Run: `zsh -lc "git diff -- openjax-gateway/README.md openjax-core/src/agent/README.md"`
Expected: shows transcript-first structure and updated module list.

- [ ] **Step 3: Complete timeline/replay backend switch checks**
Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`
Expected: PASS with assertions that timeline/replay read transcript (no SQLite biz_events/biz_messages dependency in assertions).

- [ ] **Step 4: Run mandatory regression gates**
Run: `zsh -lc "make gateway-fast"`
Expected: PASS.
Run: `zsh -lc "make core-feature-streaming"`
Expected: PASS.
Run: `zsh -lc "make core-feature-tools"`
Expected: PASS.

- [ ] **Step 5: Final commit**
```bash
git add openjax-gateway/README.md openjax-core/src/agent/README.md
git commit -m "docs: align gateway/core readme with transcript-first architecture"
```

### Task 7: End-to-End Acceptance Proof

**Files:**
- Modify: `docs/superpowers/specs/2026-03-30-transcript-first-gateway-architecture-design.md` (append evidence section)
- Create: `docs/superpowers/plans/2026-03-30-transcript-first-gateway-p0p1-evidence.md`

- [ ] **Step 1: Capture verification outputs**
Run:
```bash
zsh -lc "make gateway-fast"
zsh -lc "make core-feature-streaming"
zsh -lc "make core-feature-tools"
```
Expected: all PASS with command timestamps.

- [ ] **Step 2: Record acceptance checklist**
```text
- append failure never publishes
- unmapped core event fails gate
- SSE/timeline parity preserved
- docs synced
```

- [ ] **Step 3: Commit evidence docs**
```bash
git add docs/superpowers/specs/2026-03-30-transcript-first-gateway-architecture-design.md docs/superpowers/plans/2026-03-30-transcript-first-gateway-p0p1-evidence.md
git commit -m "test: add transcript-first P0/P1 acceptance evidence"
```
