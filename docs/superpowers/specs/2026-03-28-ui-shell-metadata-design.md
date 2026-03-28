# UI Shell Metadata Adaptation Design

> 日期：2026-03-28
> 状态：待实施
> 基线提交：`54c30c65` `📝 docs(core): 对齐 native tool calling 剩余阶段收口文档`
> 前置事实：`openjax-protocol` / `openjax-core` 已完成 `ToolCallCompleted.shell_metadata` 契约，`openjax-gateway` 已完成透传

---

## 1. 文档定位

本文只整理 `ui/tui` 与 `ui/web` 对 `ToolCallCompleted.shell_metadata` 的消费适配，不重设计 core/gateway 事件模型。

当前上游基线已经成立：

- `openjax-protocol::Event::ToolCallCompleted` 包含 `shell_metadata: Option<ShellExecutionMetadata>`
- `openjax-core` 事件生成已把 shell 结构化数据放进该字段
- `openjax-gateway` 的 SSE / timeline / stdio 已透传该字段

因此本轮 UI 任务不是“定义新协议”，而是“让现有 UI 消费侧真正用起来”。

---

## 2. 当前代码现实

### 2.1 TUI 现状

`ui/tui` 直接消费 `openjax_protocol::Event`，不经过 gateway payload 映射。

当前问题：

- `ui/tui/src/app/reducer.rs` 在 `ToolCallCompleted` 分支只读取 `tool_name`、`ok`、`output`，没有消费 `shell_metadata` 或 `display_name`
- `ui/tui/src/app/tool_output.rs` 仍通过解析 `output` 字符串中的：
  - `backend=...`
  - `policy_decision=...`
  - `runtime_deny_reason=...`
  - `result_class=...`
  来推断展示语义
- 部分测试仍使用旧的 `ToolCallCompleted` 构造，未填写 `shell_metadata`

这意味着 TUI 当前虽然还能依赖旧字符串格式工作，但没有切换到结构化字段主路径。

### 2.2 WebUI 现状

`ui/web` 通过 gateway timeline/SSE 消费 JSON payload。

当前问题：

- `ui/web/src/types/gateway.ts` 仍把 `StreamEvent.payload` 视为宽泛 `Record<string, unknown>`，没有 `shell_metadata` 的显式类型
- `ui/web/src/lib/session-events/tools.ts` 在 `tool_call_completed` 分支只读取：
  - `display_name`
  - `tool_name`
  - `output`
  并直接将状态写死为 `success`
- `ToolStepCard` 组件没有渲染 `shell_metadata` 提供的 backend / degrade / deny hint / partial success 等结构化语义
- 现有测试只覆盖 merge/output，不覆盖 `shell_metadata`

这意味着 WebUI 虽然已经“能收到字段”，但实际上并没有消费这些字段。

---

## 3. 目标

### 3.1 总目标

让 TUI 与 WebUI 都能基于 `shell_metadata` 消费 `ToolCallCompleted` 的结构化 shell 结果，而不是继续把 `output` 字符串解析作为唯一主路径。

### 3.2 TUI 目标

1. `ToolCallCompleted` 优先读取 `shell_metadata`
2. TUI 的 backend summary / degraded warning / partial 状态判断改为优先基于结构化字段
3. 旧 `output` 字符串解析仅作为历史 fallback 保留
4. 所有 TUI 相关测试构造更新到新协议字段

### 3.3 WebUI 目标

1. 为 gateway tool event payload 建立显式 `shell_metadata` 类型
2. reducer 将 `shell_metadata` 折叠为 `ToolStep` 可展示信息
3. Tool 卡片显示 backend / degraded / deny hint / partial success 等结构化语义
4. 为 reducer 与组件补齐测试

---

## 4. 方案选择

### 方案 A：只改测试，UI 逻辑继续解析 `output`

优点：

- 改动最小

缺点：

- 没有完成 UI 对结构化字段的接入
- 继续依赖脆弱字符串协议

### 方案 B：TUI / WebUI 都切到“结构化优先，字符串 fallback”

优点：

- 与 core/protocol/gateway 的完成态一致
- 可以平滑兼容旧历史事件
- 风险可控

缺点：

- 需要补类型和测试

### 方案 C：彻底移除字符串 fallback

优点：

- 最干净

缺点：

- 可能破坏旧 timeline / 历史事件 / 老测试
- 不符合当前阶段最短路径原则

推荐方案：B。

---

## 5. 分阶段设计

## Phase 1: TUI 适配

### 5.1 变更边界

允许修改：

- `ui/tui/src/app/reducer.rs`
- `ui/tui/src/app/tool_output.rs`
- `ui/tui/tests/` 中受影响的 tool 相关测试

不修改：

- core 事件生成
- gateway
- TUI 无关布局/交互

### 5.2 设计要点

