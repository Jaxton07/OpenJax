# Session Filesystem Index Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 gateway 会话元数据与会话列表链路一次性切换到 file-only（snapshot + log + session.json），并让 WebUI 使用分页会话列表与会话详情懒加载。

**Architecture:** 在 `openjax-gateway/src/transcript` 新增 `SessionIndexStore`，用单写锁维护内存索引并持久化到 `index.snapshot.json` 与 `index.log.ndjson`。`POST/DELETE/事件更新` 统一通过索引 store 写入，`GET /api/v1/sessions` 改为分页读取内存索引，`ui/web` 只读取分页 API 并按需加载 timeline。故障态采用 `index_repair_required`，并在启动时执行 `.staging` 清理与一致性审计。

**Tech Stack:** Rust 2024, tokio, serde/serde_json, axum, OpenJax gateway integration tests, React + TypeScript (ui/web)。

---

### Task 1: 建立 SessionIndex 类型与恢复骨架

**Files:**
- Create: `openjax-gateway/src/transcript/session_index_types.rs`
- Create: `openjax-gateway/src/transcript/session_index_store.rs`
- Modify: `openjax-gateway/src/transcript/mod.rs`
- Test: `openjax-gateway/tests/gateway_api/m9_session_index_store.rs`
- Modify: `openjax-gateway/tests/gateway_api/mod.rs`

- [ ] **Step 1: 写失败测试，定义 snapshot + log 恢复主路径**

```rust
#[test]
fn index_store_recovers_from_snapshot_and_log() {
    // seed snapshot + ndjson log
    // assert recovered entries are sorted and consistent
}
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m9_session_index_store -- --nocapture"`  
Expected: FAIL（缺少 `session_index_store` 模块/类型）。

- [ ] **Step 3: 增加最小可编译实现**

```rust
pub struct IndexSessionEntry {
    pub session_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_event_seq: u64,
    pub last_preview: String,
}

pub struct SessionIndexStore {
    // root path + in-memory entries + write mutex
}
```

- [ ] **Step 4: 运行测试验证恢复逻辑通过**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m9_session_index_store"`  
Expected: PASS（snapshot + log replay 恢复结果正确）。

- [ ] **Step 5: 提交**

```bash
git add openjax-gateway/src/transcript/session_index_types.rs openjax-gateway/src/transcript/session_index_store.rs openjax-gateway/src/transcript/mod.rs openjax-gateway/tests/gateway_api/m9_session_index_store.rs openjax-gateway/tests/gateway_api/mod.rs
git commit -m "feat(gateway): scaffold session index store with snapshot-log recovery"
```

### Task 2: 实现原子写、单写锁、staging 与补偿失败故障态

**Files:**
- Modify: `openjax-gateway/src/transcript/session_index_store.rs`
- Test: `openjax-gateway/tests/gateway_api/m9_session_index_store.rs`

- [ ] **Step 1: 写失败测试覆盖事务边界与故障态**

```rust
#[test]
fn create_session_uses_staging_then_publish() {}

#[test]
fn delete_session_rolls_back_index_when_remove_dir_fails() {}

#[test]
fn compensation_append_failure_enters_index_repair_required() {}
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m9_session_index_store -- --nocapture"`  
Expected: FAIL（事务顺序/故障态断言不满足）。

- [ ] **Step 3: 写最小实现使测试通过**

```rust
pub enum IndexHealth {
    Healthy,
    RepairRequired,
}

pub async fn create_session_index_entry(&self, entry: IndexSessionEntry) -> anyhow::Result<()>;
pub async fn delete_session_index_entry(&self, session_id: &str) -> anyhow::Result<()>;
// includes: staging publish, compensation log, and repair-required state
```

- [ ] **Step 4: 运行测试验证通过**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m9_session_index_store"`  
Expected: PASS（staging 生命周期、补偿失败进入 `index_repair_required`）。

- [ ] **Step 5: 提交**

