# openjax-store

`openjax-store` 是 OpenJax 的持久化存储模块，提供基于 SQLite 的会话、消息、事件与 LLM provider 配置的统一存储层。

## 职责

- 定义 `SessionRepository` 与 `ProviderRepository` 两个核心 trait，规范存储接口。
- 提供 `SqliteStore`：实现上述两个 trait 的 SQLite 单文件存储，是 OpenJax 所有持久化状态的唯一真值来源。
- 管理会话生命周期：创建、查询、列举会话。
- 管理消息序列：按序追加消息，支持 turn 关联。
- 管理事件序列：按 `event_seq` 追加事件，支持增量查询（`after_event_seq`），用于 timeline 恢复与 SSE 回放。
- 管理 LLM provider 配置：增删改查 provider，支持设置/查询当前激活 provider。

## 文件树

```text
openjax-store/
├── Cargo.toml
└── src
    ├── lib.rs          # 公开导出：SqliteStore、两个 trait、所有 Record 类型
    ├── repository.rs   # SessionRepository / ProviderRepository trait 定义
    ├── sqlite.rs       # SqliteStore 实现（含 schema 初始化、内联单元测试）
    └── types.rs        # 数据记录类型：SessionRecord / MessageRecord / EventRecord / ProviderRecord / ActiveProviderRecord
```

## 公开接口

### `SessionRepository` trait

| 方法 | 说明 |
|------|------|
| `create_session(session_id, title)` | 创建会话，返回 `SessionRecord` |
| `get_session(session_id)` | 按 ID 查询会话 |
| `list_sessions()` | 列举所有会话（按 `updated_at` 倒序） |
| `append_message(session_id, turn_id, role, content)` | 追加消息，自动分配递增 `sequence` |
| `list_messages(session_id)` | 列举会话消息（按 `sequence` 升序） |
| `append_event(session_id, event_seq, turn_seq, turn_id, event_type, payload_json, timestamp, stream_source)` | 追加事件 |
| `list_events(session_id, after_event_seq)` | 列举事件，支持增量（`after_event_seq` 为 `None` 时返回全量） |
| `last_event_seq(session_id)` | 查询会话最大 `event_seq` |
| `last_turn_seq_by_turn(session_id)` | 按 `turn_id` 分组查询各 turn 最大 `turn_seq` |

### `ProviderRepository` trait

| 方法 | 说明 |
|------|------|
| `create_provider(name, base_url, model_name, api_key)` | 创建 LLM provider |
| `update_provider(provider_id, name, base_url, model_name, api_key)` | 更新 provider，`api_key` 为 `None` 时保留原值 |
| `delete_provider(provider_id)` | 删除 provider |
| `get_provider(provider_id)` | 按 ID 查询 provider |
| `list_providers()` | 列举所有 provider（按 `created_at` 倒序） |
| `get_active_provider()` | 查询当前激活 provider（含 `model_name` 快照） |
| `set_active_provider(provider_id)` | 设置激活 provider，provider 不存在时返回 `None` |

## 数据库 Schema

### `biz_sessions`

会话主表。

| 列 | 说明 |
|----|------|
| `session_id` | 主键 |
| `title` | 可选标题 |
| `created_at` | 创建时间（RFC3339） |
| `updated_at` | 最后更新时间，消息/事件追加时同步刷新 |

索引：`updated_at DESC`

### `biz_messages`

消息表，关联 `biz_sessions`（CASCADE DELETE）。

| 列 | 说明 |
|----|------|
| `message_id` | 主键（`msg_<uuid>`） |
| `session_id` | 外键 |
| `turn_id` | 可选 turn 关联 |
| `role` | `user` / `assistant` 等 |
| `content` | 消息正文 |
| `sequence` | 会话内递增序号（从 1 开始） |
| `created_at` | 创建时间 |

唯一约束：`(session_id, sequence)`

### `biz_events`

事件表，是 timeline 恢复的主数据来源，关联 `biz_sessions`（CASCADE DELETE）。

| 列 | 说明 |
|----|------|
| `id` | 自增主键 |
| `session_id` | 外键 |
| `event_seq` | 会话级单调递增事件序号 |
| `turn_seq` | turn 内事件序号 |
| `turn_id` | 可选 turn 关联 |
| `event_type` | 事件类型字符串 |
| `payload_json` | 事件载荷 JSON |
| `timestamp` | 事件业务时间戳 |
| `stream_source` | 来源标识（如 `model_live` / `synthetic`） |
| `created_at` | 写入时间 |

唯一约束：`(session_id, event_seq)`
关键索引：`(session_id, turn_id, event_seq)`、`(session_id, created_at)`

### `llm_providers`

LLM provider 配置表。`provider_name` 有唯一约束，防止重复注册。

### `llm_runtime_settings`

运行时配置表，目前仅使用 `setting_key = 'active_provider'` 行记录激活 provider。当激活 provider 被删除时，`provider_id` / `model_name` 置 NULL（`ON DELETE SET NULL`）。更新 provider `model_name` 时同步刷新该行的 `model_name` 快照。

## 使用方式

`SqliteStore` 实现了 `SessionRepository` 与 `ProviderRepository`，可通过 trait 对象或泛型传递给上层模块（如 `openjax-gateway`）：

```rust
use openjax_store::{SqliteStore, SessionRepository, ProviderRepository};
use std::path::Path;

// 打开文件数据库（目录不存在时自动创建）
let store = SqliteStore::open(Path::new("/var/data/openjax/store.db"))?;

// 内存数据库（用于测试）
let store = SqliteStore::open_memory()?;

// 使用
let session = store.create_session("sess_abc", Some("my session"))?;
let msg = store.append_message("sess_abc", Some("turn_1"), "user", "hello")?;
let events = store.list_events("sess_abc", None)?;
```

## 本地开发

从仓库根目录执行：

```bash
zsh -lc "cargo build -p openjax-store"
zsh -lc "cargo test -p openjax-store"
zsh -lc "cargo test -p openjax-store -- --nocapture"
```
