# 05 Auth and Security Baseline

## 鉴权基线（v1）

- 鉴权方式：API Key。
- 请求头：`Authorization: Bearer <api_key>`。
- 未认证请求一律返回 `UNAUTHENTICATED`。

## 最小安全要求

- 全部 API 默认需要鉴权（健康检查端点除外）。
- API Key 不写入日志明文。
- 错误响应不得泄露内部路径、堆栈、密钥信息。

## 限流基线

- 以 API Key 为维度执行限流。
- 对 `submit_turn` 与 `events` 分开设置限流策略。
- 限流触发时返回 `RATE_LIMITED` 与重试建议。

## 审计基线

- 记录：`request_id`、`session_id`、接口、耗时、结果码。
- 对审批操作额外记录：`approval_id`、决策结果、操作者。

## 后续扩展

- v2 可扩展 JWT/多租户；v1 不引入双模式鉴权复杂度。
