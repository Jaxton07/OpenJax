# OpenJax Python TUI UI 与交互说明（用于 Rust 重写）

本文档基于当前 `python/tui` 实现与测试用例整理，目标是给 Rust 版 TUI 提供可落地的 UI/交互规格参考，不包含具体实施计划。

## 1. 目标与范围

- 对象：`python/tui/src/openjax_tui`（Textual 版）
- 目标：描述“用户可见 UI + 交互行为 + 事件驱动规则 + 状态模型”
- 非目标：重构方案、技术选型比较、开发排期

## 2. 模块分层（实现视角）

- 应用编排层：`app.py`
  - 启动/停止 runtime
  - 接收 daemon 事件并转成 UI 操作
  - 管理审批动作、slash 命令动作、错误上报
- 页面层：`screens/chat.py`
  - 主聊天页布局
  - 键盘事件分发（审批优先于命令候选）
  - 将 `AppState` 渲染到 `RichLog`
- 状态层：`state.py`
  - 会话状态、消息列表、审批队列、流式缓冲、错误状态
- 事件映射层：`event_mapper.py`
  - daemon 事件 -> 状态变更 + `UiOperation`
- 组件层：`widgets/*.py`
  - `ChatInput` 输入框
  - `CommandPalette` 命令候选层
  - `ApprovalPopup` 审批弹窗
  - `ThinkingStatus` 思考动画
  - `MarkdownMessage` 助手 Markdown 渲染包装
- SDK 运行时层：`sdk_runtime.py`
  - `start_session` / `submit_turn` / `stream_events` / `resolve_approval` / `shutdown_session`

## 3. 主界面布局规格

## 3.1 布局结构

`ChatScreen.compose()` 结构固定：

1. `Vertical#chat-container`
2. `RichLog#chat-log`（消息展示区，占满剩余高度）
3. `ChatInput#chat-input`（底部输入框）

动态插入组件（都挂在 `#chat-container` 且位于输入框上方）：

- `#command-palette`（slash 命令候选）
- `#approval-popup`（审批弹窗）
- `#thinking-status`（思考状态）

三者是“输入框上方的临时层”，并且存在互斥规则（见第 6 节）。

## 3.2 首屏文案与焦点

- `on_mount` 后 chat-log 写入欢迎文案：
  - 第一行：欢迎语
  - 第二行：输入说明 + 退出键说明
- 退出键平台差异：
  - macOS: `Ctrl+C`
  - Linux/Windows: `Ctrl+Q`
- 首屏刷新后聚焦输入框（若无审批弹窗）。

## 3.3 消息视觉约定

- 用户消息：`❯` 前缀，蓝色符号，灰底高亮整行
- 助手消息（最终态）：
  - 默认按 Markdown 渲染（`MarkdownMessage`）
  - 非 markdown 渲染时用 `⏺` 绿色前缀
- 工具消息：
  - 统一前缀 `⏺`
  - 成功绿色，失败红色
  - 可追加目标后缀：`(target)`
- 系统消息：按 Rich markup 直接渲染
- 行间距：每条消息后额外插入空行（可读性）

## 4. 视觉样式与主题策略（styles.tcss）

- 整体背景透明（`App/Screen/ChatScreen/RichLog/Input` 均 transparent）
- 滚动条背景透明，避免遮挡终端主题
- 输入框边框：
  - 默认：`tall $border`
  - focus：`tall $primary`
- 候选层：
  - `#command-palette`：round 边框，主色
  - `#approval-popup`：round 边框，次级灰色
- 思考状态：
  - `#thinking-status` 使用主色文本
- 鼠标上报默认关闭（通过环境变量可开）

对 Rust 重写的含义：

- UI 应保留“终端主题友好”的透明/低侵入风格；
- 输入框与临时层都在底部区域组织，保证单手键盘操作路径短。

## 5. 状态模型规格（AppState）

## 5.1 核心字段

