# OpenJax Rust TUI 重写执行计划（Codex 架构同构版）

## 1. 目标与范围

- 范围：一次到位对齐 Codex TUI 的分层与关键交互能力。
- 策略：架构同构 + OpenJax 协议接口重写，不引入 Codex 内部依赖。
- 参考：
  - `docs/plan/refactor/tui/python-tui-ui-interaction-spec.md`
  - `/Users/ericw/work/code/ai/codex/docs/tui-architecture.md`
  - `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/*`

## 2. 技术选型

- UI: `ratatui`
- Terminal: `crossterm`
- Async runtime: `tokio`
- Logging: `tracing`
- Markdown: `pulldown-cmark`
- Text wrapping: `textwrap`
- Input grapheme compatibility: `unicode-segmentation`
- Snapshot testing: `insta`

## 3. 目标架构

- `src/app.rs`: 主状态机与事件路由
- `src/tui.rs`: 终端模式与键位映射
- `src/app_event.rs`: 统一事件入口
- `src/chatwidget.rs`: transcript 渲染与流式尾部
- `src/bottom_pane/`
  - `chat_composer.rs`
  - `slash_commands.rs`
  - `command_popup.rs`
  - `approval_overlay.rs`
  - `footer.rs`
- `src/state/`
  - `app_state.rs`
  - `turn_state.rs`
  - `approval_state.rs`
  - `input_state.rs`
  - `event_mapper.rs`
- `src/render/renderable.rs`: 可渲染接口

## 4. 交互规范

### 4.1 布局

- 顶部：Logo/运行上下文
- 中部：聊天 transcript（含流式）
- 底部：composer + slash popup + approval popup + footer

### 4.2 输入与 Slash

- 仅 `input.startswith("/")` 打开命令候选
- 候选排序：精确 > 前缀 > 子串 > 描述 > 模糊序列
- 键位：`Up/Down` 选择，`Enter` 执行，`Esc` 关闭
- 首版命令：`/help /clear /exit /pending /approve /deny`

### 4.3 审批

- 队列模型：FIFO + focus
- 审批出现时：禁用输入、关闭 slash、显示审批弹层
- 快捷键：`y/n` 快速决策，`Enter` 确认选中，`Esc` 暂存

### 4.4 流式与消息

- `assistant_delta` 聚合到 active 消息
- `assistant_message` 以 markdown 渲染标记收敛
- `turn_completed` 清理流缓冲并回到 `IDLE`
- tool 消息：单行 label + 成功/失败色 + 可选 target

## 5. 分步实施计划

1. 事件与终端：统一事件模型，加入更稳健的键位映射与恢复流程。
2. 状态拆分：将单体 state 拆分为 turn/approval/input/transcript 子状态。
3. ChatWidget：实现消息渲染、流式拼接、tool 卡片化行展示。
4. BottomPane：实现输入状态机、slash popup、审批 popup、footer。
5. 命令执行：接入 slash 命令执行与审批回传链路。
6. 审批闭环：队列处理 + 回传 `TuiApprovalHandler`。
7. 视觉收敛：深色高对比，低噪音，弹层优先级清晰。
8. 测试收敛：状态/映射/键位/渲染/审批全覆盖。

## 6. 测试清单

- 流式：delta 聚合、final 覆盖、turn 完结清理
- slash：打开/关闭、排序、空匹配
- 审批：单请求、多请求、焦点迁移、cancel 语义
- 输入：光标移动、历史恢复、提交清理
- 终端：alt-screen 模式与恢复流程
- 渲染：markdown 代码块、tool target 后缀、小窗口

## 7. 已实施结果（本次）

- 已完成状态拆分与模块重组（`state/*`, `bottom_pane/*`, `chatwidget`, `renderable`）。
- 已完成 slash 命令候选、审批弹层优先级、footer 状态提示。
- 已完成流式/消息渲染与 tool 行语义显示。
- 已完成协议事件映射迁移至 `state/event_mapper.rs`。
- 已完成依赖升级与测试迁移。
- 验证：`cargo test -p openjax-tui` 全绿。

## 8. 后续增强项

- `request_user_input` 多题弹层
- 更完整的 markdown 样式（列表缩进、代码语言主题）
- streaming commit tick 节流策略（Smooth/CatchUp）
- 更细粒度快照测试（vt100 级 UI 回归）
