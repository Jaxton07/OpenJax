# 02 API Contract

## 认证接口（`/api/v1/auth`）

### `POST /api/v1/auth/login`

- 鉴权：`Authorization: Bearer <owner_key>`
- 请求：

```json
{
  "device_name": "web-chrome",
  "platform": "web",
  "user_agent": "Mozilla/5.0 ..."
}
```

- 响应：

```json
{
  "request_id": "req_xxx",
  "access_token": "atk_xxx",
  "access_expires_in": 900,
  "session_id": "authsess_xxx",
  "scope": "owner",
  "timestamp": "2026-03-12T09:00:00Z"
}
```

- Cookie：设置 `ojx_refresh_token`（HttpOnly、SameSite=Lax、Path=/api/v1/auth）。

### `POST /api/v1/auth/refresh`

- 鉴权：
  - 优先从 Cookie 读取 `ojx_refresh_token`
  - 无 Cookie 时允许 body:

```json
{
  "refresh_token": "rtk_xxx"
}
```

- 响应：

```json
{
  "request_id": "req_xxx",
  "access_token": "atk_new",
  "access_expires_in": 900,
  "session_id": "authsess_xxx",
  "scope": "owner",
  "timestamp": "2026-03-12T09:10:00Z"
}
```

- Cookie：轮换并重写 `ojx_refresh_token`。

### `POST /api/v1/auth/logout`

- 鉴权：refresh（Cookie 或 body）
- 请求：

```json
{
  "session_id": "authsess_xxx"
}
```

- 响应：

```json
{
  "request_id": "req_xxx",
  "status": "logged_out",
  "timestamp": "2026-03-12T09:11:00Z"
}
```

- Cookie：清除 `ojx_refresh_token`。

### `POST /api/v1/auth/revoke`

- 鉴权：Access token
- 请求：

```json
{
  "session_id": "authsess_xxx",
  "device_id": "dev_xxx",
  "revoke_all": false
}
```

- 规则：`revoke_all=true` 时忽略 `session_id/device_id` 并全量撤销。
- 响应：

```json
{
  "request_id": "req_xxx",
  "revoked": 1,
  "timestamp": "2026-03-12T09:12:00Z"
}
```

### `GET /api/v1/auth/sessions`

- 鉴权：Access token
- 响应：

```json
{
  "request_id": "req_xxx",
  "sessions": [
    {
      "session_id": "authsess_xxx",
      "device_id": "dev_xxx",
      "scope": "owner",
      "device_name": "web-chrome",
      "platform": "web",
      "user_agent": "Mozilla/5.0 ...",
      "status": "active",
      "created_at": "2026-03-12T09:00:00Z",
      "last_seen_at": "2026-03-12T09:10:00Z",
      "revoked_at": null
    }
  ],
  "timestamp": "2026-03-12T09:13:00Z"
}
```

## 业务接口鉴权

- `/api/v1/sessions*`、`/api/v1/auth/revoke`、`/api/v1/auth/sessions`：必须 Access token。
- `/api/v1/auth/login`：必须 owner key。

## 错误语义

- `UNAUTHENTICATED`：token 缺失/失效/过期/刷新失败。
- `FORBIDDEN`：权限不足（预留 scope 扩展）。
- `CONFLICT`：refresh 轮换冲突或复用检测。
- `RATE_LIMITED`：登录或刷新限流。