- 会话：
  - `session_id: str | None`
  - `turn_phase: TurnPhase`（`IDLE/THINKING/STREAMING/ERROR`）
- 消息：
  - `messages: list[Message]`
- 审批：
  - `pending_approvals: dict[id, ApprovalRequest]`
  - `approval_order: list[id]`
  - `approval_focus_id: str | None`
- 流式：
  - `active_turn_id: str | None`
  - `stream_text_by_turn: dict[turn_id, aggregated_text]`
  - `turn_render_kind_by_turn: dict[turn_id, "plain"|"markdown"]`
  - `tool_target_hints: dict[(turn_id, tool_name), queue[target]]`
- 错误：
  - `last_error: str | None`

## 5.2 状态行为约束

- `start_turn(turn_id)`：
  - 初始化 turn 缓冲
  - phase -> `THINKING`
- `append_delta(turn_id, delta)`：
  - 追加流式文本
  - phase -> `STREAMING`
- `finalize_turn(turn_id)`：
  - 清理活跃 turn 与缓冲
  - phase -> `IDLE`
- `add_approval(...)`：
  - 入队并将焦点设为该审批
- `resolve_approval(id)`：
  - 出队，若当前焦点被移除，则焦点回退到队尾

## 6. 交互规则（用户行为）

## 6.1 普通消息发送

1. 用户在输入框回车提交文本。
2. 若非空且不以 `/` 开头：
  - 立即追加用户消息到消息区
  - phase 切到 `THINKING`
  - 异步调用 `submit_turn(text)`
3. 输入框清空。

## 6.2 Slash 命令候选

触发条件：

- 输入变化时，`value.startswith("/")` 才允许显示候选层；
- 非 slash 模式立即关闭候选层。

候选行为：

- 候选由 `create_commands()` 固定提供：
  - `/help` `/clear` `/exit` `/pending` `/approve` `/deny`
- 支持模糊匹配评分（优先级）：
  - 精确名匹配 > 前缀 > 子串 > 描述命中 > 字符顺序模糊匹配
- `Up/Down` 移动选中（不循环）
- `Enter` 执行当前最优候选
- `Esc` 关闭候选层

关键约束：

- 审批弹窗激活时，禁止候选层打开；
- 候选层显示时会隐藏思考状态；
- 候选层关闭后根据 phase 恢复思考状态。

## 6.3 Slash 命令执行语义

- `/help`：输出帮助说明系统消息
- `/clear`：清空历史消息，再输出“已清空”系统消息
- `/exit`：退出程序
- `/pending`：列出待审批数量与条目（带 focus 标记）
- `/approve`：批准当前焦点审批
- `/deny`：拒绝当前焦点审批

无匹配命令时输出黄色系统消息提示。

## 6.4 审批弹窗交互

触发：

- 收到 `approval_requested` 事件后，弹窗自动出现并抢焦点。

行为：

- 弹窗显示摘要 + 3 个选项：
  - `Approve`
  - `Deny`
  - `Cancel or decide later`
- 按键：
  - `Up/Down` 选择
  - `Enter` 确认
  - `Esc` 视作 `cancel`

互斥与优先级：

- 弹窗显示时：
  - 输入框 disabled
  - 命令候选层关闭
  - 思考状态隐藏
- 键盘优先级：审批弹窗 > 命令候选层

`cancel` 语义：

- 仅关闭弹窗并提示“审批已暂存”，不向 daemon 发送 resolve 请求。

## 6.5 思考状态指示器

- 仅在 `turn_phase == THINKING` 且“无审批弹窗、无命令候选层”时显示
- 文案：`Thinking` + 5 点流动动画
- 动画：
  - 刷新间隔 `0.06s`
  - 相位步长 `0.28`
  - 到尾端后短暂停顿再回到起点

## 7. 事件驱动规格（daemon -> UI）

由 `event_mapper.map_event` 定义：

1. `turn_started`
  - `state.start_turn(turn_id)`
  - 产出 `phase_changed`