```bash
git add openjax-gateway/src/transcript/session_index_store.rs openjax-gateway/tests/gateway_api/m9_session_index_store.rs
git commit -m "feat(gateway): enforce session index transaction boundaries and repair-required state"
```

### Task 3: 实现 compact、启动审计与重建路径

**Files:**
- Modify: `openjax-gateway/src/transcript/session_index_store.rs`
- Test: `openjax-gateway/tests/gateway_api/m9_session_index_store.rs`

- [ ] **Step 1: 写失败测试覆盖 compact 与重建**

```rust
#[test]
fn compact_rotates_log_with_tmp_and_bak_without_truncating_live_log() {}

#[test]
fn startup_audit_reconciles_index_and_session_dirs() {}

#[test]
fn rebuild_from_sessions_dir_recovers_when_snapshot_corrupted() {}

#[test]
fn rebuild_from_sessions_dir_runs_when_log_replay_corrupted() {}

#[test]
fn startup_fails_when_rebuild_fails() {}

#[test]
fn startup_cleanup_removes_stale_staging_dirs_and_keeps_recent_ones() {}

#[tokio::test]
async fn concurrent_upsert_and_touch_do_not_corrupt_index_log() {}

#[test]
fn compact_rollback_failure_sets_repair_required() {}

#[test]
fn startup_audit_persists_repair_ops_into_index_log() {}

#[test]
fn rebuild_ignores_sessions_under_staging_directory() {}

#[test]
fn restart_rebuild_clears_previous_repair_required_state() {}
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m9_session_index_store -- --nocapture"`  
Expected: FAIL（compact、log 损坏恢复、重建失败启动失败、staging 清理、并发完整性断言未满足）。

- [ ] **Step 3: 写最小实现**

```rust
const COMPACT_MAX_LINES: usize = 1000;
const COMPACT_MAX_BYTES: u64 = 4 * 1024 * 1024;

fn maybe_compact(&self) -> anyhow::Result<()>;
fn startup_audit(&self) -> anyhow::Result<()>;
fn rebuild_from_sessions_dir(&self) -> anyhow::Result<()>;
fn cleanup_staging_dirs(&self, now: OffsetDateTime) -> anyhow::Result<()>;
fn mark_repair_required(&self) -> anyhow::Result<()>;
fn load_with_recovery_or_fail(&self) -> anyhow::Result<()>; // rebuild fails => startup fails
fn append_audit_repair_op(&self, op: IndexLogOp) -> anyhow::Result<()>; // audit durability
```

- [ ] **Step 4: 运行测试验证通过**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m9_session_index_store"`  
Expected: PASS（compact 原子替换、staging 生命周期、snapshot/log 损坏重建、审计修复落盘、重建失败启动失败、重启后恢复可用、并发完整性均满足）。

- [ ] **Step 5: 提交**

```bash
git add openjax-gateway/src/transcript/session_index_store.rs openjax-gateway/tests/gateway_api/m9_session_index_store.rs
git commit -m "feat(gateway): add session index compact audit and rebuild flow"
```

### Task 4: Gateway 会话 API 切换到 file-only 索引分页

**Files:**
- Modify: `openjax-gateway/src/state/events.rs`
- Modify: `openjax-gateway/src/handlers/session.rs`
- Modify: `openjax-gateway/src/lib.rs`
- Test: `openjax-gateway/tests/gateway_api/m2_session_lifecycle.rs`
- Test: `openjax-gateway/tests/gateway_api/helpers.rs`

- [ ] **Step 1: 写失败测试覆盖分页契约与错误码**

