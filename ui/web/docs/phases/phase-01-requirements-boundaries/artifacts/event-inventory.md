# Event & UI Inventory

## 现状（Web）
- `assistant_delta/assistant_message` 折叠为 assistant 文本消息。
- `tool_call_started/tool_call_completed` 作为独立 tool 文本消息。
- `approval_requested` 独立进入审批面板。

## demo 能力
- 单条 tool step 包含结构化字段：type/status/time/code/output。
- 具备折叠/展开交互。
- 支持不同状态视觉标签。

## 差异结论
- 现有消息类型不足以承载结构化 step。
- 现有渲染层缺少“消息内嵌步骤流”容器。
- 需要补充可访问性语义（aria-expanded/controls）。