- `Event::ToolCallCompleted` 分支应解构 `display_name`、`shell_metadata`
- completed cell 标题应优先使用 `display_name`
- `partial_success` 判断优先来自 `shell_metadata.result_class`
- backend 文案优先来自 `shell_metadata.backend`
- degraded 风险与 deny hint 优先来自：
  - `shell_metadata.backend`
  - `shell_metadata.policy_decision`
  - `shell_metadata.degrade_reason`
  - `shell_metadata.runtime_deny_reason`
- 旧 `output` 解析逻辑保留为 fallback，避免历史记录断裂

### 5.3 完成定义

- TUI 不再依赖 `output` 中必须出现 `backend=` / `result_class=` 才能显示关键信息
- 现有测试迁移到新事件结构
- partial/degraded/deny 场景测试覆盖结构化字段主路径

## Phase 2: WebUI 适配

### 5.4 变更边界

允许修改：

- `ui/web/src/types/gateway.ts`
- `ui/web/src/types/chat.ts`
- `ui/web/src/lib/session-events/tools.ts`
- `ui/web/src/lib/session-events/tools.test.ts`
- `ui/web/src/components/tool-steps/ToolStepCard.tsx`
- `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`
- 必要时 `MessageList` 相关测试

不修改：

- gateway
- WebUI 无关页面
- settings/provider 逻辑

### 5.5 设计要点

- 在类型层新增 `ShellExecutionMetadata`
- 为 `tool_call_completed` payload 建立更具体的读取逻辑
- reducer 不再把 completed 状态简单写死为 `success`
  - `ok=false` 时应反映失败
  - `result_class=partial_success` 时应映射为可辨识状态或说明
- 将结构化元数据折进 `ToolStep`
  - 可直接用 `description`
  - 或保留在 `meta`，再由卡片层负责展示
- ToolStepCard 展示应优先使用结构化信息生成：
  - sandbox backend 摘要
  - degraded 警示
  - runtime deny hint
  - partial success 提示

### 5.6 完成定义

- WebUI 在接收到 gateway 新 payload 后，卡片能展示 `shell_metadata` 导出的信息
- reducer 和组件测试覆盖 `partial_success`、`none_escalated`、`runtime_deny_reason`

---

## 6. 文件责任映射

### TUI

- `ui/tui/src/app/reducer.rs`
  - 负责把 `ToolCallCompleted` 的结构化字段传入渲染层
- `ui/tui/src/app/tool_output.rs`
  - 负责将 shell metadata 或 fallback output 转为用户可读摘要
- `ui/tui/tests/m12_tool_partial_status.rs`
  - 验证 partial success 展示
- `ui/tui/tests/m17_degraded_mutating_warning.rs`
  - 验证 degraded + mutating warning

### WebUI

- `ui/web/src/types/gateway.ts`
  - 负责 gateway 事件类型定义
- `ui/web/src/types/chat.ts`
  - 负责本地 ToolStep 承载结构
- `ui/web/src/lib/session-events/tools.ts`
  - 负责将 gateway tool 事件归并为 ToolStep
- `ui/web/src/components/tool-steps/ToolStepCard.tsx`
  - 负责渲染结构化 shell 信息
- `ui/web/src/lib/session-events/tools.test.ts`
  - 验证 reducer 归并语义
- `ui/web/src/components/tool-steps/ToolStepCard.test.tsx`
  - 验证卡片显示语义

---

## 7. 测试策略

### TUI

- 更新所有受影响的 `ToolCallCompleted` 构造
- 新增或调整测试覆盖：
  - `shell_metadata.result_class = partial_success`
  - `shell_metadata.backend = none_escalated`
  - `shell_metadata.runtime_deny_reason = skill_trigger_not_shell_command`
- 回归：
  - `zsh -lc "cargo test -p tui_next"`

### WebUI

- reducer 测试覆盖：
  - payload 中有 `shell_metadata`
  - `ok=false`
  - `partial_success`
  - degraded / runtime deny
- 组件测试覆盖：
  - 卡片能展示 backend summary
  - 卡片能展示 degraded/risk/hint 信息
- 回归：
  - `zsh -lc "cd ui/web && pnpm test"`

---

## 8. 风险与约束

### 8.1 TUI 风险

- 如果直接删除字符串 fallback，历史事件展示可能退化
- 若 completed cell 构造函数接口耦合过深，可能需要小范围重签名

### 8.2 WebUI 风险

- 若把 payload 类型收得过窄，可能影响现有通用事件处理
- 状态枚举若强行增加新值，可能引发额外 UI 改动；本轮更适合把细粒度语义先落在描述/meta 层

### 8.3 范围约束

- 本轮只整理 TUI / WebUI 适配 spec 和 plan
- 不回头修改 core / protocol / gateway
- 不把这轮扩展成 UI 风格重构
