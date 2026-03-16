# 06 Ops Runbook

## 关键环境变量

- `OPENJAX_GATEWAY_ACCESS_TTL_MINUTES`：默认 15
- `OPENJAX_GATEWAY_REFRESH_TTL_DAYS`：默认 30
- `OPENJAX_GATEWAY_COOKIE_SECURE`：默认 true
- `OPENJAX_GATEWAY_AUTH_RATE_LIMIT_LOGIN_PER_MIN`：默认 30
- `OPENJAX_GATEWAY_AUTH_RATE_LIMIT_REFRESH_PER_MIN`：默认 120
- `OPENJAX_GATEWAY_AUTH_TOKEN_PEPPER`：token 哈希 pepper（生产必配）

## 密钥轮换 SOP

1. 在 `OPENJAX_GATEWAY_API_KEYS` 加入新 key（保留旧 key）。
2. 通知客户端重新登录完成 refresh 会话更新。
3. 观察 24 小时后移除旧 key。
4. 审计日志确认无旧 key 登录成功记录。

## 故障排查

- 频繁 401：检查 access TTL、客户端 refresh 是否成功、cookie 是否被拦截。
- refresh 失败：检查 `OPENJAX_GATEWAY_COOKIE_SECURE` 与浏览器协议（http/https）匹配。
- 会话撤销不生效：检查 DB 写权限和 `auth_sessions` 状态。
- 高并发登录抖动：检查限流阈值并调高 `*_PER_MIN`。
