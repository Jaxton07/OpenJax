# 01 Decisions

## 安全与协议决策

1. 使用 `opaque token + server session`，不使用 JWT。
2. Access token TTL 固定为 15 分钟。
3. Refresh token TTL 固定为 30 天，且每次 refresh 必须轮换。
4. Refresh token 仅服务端持久化（存哈希），客户端不保存明文到 localStorage。
5. Web 使用 HttpOnly Cookie 存 refresh token，access token 仅内存态。
6. 首版只支持 `owner` scope，但响应中保留 `scope` 字段。

## 存储决策

7. 认证存储后端固定为 SQLite（`rusqlite`）。
8. token 存储为 `sha256(token + server_pepper)`，不落明文。

## 兼容决策

9. 保留 owner key 作为 `/auth/login` 的入场凭证。
