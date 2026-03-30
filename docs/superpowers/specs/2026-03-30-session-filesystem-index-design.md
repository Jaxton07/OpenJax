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

### 3.5 `IndexSessionEntry` 固定 schema

`IndexSessionEntry` 是索引唯一条目结构，约束如下：

1. `index.snapshot.json.sessions[]` 与 `upsert_session.payload` 完全同构。
2. 固定字段：`session_id/title/created_at/updated_at/last_event_seq/last_preview`。
3. `tags` 不进入索引层，仅保留在 `session.json`。

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

### 4.3 创建/删除事务边界（避免部分提交）

#### 创建会话（`POST /sessions`）

锁内顺序固定为：

1. 在 `sessions/.staging/<session_id>/session.json` 写入元数据（原子写）。
2. append `upsert_session` 到 `index.log.ndjson`，成功后更新内存索引。
3. `rename(.staging/<id>, <id>)` 原子发布会话目录。
4. 若第 3 步失败：立即追加补偿 `delete_session` 日志并回滚内存索引，再返回错误。

若第 4 步中的补偿日志写入失败：网关进入 `index_repair_required` 致命状态，拒绝后续会话索引写入，并在下次启动自动执行 `rebuild_from_sessions_dir` 后才恢复服务。

说明：对外可见的会话列表与索引始终一致；失败不会留下“索引可见但目录未发布”的长期状态。

#### 删除会话（`DELETE /sessions/:id`）

锁内顺序固定为：

1. 读取并缓存当前索引项（用于必要时补偿）。
2. append `delete_session` 到 `index.log.ndjson`，成功后移除内存索引。
3. 直接 `remove_dir_all(sessions/<id>)`。
4. 若第 3 步失败：立即追加补偿 `upsert_session` 日志并恢复内存索引，返回错误。

若第 4 步中的补偿日志写入失败：同样进入 `index_repair_required` 致命状态并要求重建后恢复。

说明：删除要么“索引+目录”同时生效，要么显式回滚，避免幽灵目录或幽灵索引。

### 4.4 `.staging` 生命周期规则

1. `.staging` 目录不参与索引重建，不对外暴露为有效会话。
2. 启动恢复前扫描 `sessions/.staging/*`，删除最后修改时间超过 10 分钟的目录。
3. 10 分钟内的目录保留，留给可能仍在进行中的写入；后续启动继续按同规则处理。

## 5. 启动恢复与重建

### 5.1 启动恢复主路径

1. 按 4.4 先清理过期 `.staging` 目录。
2. 读取 `index.snapshot.json` 到内存索引（不存在则空）。
3. 顺序回放 `index.log.ndjson`，应用 `upsert/delete`。
4. 回放结束后的内存索引作为当前真值候选。
5. 追加一致性审计（启动必跑）：
   - 索引中存在但 `sessions/<id>/session.json` 缺失：删除该索引项并写入修复日志；
   - 目录中存在 `session.json` 但索引缺失：补齐索引项并写入修复日志。

审计后才对外提供 `GET /api/v1/sessions`。

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

### 5.3 `index_repair_required` 故障态

进入条件：

- 创建/删除流程中的补偿日志写入失败；
- compact 回滚失败且无法确认索引一致性；
- 其他被判定为“索引不可安全继续写入”的致命错误。

对外行为：

1. 拒绝会话索引写接口（如 `POST /api/v1/sessions`、`DELETE /api/v1/sessions/:id`），返回 `503 INDEX_REPAIR_REQUIRED`。
2. 会话列表读接口 `GET /api/v1/sessions` 返回 `503 INDEX_REPAIR_REQUIRED`。
3. 恢复入口为“下次启动自动重建”，本阶段不新增独立修复 API。

### 5.4 索引更新矩阵（运行期）

以下变更必须写 `upsert_session`（除删除）并更新内存索引：

1. `create_session`：初始化 `created_at/updated_at`，`last_event_seq=0`，`last_preview=""`。
2. 新增 transcript 事件（append 成功后）：更新 `updated_at`、`last_event_seq`，并按规则刷新 `last_preview`。
3. 会话标题变更：更新 `title` 与 `updated_at`。
4. 会话 tags 变更：更新 `session.json.tags`，并同步刷新索引 `updated_at`（不把 tags 写入索引条目）。
5. `delete_session`：写 `delete_session` 并从内存索引移除。

`last_preview` 规则：

- 优先取最近一条用户消息纯文本摘要（截断长度固定，例如 120 UTF-8 字符）。
- 若无用户消息则回退为空字符串。

## 6. Compact 机制

触发条件（任一满足）：

- `index.log.ndjson` 行数 >= 1000；
- `index.log.ndjson` 文件大小 >= 4MB。

compact 流程（锁内）：

1. 基于当前内存索引原子重写 `index.snapshot.json`。
2. 写入空日志临时文件 `index.log.ndjson.tmp`，并 `fsync(file)`。
3. `rename(index.log.ndjson, index.log.ndjson.bak)`。
4. `rename(index.log.ndjson.tmp, index.log.ndjson)`，并 `fsync(parent_dir)`。
5. 删除 `index.log.ndjson.bak`。
6. 若第 3-5 步任一步失败，优先尝试用 `.bak` 回滚到旧日志；无法回滚则返回错误并触发下次启动重建。

说明：compact 期间不允许直接 truncate 正式日志文件，避免失败时丢失旧日志。

## 7. Gateway API 改造

### 7.1 `GET /api/v1/sessions`

- 保持原有 `sessions` 字段。
- 新增 query：`cursor`、`limit`（可选）。
- 新增响应字段：`next_cursor`（可选）。
- `limit` 约束：默认 `20`，最大 `100`，超限按 `100` 处理。
- 非法 `cursor`（非 base64url 或解码结构不合法）返回 `400 INVALID_ARGUMENT`。

分页排序：

- `updated_at DESC, session_id DESC`（稳定排序）。

cursor 语义：

- 由上一页末条的 `(updated_at, session_id)` 编码得到；
- 编码格式：`base64url(JSON)`，JSON 结构为 `{ "updated_at": "...", "session_id": "..." }`；
- `updated_at` 必须规范化为 UTC `Z` 且固定毫秒精度（`YYYY-MM-DDTHH:MM:SS.mmmZ`），保证字符串字典序可比较；
- 下一页查询使用“严格小于游标键”避免重复或漏项。

### 7.2 `POST /api/v1/sessions`

创建会话时：

1. 按 4.3 的“创建会话事务边界”执行 staging + 索引提交 + 目录发布。
2. 成功后会话立即可被分页列表读取。
3. 保持现有接口返回结构不变。

### 7.3 `DELETE /api/v1/sessions/:id`

删除会话时：

1. 按 4.3 的“删除会话事务边界”执行索引提交 + 目录删除 + 失败补偿。
2. 成功后会话目录与索引项均不可见。
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
