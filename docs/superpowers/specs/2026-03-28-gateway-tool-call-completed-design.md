# Gateway ToolCallCompleted Shell Metadata Design

> 日期：2026-03-28
> 状态：待实施
> 基线提交：`54c30c65` `📝 docs(core): 对齐 native tool calling 剩余阶段收口文档`
> 范围：`openjax-gateway`
> 非目标：`ui/tui`、`ui/web`、`openjax-core` 运行时重构

---

## 1. 文档定位

本文只覆盖 `openjax-gateway` 对已完成 `openjax-protocol` / `openjax-core` 协议语义的适配收口，不重新设计工具事件模型，也不回退 core 已完成的数据契约。

当前事实基线：

- `openjax-protocol::Event::ToolCallCompleted` 已包含 `shell_metadata: Option<ShellExecutionMetadata>`。
- `openjax-core` 已完成 shell 结果语义拆分：
  - 模型消费：`model_content`
  - 展示/事件：`display_output`
  - 结构化字段：`shell_metadata`
- 本轮问题集中在 gateway 仍有旧映射，导致不同出口的 payload 结构不一致。

---

## 2. 问题定义

### 2.1 SSE / timeline 映射落后于协议

`openjax-gateway/src/event_mapper/tool.rs` 中 `Event::ToolCallCompleted` 当前仅映射：

- `tool_call_id`
- `tool_name`
- `ok`
- `output`
- `display_name`

缺少：

- `shell_metadata`

由于 gateway 的时间线持久化复用该 payload，SSE 与 timeline 会一起丢失新字段。

### 2.2 stdio 映射与 SSE 口径分叉

`openjax-gateway/src/stdio/dispatch.rs` 中 `tool_call_completed` 当前仅映射：

- `tool_name`
- `ok`
- `output`

缺少：

- `tool_call_id`
- `display_name`
- `shell_metadata`

这会让 gateway 的 stdio 出口与 HTTP/SSE 出口继续保持不同结构，不利于后续消费者统一处理。

### 2.3 测试口径未覆盖新字段

当前 gateway 测试未明确断言 `tool_call_completed.shell_metadata` 的透传，也未验证 stdio 与 SSE/timeline 在该事件上的 payload 一致性。

---

## 3. 设计目标

本轮 gateway 收口目标：

1. `tool_call_completed` 在 gateway 的 SSE / timeline / stdio 三条出口都透传 `shell_metadata`。
2. `tool_call_completed` 的 gateway payload 统一包含：
   - `tool_call_id`
   - `tool_name`
   - `ok`
   - `output`
   - `shell_metadata`
   - `display_name`
3. 只做协议对齐，不引入 UI 适配、不改 core 事件生成逻辑。
4. 补齐 gateway 侧测试，证明新字段在不同出口都能被观察到。

成功标准：

- `map_core_event_payload(Event::ToolCallCompleted { ... })` 返回的 payload 包含 `shell_metadata`。
- timeline 持久化后拉取的 `tool_call_completed` 事件 payload 保留 `shell_metadata`。
- stdio `map_event` 输出的 `tool_call_completed` payload 与 gateway SSE 口径一致。

---

## 4. 设计原则

### 4.1 不重设计协议

`shell_metadata`、`tool_call_id`、`display_name` 的权威来源已经在 `openjax-protocol` 与 `openjax-core` 确立，gateway 仅做透传和一致化，不重新定义字段语义。

### 4.2 保持最短路径

优先修改现有 mapper 和测试，不新增抽象层，不把这轮扩展成 UI 或 daemon 协议升级项目。

### 4.3 出口一致优先于局部兼容

gateway 既然同时维护 SSE/timeline 与 stdio 两条输出，就应保证同一事件在不同出口上的 payload 结构尽量一致。本轮至少先对齐 `tool_call_completed`，并顺手补齐 stdio 中同事件已经缺失的 `tool_call_id` / `display_name`。

### 4.4 持久化链路不做额外特判

`state/events.rs` 已按通用 `payload_json` 落盘。只要上游 mapper 输出完整 payload，就不应再在持久化层对 `shell_metadata` 做特殊处理。

---

## 5. 方案选择

### 方案 A：只补 SSE mapper，stdio 维持现状

优点：

- 改动最少

缺点：

- 无法满足“stdio 也跟上协议变化”的目标
- gateway 内部继续保留分叉 payload

### 方案 B：同时补 SSE/timeline 与 stdio，并补测试

优点：

- 符合本轮范围
- 一次收口 gateway 三条出口
- 风险可控，改动集中

缺点：

- 需要补几处测试

### 方案 C：抽公共 mapper，重构 SSE 与 stdio 共用映射逻辑

优点：

- 长期可减少重复

缺点：

- 超出本轮“最短路径适配”目标
- 会把这轮从协议对齐变成局部重构

推荐采用方案 B。

---

## 6. 实施边界

本轮允许修改：

- `openjax-gateway/src/event_mapper/tool.rs`
- `openjax-gateway/src/event_mapper/mod.rs` 内测试（如已有）
- `openjax-gateway/src/stdio/dispatch.rs`
- `openjax-gateway/src/state/events.rs` 相关测试或最小辅助代码（仅在需要构造 timeline 断言时）
- `openjax-gateway/tests/`
- `openjax-gateway/README.md`

本轮不修改：

- `openjax-core` 事件生成逻辑
- `openjax-protocol` 事件定义
- `ui/tui`
- `ui/web`

如果在验证过程中发现 TUI/WebUI 仍依赖旧形状，只记录为后续 phase，不混入本轮实现。

---

## 7. 测试策略

### 7.1 单元/模块测试

- 为 `event_mapper` 增加 `ToolCallCompleted` payload 断言：
  - 包含 `tool_call_id`
  - 包含 `display_name`
  - 包含 `shell_metadata`

- 为 stdio `map_event` 增加 `ToolCallCompleted` payload 断言：
  - 与 SSE 口径一致

### 7.2 timeline 验证

如果现有 gateway 状态层测试便于直接构造 `ToolCallCompleted` 事件，则补一条 timeline/持久化断言，证明 `shell_metadata` 会被持久化并在 timeline 查询中返回。

若无现成低成本入口，则至少通过 `map_core_event_payload + state/events` 现有通用 payload 落盘逻辑做局部验证，不引入重型集成搭建。

### 7.3 回归验证

- `zsh -lc "cargo test -p openjax-gateway"`
- `zsh -lc "cargo build -p openjax-gateway"`

如需更快回归，可先跑目标测试，再跑 crate 全量测试。

---

## 8. 风险与后续

### 8.1 风险

- stdio 历史消费者可能默认只读取旧 payload 字段；但本轮是追加字段和补齐缺失标识，不改变已有字段含义，风险可控。
- 若 `openjax-gateway/tests/gateway_api.rs` 缺少便捷入口生成真实 `tool_call_completed`，timeline 断言可能更适合放在状态层测试而不是 HTTP 集成测试。

### 8.2 明确留给后续 phase 的事项

- `ui/tui` 对 `shell_metadata` 的消费升级
- `ui/web` 对 `shell_metadata` 的展示与类型升级
- gateway SSE 与 stdio 更大范围的公共 mapper 收敛重构
