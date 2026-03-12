# 03 Data Model

## SQLite 文件

- 环境变量：`OPENJAX_GATEWAY_AUTH_DB_PATH`
- 默认：`./.openjax/auth.db`

## 表结构

### `auth_sessions`

- `session_id TEXT PRIMARY KEY`
- `device_id TEXT NOT NULL`
- `scope TEXT NOT NULL`（固定 `owner`）
- `device_name TEXT`
- `platform TEXT`
- `user_agent TEXT`
- `status TEXT NOT NULL`（`active` / `revoked` / `logged_out`）
- `created_at TEXT NOT NULL`
- `last_seen_at TEXT NOT NULL`
- `revoked_at TEXT`

索引：
- `idx_auth_sessions_status`
- `idx_auth_sessions_last_seen`

### `auth_refresh_tokens`

- `token_id TEXT PRIMARY KEY`
- `session_id TEXT NOT NULL`
- `token_hash TEXT NOT NULL UNIQUE`
- `rotated_from TEXT`
- `expires_at TEXT NOT NULL`
- `created_at TEXT NOT NULL`
- `revoked_at TEXT`

索引：
- `idx_auth_refresh_session`
- `idx_auth_refresh_expires`

### `auth_access_tokens`

- `token_hash TEXT PRIMARY KEY`
- `session_id TEXT NOT NULL`
- `expires_at TEXT NOT NULL`
- `created_at TEXT NOT NULL`
- `revoked_at TEXT`

索引：
- `idx_auth_access_session`
- `idx_auth_access_expires`

## 清理策略

- 每次 login/refresh 触发惰性清理：
  - 删除 `auth_access_tokens` 中过期且 `revoked_at` 非空或超保留期记录。
  - 删除 `auth_refresh_tokens` 中过期且已撤销记录。
  - 标记长时间无活跃会话（超过 refresh TTL）为 `revoked`。
