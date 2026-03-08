# 03 State and Session Model

## 会话状态

- `active`: 会话可接收新 turn。
- `closing`: 收到关闭请求，正在完成清理。
- `closed`: 会话终态，不再接受请求。

## 回合状态

- `queued`: 已接收，等待执行。
- `running`: 正在执行并产生事件。
- `completed`: 正常完成。
- `failed`: 异常终止。

## 状态约束

- `closed` 会话下 `submit_turn` 返回 `CONFLICT`。
- 同一 `turn_id` 只允许一个终态事件（`turn_completed` 或 `error`）。
- `approval_id` 只能 resolve 一次，重复 resolve 返回 `CONFLICT`。
