# 02 Client State Machine

## 会话状态机

- `idle` -> `connecting` -> `active` -> `closing` -> `closed`

## 回合状态机

- `draft` -> `submitting` -> `streaming` -> `completed`
- 异常分支：`submitting/streaming -> failed`

## 事件驱动

- `turn_started` 进入 `streaming`
- `assistant_delta` 追加流式文本
- `assistant_message` 固化最终文本
- `turn_completed` 进入 `completed`
- `error` 进入 `failed`

## 约束

- 状态机只消费 phase-2 登记事件类型。
- 未知事件类型忽略并记录调试日志。
