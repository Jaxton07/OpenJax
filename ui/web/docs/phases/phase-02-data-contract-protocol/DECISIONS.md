# Phase 02 决策记录

## 决策日志
- 日期: 2026-03-09
- 状态: Accepted
- 主题: ToolStep 与 ChatMessage 关系
- 背景: 现有 ChatMessage 仅文本 `content`。
- 决策: 增加可选结构化字段承载 step 流，并保留 content 回退。
- 影响: reducer 与 MessageList 需要同步升级。
- 关联任务/PR: Phase 04 Track A/B

- 日期: 2026-03-09
- 状态: Accepted
- 主题: ToolStep 主键与 approval 绑定策略
- 背景: 若无稳定主键，stream 更新会出现重复 step 与错配风险。
- 决策: step 主键优先 `tool_call_id`，缺失时使用 `turn_id + event_seq`；approval 以 `approval_id` 为主键，可选记录 `tool_call_id` 做展示关联。
- 影响: reducer 幂等逻辑可落地，approval 与 step 的关系可追踪但不强耦合。
- 关联任务/PR: Phase 02 mapping freeze

## 阶段评审（2026-03-09）
- 结论: Pass
- 阻塞项: 无
- 行动项: 进入 Phase 03，冻结组件边界、交互规范、a11y 要求。
- 责任人: Team Lead