```rust
#[tokio::test]
async fn list_sessions_returns_next_cursor_with_limit() {}

#[tokio::test]
async fn list_sessions_rejects_invalid_cursor() {}

#[tokio::test]
async fn list_sessions_returns_503_when_index_repair_required() {}

#[tokio::test]
async fn list_sessions_uses_default_limit_20_and_max_100() {}

#[tokio::test]
async fn list_sessions_cursor_uses_strict_less_than_boundary() {}

#[tokio::test]
async fn list_sessions_sorted_by_updated_at_desc_then_session_id_desc() {}

#[tokio::test]
async fn list_sessions_cursor_payload_uses_utc_z_millis_format() {}

#[tokio::test]
async fn create_session_returns_503_when_index_repair_required() {}

#[tokio::test]
async fn delete_session_returns_503_when_index_repair_required() {}

#[tokio::test]
async fn list_sessions_returns_service_unavailable_before_startup_audit_ready() {}

#[tokio::test]
async fn list_sessions_keeps_backward_compatible_response_shape_without_paging_args() {}

#[tokio::test]
async fn list_sessions_keeps_existing_fields_when_paging_is_used() {}
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m2_session_lifecycle -- --nocapture"`  
Expected: FAIL（旧接口缺少启动门禁、分页契约与 repair 状态写接口拦截）。

- [ ] **Step 3: 实现 API 读写切换**

```rust
#[derive(Deserialize)]
pub struct ListSessionsQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct SessionListResponse {
    request_id: String,
    sessions: Vec<SessionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    timestamp: String,
}

fn normalize_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(20).clamp(1, 100)
}

fn normalize_cursor_timestamp(ts: &str) -> Result<String, ApiError> {
    // parse RFC3339 and emit YYYY-MM-DDTHH:MM:SS.mmmZ
}
fn ensure_session_index_ready(&self) -> Result<(), ApiError>; // startup audit not ready => 503
```

- [ ] **Step 4: 运行会话生命周期测试**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m2_session_lifecycle"`  
Expected: PASS（默认/上限 limit、严格 cursor 边界、启动审计完成前门禁、`GET/POST/DELETE` 的 repair 状态 `503`、以及响应兼容性均满足）。

- [ ] **Step 5: 提交**

```bash
git add openjax-gateway/src/state/events.rs openjax-gateway/src/handlers/session.rs openjax-gateway/src/lib.rs openjax-gateway/tests/gateway_api/m2_session_lifecycle.rs openjax-gateway/tests/gateway_api/helpers.rs
git commit -m "feat(gateway): switch session list API to file-only paged index"
```

### Task 5: 完整索引更新矩阵（事件 + title + tags）

**Files:**
- Modify: `openjax-gateway/src/state/events.rs`
- Modify: `openjax-gateway/src/state/core_projection.rs`
- Modify: `openjax-gateway/src/transcript/session_index_store.rs`
- Test: `openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs`
- Test: `openjax-gateway/tests/gateway_api/m9_session_index_store.rs`

- [ ] **Step 1: 写失败测试覆盖 last_event_seq/last_preview/updated_at 更新**

```rust
#[tokio::test]
async fn appending_user_message_updates_session_index_preview_and_event_seq() {}

#[test]
fn updating_session_title_updates_index_title_and_updated_at() {}

#[test]
fn updating_session_tags_updates_session_json_and_index_updated_at_only() {}

#[test]
fn last_preview_prefers_latest_user_message_and_truncates_to_120_utf8_chars() {}

#[test]
fn last_preview_is_empty_when_no_user_message_exists() {}
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline -- --nocapture"`  
Expected: FAIL（title/tags/preview 的更新矩阵不完整）。

- [ ] **Step 3: 实现最小联动逻辑**

```rust
fn derive_last_preview(event_type: &str, payload: &serde_json::Value) -> Option<String>;
// on append success: touch updated_at + last_event_seq + maybe preview
pub async fn update_session_title(&self, session_id: &str, title: Option<String>) -> anyhow::Result<()>;
pub async fn update_session_tags(&self, session_id: &str, tags: Vec<String>) -> anyhow::Result<()>;
fn truncate_utf8_chars(input: &str, max_chars: usize) -> String;
```

- [ ] **Step 4: 运行测试验证通过**

Run: `zsh -lc "cargo test -p openjax-gateway --test gateway_api_suite m5_stream_and_timeline"`  
Expected: PASS（事件、title、tags 的索引更新矩阵全部满足 spec）。

