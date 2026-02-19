# OpenJax TUI 技术方向（精简版）

本文档用于后续快速进入 TUI 优化工作，不追求完整背景，只保留高价值工程信息。

## 1. 当前状态（已落地）

- TUI 已接入 `openjax-core::Agent`，提交输入后通过 `submit_with_sink` 消费事件流。
- 审批链路已打通：
  - 协议事件：`ApprovalRequested` / `ApprovalResolved`
  - UI 弹层展示审批请求
  - `y/n` 可直接给出审批决策并回填到 core
- 增量输出链路已打通：
  - 协议事件：`AssistantDelta`
  - UI 侧合并到同一 assistant 消息
- 终端行为已具备基础稳定性：
  - raw mode 进入/恢复
  - alternate screen 策略：`OPENJAX_TUI_ALT_SCREEN=auto|always|never`
- 测试基线覆盖：
  - `m1~m9`（状态、映射、渲染、审批、增量、键位、终端恢复、审批 handler）

## 2. 核心代码地图（优先阅读顺序）

1. `openjax-tui/src/main.rs`
   - 主循环与 agent 集成入口
   - turn 后台任务 + core event 回流
2. `openjax-tui/src/app.rs`
   - UI 事件处理与渲染主入口
   - help/approval overlay 渲染
3. `openjax-tui/src/state.rs`
   - `Event -> UIState` 映射核心
   - assistant delta 合并逻辑
4. `openjax-tui/src/approval.rs`
   - `TuiApprovalHandler`（UI 决策 -> core 审批）
5. `openjax-tui/src/tui.rs`
   - 键盘映射
   - 终端进入/恢复与 alt screen 策略

关联 core/protocol：
- `openjax-protocol/src/lib.rs`（`Event` 变体定义）
- `openjax-core/src/approval.rs`（审批请求结构、handler trait）
- `openjax-core/src/lib.rs`（`submit_with_sink`、工具事件并入）

## 3. 运行与验证

开发运行：
- `zsh -lc "cargo run -p openjax-tui"`

推荐环境变量：
- `OPENAI_API_KEY=...`
- `OPENJAX_MODEL=...`（可选）
- `OPENJAX_APPROVAL_POLICY=on_request|never`
- `OPENJAX_SANDBOX_MODE=workspace_write`
- `OPENJAX_TUI_ALT_SCREEN=auto|always|never`

测试回归：
- `zsh -lc "cargo test -p openjax-tui"`
- `zsh -lc "cargo test -p openjax-core"`
- `zsh -lc "cargo test -p openjax-cli"`

## 4. 现阶段设计约束（优化前先确认）

- 当前是单会话 UI，尚无多会话/多线程 UI 编排。
- turn 执行是“单任务串行”模型；并发 turn 尚未开放。
- Markdown 渲染目前是轻量文本转换（非完整 markdown 引擎）。
- 审批事件可视化已打通，但审批 UX 仍偏基础（缺少上下文细节/快捷操作提示优化）。

## 5. 优化优先级（建议顺序）

1. 渲染质量
   - 完整 markdown（标题/列表/代码块/引用/链接）渲染质量提升
   - 长消息滚动和代码块可读性优化
2. 交互体验
   - 输入编辑能力（光标移动、多行编辑、历史输入）
   - 审批弹层交互增强（默认键、高亮、安全提示）
3. 事件节奏
   - assistant delta 的节流/分块刷新策略（平滑与追赶模式）
4. 稳定性
   - panic/异常退出下终端恢复兜底
   - tmux/zellij 差异化行为测试

## 6. 常见坑（已踩过）

- 不能把 turn 执行放在阻塞主循环，否则审批无法在 UI 中决策。
- 审批必须带稳定 `request_id`，否则 UI 决策与 core 请求无法一一对应。
- raw mode 下 stdin 审批会和 TUI 事件循环冲突；必须走 TUI handler。
- 终端恢复逻辑必须做 guard，避免异常路径遗留 raw/alt screen 状态。

## 7. 文档索引（按用途）

- 架构参考：`docs/codex-tui-architecture.md`
- 实施与阶段记录：`docs/plan/tui/tui-design-and-implementation-plan.md`
- 本文档：`docs/tui/technical-direction.md`

