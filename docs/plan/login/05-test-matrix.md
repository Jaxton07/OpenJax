# 05 Test Matrix

## Gateway 集成测试

1. login 成功（owner key 正确，cookie 设置正确）。
2. login 失败（缺 header / 错 key / 限流触发）。
3. access token 正常访问业务接口。
4. access token 过期后访问返回 `UNAUTHENTICATED`。
5. refresh 成功并轮换；旧 refresh 复用返回 `CONFLICT`。
6. logout 后 refresh 不可再用。
7. revoke 单会话后该会话 access/refresh 均失效。
8. revoke_all 后所有会话失效。
9. sessions 列表返回设备信息与状态。

## Web 测试

1. 登录成功后 `localStorage` 不包含 owner/access token。
2. 业务请求 401 触发 refresh 并自动重放。
3. refresh 失败时清空登录态并跳转 `/login`。
4. logout 后清空本地状态并请求服务端清 cookie。

## 验收命令

- `zsh -lc "cargo test -p openjax-gateway"`
- `zsh -lc "cd ui/web && pnpm test"`
- `zsh -lc "cargo build -p openjax-gateway && cd ui/web && pnpm build"`