- [ ] **Step 5: 提交**

```bash
git add openjax-gateway/src/state/events.rs openjax-gateway/src/state/core_projection.rs openjax-gateway/src/transcript/session_index_store.rs openjax-gateway/tests/gateway_api/m5_stream_and_timeline.rs openjax-gateway/tests/gateway_api/m9_session_index_store.rs
git commit -m "feat(gateway): complete session index update matrix for event title tags"
```

### Task 6: WebUI 分页会话列表与懒加载改造

**Files:**
- Modify: `ui/web/src/types/gateway.ts`
- Modify: `ui/web/src/lib/gatewayClient.ts`
- Modify: `ui/web/src/hooks/useChatApp.ts`
- Test: `ui/web/src/hooks/useChatApp.test.ts`
- Test: `ui/web/src/lib/gatewayClient.test.ts`

- [ ] **Step 1: 写失败测试覆盖分页请求与懒加载**

```ts
it("loads session sidebar with cursor pagination only", async () => {});
it("hydrates timeline only after selecting a session", async () => {});
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/useChatApp.test.ts src/lib/gatewayClient.test.ts"`  
Expected: FAIL（当前是全量 hydrate timeline）。

- [ ] **Step 3: 写最小前端实现**

```ts
type GatewaySessionListResponse = {
  request_id: string;
  sessions: GatewaySessionSummary[];
  next_cursor?: string;
  timestamp: string;
};

listChatSessions(params?: { cursor?: string; limit?: number })
```

- [ ] **Step 4: 运行前端测试验证通过**

Run: `zsh -lc "cd ui/web && pnpm test -- src/hooks/useChatApp.test.ts src/lib/gatewayClient.test.ts"`  
Expected: PASS（列表分页 + 点击后懒加载）。

- [ ] **Step 5: 提交**

```bash
git add ui/web/src/types/gateway.ts ui/web/src/lib/gatewayClient.ts ui/web/src/hooks/useChatApp.ts ui/web/src/hooks/useChatApp.test.ts ui/web/src/lib/gatewayClient.test.ts
git commit -m "feat(web): paginate session sidebar and lazy-load session timeline"
```

### Task 7: 回归验证与文档同步

**Files:**
- Modify: `openjax-gateway/README.md`
- Modify: `ui/web/README.md`
- Modify: `docs/superpowers/specs/2026-03-30-session-filesystem-index-design.md`（如审计后需术语校正）

- [ ] **Step 1: 文档断言清单对齐实现**

```text
- 会话列表来源为 file-only index（非 biz_sessions）
- GET /api/v1/sessions 支持 cursor/limit/next_cursor
- Web 会话列表为分页 API + 懒加载详情
```

- [ ] **Step 2: 跑 gateway 快速回归**

Run: `zsh -lc "make gateway-fast"`  
Expected: PASS。

- [ ] **Step 3: 跑核心流式与工具回归（防回归）**

Run: `zsh -lc "make core-feature-streaming"`  
Run: `zsh -lc "make core-feature-tools"`  
Expected: PASS。

- [ ] **Step 4: 跑前端构建与测试**

Run: `zsh -lc "cd ui/web && pnpm test && pnpm build"`  
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add openjax-gateway/README.md ui/web/README.md docs/superpowers/specs/2026-03-30-session-filesystem-index-design.md
git commit -m "docs(gateway,web): align session file-index pagination and lazy-load flow"
```

## 执行顺序与边界提醒

1. 严格按 Task 1 -> Task 7 顺序执行，不跳步，不并行改同一文件。
2. 所有行为改动都先写失败测试再实现（TDD），禁止先写实现再补测试。
3. `openjax-gateway/src/state/events.rs` 如果在执行中超过 800 行，必须在该任务内继续拆分，避免单文件继续膨胀。
4. 不保留 SQLite `biz_sessions` 作为会话列表读取后门，不做兼容双写。
5. 出现 `index_repair_required` 相关实现分歧时，以 spec 为唯一准绳，不加额外兜底逻辑。
