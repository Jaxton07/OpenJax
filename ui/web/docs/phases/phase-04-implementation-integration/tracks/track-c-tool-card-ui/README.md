# Track C - Tool Card UI

## 目标
- 将 demo 的 step 卡片 UI 迁移为可复用 React 组件并满足 a11y。

## 上游文档（编码前必读）
- [page_demo/app.js](../../../../../page_demo/app.js)
- [page_demo/styles.css](../../../../../page_demo/styles.css)
- [Phase 03 - Component Boundary](../../../phase-03-ui-architecture-components/artifacts/component-boundary.md)
- [Phase 03 - Interaction & A11y](../../../phase-03-ui-architecture-components/artifacts/interaction-a11y.md)
- [Phase 03 - Style & Token Plan](../../../phase-03-ui-architecture-components/artifacts/style-token-plan.md)
- [Phase 03 - Decisions](../../../phase-03-ui-architecture-components/DECISIONS.md)

## 任务范围
- 落地 ToolStepList/ToolStepCard/StepStatusBadge/StepBody。
- 实现展开/收起及状态视觉。
- 对齐现有 app.css 风格体系。

## 交付物
- 组件实现说明。
- 样式命名与 token 使用说明。

## 已实施行为（2026-03-09）
- 工具步骤渲染已拆分为 `ToolStepList / ToolStepCard / StepStatusBadge / StepBody`，组件职责按 Phase 03 边界执行。
- 卡片支持默认折叠与轻量展开动画（body 过渡 + chevron 旋转）。
- a11y 已落地：头部使用 `button`，包含 `aria-expanded`、`aria-controls`，详情容器使用 `role=region` + `aria-labelledby`。
- 状态样式统一为 `step-card--{status}` 与 `step-status--{status}`；v1 不渲染动作按钮。
