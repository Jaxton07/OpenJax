# 登录体系重构（Owner Key -> Access/Refresh）

本目录用于冻结并推进 OpenJax Gateway 登录体系重构，目标是让 Web 与未来移动端复用同一套认证协议。

## 目标

- 从静态 API Key 直连迁移为 Access/Refresh token 体系。
- Access token 短期有效，Refresh token 持久化并可撤销。
- Web 不再持久化 owner key，不再持久化 access token。
- 提供会话撤销（按设备/会话/全量）能力。

## 固定决策

- token 方案：opaque token + server session
- refresh 存储：服务端 SQLite 持久化
- Web 策略：HttpOnly Cookie（refresh）+ 内存 access token
- access TTL：15 分钟
- refresh TTL：30 天（轮换）
- scope：首版仅 owner，预留扩展字段

## 文档索引

- [01-decisions.md](./01-decisions.md)
- [02-api-contract.md](./02-api-contract.md)
- [03-data-model.md](./03-data-model.md)
- [04-migration-steps.md](./04-migration-steps.md)
- [05-test-matrix.md](./05-test-matrix.md)
- [06-ops-runbook.md](./06-ops-runbook.md)

## 当前状态

- [x] 决策冻结
- [x] 协议冻结
- [x] 数据模型冻结
- [x] 迁移步骤冻结
- [x] 测试矩阵冻结
- [x] 运维手册冻结