2. `assistant_delta`
  - 追加增量到 turn 缓冲
  - 产出 `stream_updated` + `phase_changed`
3. `assistant_message`
  - 用 payload `content` 覆盖该 turn 流文本
  - render_kind 标记为 `markdown`
  - 产出 `stream_updated` + `phase_changed`
4. `tool_call_started`
  - 若携带 `target`，记录 target hint（turn+tool 级队列）
5. `tool_call_completed`
  - 生成一条 tool 消息（label + ok + preview + target）
  - target 优先级：hint 队列 > 输出文本解析
  - 产出 `tool_call_completed`
6. `approval_requested`
  - 入 pending 队列
  - 产出 `approval_added`
7. `approval_resolved`
  - 从 pending 队列移除
  - 产出 `approval_removed`
8. `turn_completed`
  - 从缓冲 finalize，得到最终文本
  - 产出 `turn_completed` + `phase_changed`

`OpenJaxApp._apply_ui_operations` 的渲染策略：

- `stream_updated`：走节流刷新（40ms 定时）
- 其他 `needs_render` 事件：立即全量 render
- 审批增删：render 后同步弹窗显示/隐藏
- `turn_completed`：把最终文本落为 assistant 消息（render_kind 默认 markdown）

## 8. 工具消息映射规则

tool_name -> 默认 label：

- `read_file` -> `Read 1 file`
- `apply_patch/edit_file_range/write_file` -> `Update 1 file`
- `list_dir` -> `Read directory`
- `grep_files` -> `Search files`
- `shell` -> `Run shell command`
- 其他 -> `Title Case` 名称

target 提取规则：

- `read_file`：从 `READ ...`、`path=...`、`file=...` 提取
- patch/write/edit：从 `UPDATE <path>` 提取

## 9. 错误与可观测性

- runtime 错误处理：
  - 记录日志
  - `last_error` 更新
  - UI 插入红色错误系统消息
  - phase 最终回到 `IDLE`
- 日志：
  - 文件：`.openjax/logs/openjax_tui.log`
  - 轮转：`2 MiB * 5`（可环境变量覆盖）
  - `OPENJAX_TUI_DEBUG=1` 开 debug 级别

## 10. 键位与行为矩阵（可直接映射到 Rust）

- 全局：
  - `Ctrl+C`(macOS) / `Ctrl+Q`(其他) -> 退出
- 输入框提交：
  - `Enter` -> 提交文本或执行 slash 命令
- 命令候选层（slash 模式）：
  - `Up/Down` -> 移动选择
  - `Esc` -> 关闭
  - `Enter` -> 执行
- 审批弹窗：
  - `Up/Down` -> 移动选择
  - `Enter` -> 确认
  - `Esc` -> cancel

冲突优先级：

1. 审批弹窗
2. 命令候选层
3. 输入框默认行为

## 11. Rust 重写时必须保持的一致性（建议作为验收项）

- 视觉层级：消息区 + 底部输入框 + 输入框上方临时层
- slash 触发条件：仅输入首字符为 `/` 时展示候选
- 审批机制：弹窗抢焦点、输入禁用、支持 approve/deny/cancel
- 流式收敛：delta 可见、turn_completed 后落最终消息并清理缓冲
- 消息语义：用户/助手/tool/system 四类格式与颜色约定
- phase 驱动：THINKING 时显示思考态，但被审批/候选层抑制
- 工具结果：简洁单行 + 成功失败颜色 + 可选 target
- 错误可见性：界面提示 + 日志落盘

## 12. 可作为 Rust 版的最小接口草案（非实现计划）

建议将 Rust TUI 抽象为以下边界（名称可调整）：

- `AppState`（纯状态，不依赖 UI 框架）
- `EventMapper`（daemon event -> state diff + ui op）
- `Renderer`（state -> view model）
- `InputController`（键盘事件路由与优先级）
- `RuntimeAdapter`（会话生命周期与事件流）

这样可以维持当前 Python 版“状态机与 UI 解耦”的优点，便于后续测试迁移。
