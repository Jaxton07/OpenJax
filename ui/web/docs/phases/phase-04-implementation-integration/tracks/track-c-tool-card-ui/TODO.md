# Track C TODO

- [x] 新建 ToolStep 相关组件并定义 props
  - 状态: Done
  - 证据: `ui/web/src/components/tool-steps/ToolStepList.tsx` + `ui/web/src/components/tool-steps/ToolStepCard.tsx` + `ui/web/src/components/tool-steps/StepStatusBadge.tsx` + `ui/web/src/components/tool-steps/StepBody.tsx`

- [x] 落地展开/收起交互与 aria 语义
  - 状态: Done
  - 证据: `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`

- [x] 落地状态样式类并避免卡片/徽标串色
  - 状态: Done
  - 证据: `ui/web/src/styles/app.css`

- [x] 移动端布局与长文本场景验证
  - 状态: Done
  - 证据: `zsh -lc "cd ui/web && pnpm test && pnpm build"`
