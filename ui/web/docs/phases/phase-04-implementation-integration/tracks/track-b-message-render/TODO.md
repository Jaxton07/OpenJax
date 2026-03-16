# Track B TODO

- [x] MessageList 增加 ToolStep payload 分支渲染
  - 状态: Done
  - 证据: `ui/web/src/components/MessageList.tsx` + `ui/web/src/components/tool-steps/ToolStepList.tsx`

- [x] 旧消息纯文本渲染路径保持不变
  - 状态: Done
  - 证据: `ui/web/src/components/MessageList.test.tsx` + `zsh -lc "cd ui/web && pnpm test"`

- [x] mixed message 场景验证（assistant + toolSteps）
  - 状态: Done
  - 证据: `ui/web/src/lib/session-events/tools.test.ts`（消息模型已拆分，验证为“每事件独立 tool_steps 消息”）
