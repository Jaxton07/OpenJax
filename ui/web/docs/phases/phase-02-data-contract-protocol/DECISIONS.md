# Phase 02 决策记录

## 决策日志
- 日期: 2026-03-09
- 状态: Proposed
- 主题: ToolStep 与 ChatMessage 关系
- 背景: 现有 ChatMessage 仅文本 `content`。
- 决策: 增加可选结构化字段承载 step 流，并保留 content 回退。
- 影响: reducer 与 MessageList 需要同步升级。
- 关联任务/PR: Phase 04 Track A/B
