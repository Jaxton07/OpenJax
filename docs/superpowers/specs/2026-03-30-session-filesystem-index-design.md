# OpenJax Session Filesystem Index 设计（File-Only Session Metadata + Paged List）

## 1. 背景与目标

在 2026-03-30 已完成的 transcript-first P0/P1 基础上，当前会话列表仍主要依赖 SQLite `biz_sessions`。本设计的目标是将会话元数据与会话列表读取链路彻底切换到文件系统，并为 WebUI 提供可分页、可扩展且稳定的会话列表加载路径。

本次为一次性切换，不保留长期双写。

## 2. 已确认约束

1. 不做兼容补丁，不做长期双写。
2. 保持现有 public API 语义稳定（路径与主要响应语义不变）。
3. 文件写入遵循原子化流程：`tmp -> fsync -> rename`。
4. 索引更新必须有单写锁，避免并发损坏。
5. 启动恢复使用 `snapshot + log replay`。
6. 提供索引重建机制（从 `sessions/*` 重建）。
7. 会话列表必须支持分页，避免一次性返回全量。
8. 会话根目录策略不变：dev 与正式版统一继续使用 `~/.openjax`（不引入 dev 独立路径）。
9. 会话删除策略采用直接物理删除目录（`remove_dir_all`）。
10. 索引 compact 阈值：`log 行数 >= 1000` 或 `log 文件 >= 4MB`。

## 3. 存储模型

### 3.1 目录结构

```text
~/.openjax/transcripts/
  sessions/
    index.snapshot.json
    index.log.ndjson
    <session_id>/
      manifest.json
      segments/
        segment-000001.jsonl
        ...
      session.json
```

- `manifest.json` 与 `segments/*.jsonl` 继续用于 transcript 事件。
- `session.json` 是会话级元数据文件（新增）。
- `index.snapshot.json` 与 `index.log.ndjson` 是全局会话索引文件（新增）。

### 3.2 `session.json`

最小字段：

- `schema_version`
- `session_id`
- `title`（可选）
- `created_at`
- `updated_at`
- `tags`（可选）

### 3.3 `index.snapshot.json`

最小字段：

- `schema_version`
- `updated_at`
- `sessions`（数组）

数组元素：

- `session_id`
- `title`
- `created_at`
- `updated_at`
- `last_event_seq`
- `last_preview`

### 3.4 `index.log.ndjson`

每行一条操作记录：

- `op`: `upsert_session | delete_session`
- `session_id`
- `ts`
- `payload`（`upsert_session` 时携带索引条目）

## 4. 写入一致性与并发控制

### 4.1 原子写规则

对 `session.json` 与 `index.snapshot.json`：

1. 写入 `<target>.tmp`。
2. `flush` + `fsync(file)`。
3. `rename(tmp, target)`。
4. `fsync(parent_dir)`。

对 `index.log.ndjson`：

1. 仅允许 append。
2. 每次 append 后 `flush` + `fsync(file)`。
3. append 失败则当前索引更新失败，不更新内存索引。

### 4.2 单写锁

引入 `SessionIndexStore` 的单写锁（`tokio::sync::Mutex`）：

- 所有 `upsert/delete/compact/rebuild` 必须在同一锁域内执行。
- 锁内顺序为：持久化操作成功后再更新内存索引；失败则返回错误，不做部分提交。

## 5. 启动恢复与重建

### 5.1 启动恢复主路径

1. 读取 `index.snapshot.json` 到内存索引（不存在则空）。
2. 顺序回放 `index.log.ndjson`，应用 `upsert/delete`。
3. 回放结束后的内存索引作为当前真值服务分页查询。

### 5.2 损坏处理与重建

触发条件：

- snapshot 解析失败；
- log 解析/回放失败；
- snapshot/log 的 schema 或结构非法导致无法继续。

处理方式：

1. 扫描 `sessions/<id>/session.json`（必要时结合 `manifest.json` 补 `last_event_seq`）。
2. 重建完整内存索引。
3. 原子重写 `index.snapshot.json`。
4. 重置 `index.log.ndjson` 为空日志。

若重建失败：启动失败，显式报错，不静默降级。

## 6. Compact 机制

触发条件（任一满足）：

- `index.log.ndjson` 行数 >= 1000；
- `index.log.ndjson` 文件大小 >= 4MB。

compact 流程（锁内）：

1. 基于当前内存索引原子重写 `index.snapshot.json`。
2. 截断并重建 `index.log.ndjson`（空）。
3. 若任一步失败，保持旧文件并返回错误。

## 7. Gateway API 改造

### 7.1 `GET /api/v1/sessions`

- 保持原有 `sessions` 字段。
- 新增 query：`cursor`、`limit`（可选）。
- 新增响应字段：`next_cursor`（可选）。

分页排序：

- `updated_at DESC, session_id DESC`（稳定排序）。

cursor 语义：

- 由上一页末条的 `(updated_at, session_id)` 编码得到；
- 下一页查询使用“严格小于游标键”避免重复或漏项。

### 7.2 `POST /api/v1/sessions`

创建会话时：

1. 创建 `sessions/<id>/session.json`。
2. 写入 `upsert_session` 到索引日志并更新内存索引。
3. 保持现有接口返回结构不变。

### 7.3 `DELETE /api/v1/sessions/:id`

删除会话时：

1. 直接 `remove_dir_all(sessions/<id>)`。
2. 写入 `delete_session` 索引日志并更新内存索引。
3. 保持现有接口返回语义不变。

## 8. WebUI 链路改造

1. 会话侧栏只依赖分页 `GET /api/v1/sessions`。
2. 初次加载第一页；滚动或“加载更多”使用 `next_cursor` 请求下一页。
3. 点击会话后再懒加载 timeline/details，不在会话列表阶段加载全量 timeline。
4. 不做目录扫描，不由前端推断文件系统状态。

## 9. 模块与代码落点

Gateway：

- 新增 `openjax-gateway/src/transcript/session_index_types.rs`
- 新增 `openjax-gateway/src/transcript/session_index_store.rs`
- 修改 `openjax-gateway/src/transcript/mod.rs`
- 修改 `openjax-gateway/src/state/events.rs`
- 修改 `openjax-gateway/src/handlers/session.rs`

Web：

- 修改 `ui/web/src/types/gateway.ts`
- 修改 `ui/web/src/lib/gatewayClient.ts`
- 修改 `ui/web/src/hooks/useChatApp.ts`

## 10. 测试与验收门禁

必须覆盖以下最小集合：

1. 会话创建后立即出现在文件索引分页列表。
2. 会话删除后目录与索引项都消失。
3. `snapshot + log replay` 恢复结果正确。
4. 索引损坏时可从 `sessions/*` 重建。
5. 并发创建/更新时索引文件不损坏（含日志完整性）。
6. `make gateway-fast` 通过，相关 session/timeline 测试通过。
7. WebUI 会话列表走 API 分页链路，不扫目录。

## 11. 非目标

1. 不改会话根目录策略（仍统一 `~/.openjax`）。
2. 不做 SQLite 与文件索引长期双写。
3. 不变更公开 API 路径与核心语义（仅新增可选分页参数/字段）。
4. 不在本阶段引入额外存储后端。

## 12. 风险与控制

1. 并发写风险：通过单写锁 + 原子写 + fsync 顺序控制。
2. 日志损坏风险：启动时回放校验；失败触发重建而非静默跳过。
3. 日志增长风险：通过 1000 行 / 4MB compact 阈值收敛。
4. 列表性能风险：分页与懒加载替代全量 hydrate。
