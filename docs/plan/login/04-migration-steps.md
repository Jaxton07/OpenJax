# 04 Migration Steps

## 执行顺序

1. 网关新增 auth 子系统（token/store/service/rate_limit/cookie/types）。
2. 新增 `/api/v1/auth/*` 接口与 owner/access 双中间件。
3. 业务接口切换为 access token 鉴权。
4. Web 登录流切换：owner key -> login -> access token in-memory。
5. 接入 refresh 单飞与请求重放。
6. 新增会话管理（列出/撤销）入口。
7. 跑完整测试并更新对外文档。

## 回滚点

- 回滚：仅通过回滚版本处理，不保留运行时 legacy 开关。
