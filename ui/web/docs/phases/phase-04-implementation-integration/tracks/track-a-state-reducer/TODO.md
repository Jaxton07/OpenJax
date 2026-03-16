# Track A TODO

- [x] 定义 ToolStep 类型与状态枚举
  - 状态: Done
  - 证据: `ui/web/src/types/chat.ts`

- [x] 定义 ChatMessage 结构化扩展字段
  - 状态: Done
  - 证据: `ui/web/src/types/chat.ts`

- [x] 实现 StreamEvent 到 step 的创建/更新规则
  - 状态: Done
  - 证据: `ui/web/src/lib/session-events/reducer.ts`

- [x] 增加幂等与异常 payload 处理
  - 状态: Done
  - 证据: `zsh -lc "cd ui/web && pnpm test -- src/lib/session-events/reducer.test.ts"`
