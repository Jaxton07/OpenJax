# OpenJax TUI 视口与历史回卷重构计划（对齐 Codex 方案）

## 摘要
当前问题的根因不是单点 bug，而是终端层缺少 Codex 的“持久 viewport + history 插入队列 + custom terminal”架构。
本计划将把 `openjax-tui` 从“补丁式 stdout ANSI 插入”升级为“viewport-aware 渲染内核”，彻底解决：
1. 长会话上翻看不到 OpenJax 历史（先掉到 shell 历史）
2. 历史与主视图混排/重叠
3. overflow 计数与实际插入区域不一致

## 现状根因（锁定）
1. `openjax-tui` 没有持久化 `viewport_area` 状态，只有每帧临时高度计算，历史插入与渲染不在同一坐标系。
2. overflow 逻辑基于全屏高度计算，但插入逻辑基于临时 `viewport_top`，语义错位。
3. 历史插入直接走 stdout ANSI，`ratatui::Terminal` 双缓冲不知道外部写入，导致视觉不一致。
4. 缺少 Codex 的 `pending_history_lines -> draw阶段统一插入 -> viewport_area更新` 机制。

## 重构目标（验收定义）
1. inline 模式（`OPENJAX_TUI_ALT_SCREEN=never`）上翻优先看到 OpenJax 会话历史（而非 shell 历史）。
2. 历史写入与主视图渲染无重叠、无跳动、无行错位。
3. 样式（颜色/修饰符）在 scrollback 中尽量保持一致。
4. alt-screen 与 inline 两种模式行为边界清晰。
5. 终端 resize 后历史/视口仍正确。

## 目标架构
### 终端层分层
- `openjax-tui/src/custom_terminal.rs`：管理 `viewport_area` / 屏幕尺寸 / 光标状态。
- `openjax-tui/src/insert_history.rs`：统一历史插入实现。
- `openjax-tui/src/tui.rs`：orchestrator（事件流、draw 调度、alt-screen、history queue）。

### 状态模型
- `Tui` 内部包含：
  - `pending_history_lines: Vec<Line<'static>>`
  - `terminal_state.viewport_area`
  - `alt_screen` 策略状态
- 历史插入不在 `main.rs` 临时计算 overflow 后直接写 stdout。

### 渲染主路径
固定为：
1. 计算并更新 `viewport_area`（依赖 `desired_height`）
2. 先插入 `pending_history_lines` 到 viewport 上方
3. 再 `draw` 当前帧
4. 若插入导致 viewport 变化，回写 `terminal_state`

## 实施步骤
### Phase A：终端内核迁移
1. 引入 `custom_terminal` 状态模型（viewport/screen/cursor）。
2. `tui.rs` 统一入口：`Tui::draw(height, ...)` / `Tui::insert_history_lines(...)`。
3. `main.rs` 改为通过 `Tui` 绘制，不再直接 `terminal.draw`。

### Phase B：历史插入统一化
1. `insert_history.rs` 提供 `insert_history_lines(writer, state, lines)`。
2. 采用 scroll region + cursor restore。
3. 支持 line-level + span-level 样式合并。

### Phase C：App 层职责收敛
1. 删除 `app.rs` 过渡逻辑：`scrollback_overflow_lines` / `scrollback_overflow_render_lines`。
2. 保留：
  - `desired_height(width)`
  - `render_in_area(frame, area)`
  - chat 视觉行滚动逻辑

### Phase D：alt-screen/inline 策略对齐
1. `OPENJAX_TUI_ALT_SCREEN=auto` 保持自动策略。
2. inline 下启用 history 插入；alt-screen 下不向普通 scrollback 注入历史。
3. README 明确两种模式差异。

### Phase E：测试补齐
1. `m12_inline_history_scrollback.rs`：长会话上翻可见 OpenJax 历史。
2. `m13_viewport_resize.rs`：resize 后 viewport 与历史插入一致。
3. `m14_alt_screen_behavior.rs`：alt-screen/inline 行为边界。
4. 回归：`m1_app_state`、`m3_render_smoke`、`m11_chat_scroll_visual`。

## 失败模式与防护
1. `viewport_top == 0`：视为异常路径，需降级插入并记录日志。
2. 历史重复插入：通过 queue 消费语义保证 exactly-once。
3. 样式泄漏：每行写入后 reset colors/modifiers。
4. resize 抖动：draw 前执行 viewport 修正，保持 cursor 与 viewport 一致。

## 执行命令
```bash
zsh -lc "cargo fmt"
zsh -lc "cargo build -p openjax-tui"
zsh -lc "cargo test -p openjax-tui"
zsh -lc "cargo test -p openjax-tui --test m12_inline_history_scrollback"
zsh -lc "cargo test -p openjax-tui --test m13_viewport_resize"
zsh -lc "cargo test -p openjax-tui --test m14_alt_screen_behavior"
```

手动验收：
```bash
zsh -lc "OPENJAX_TUI_ALT_SCREEN=never cargo run -q -p openjax-tui"
```

## Codex 参考索引（锁定）

### 终端 orchestrator 与 viewport 生命周期
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/tui.rs:241`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/tui.rs:443`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/tui.rs:448`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/tui.rs:478`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/tui.rs:494`

### 历史插入算法（scroll region + style merge + viewport update）
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/insert_history.rs:27`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/insert_history.rs:43`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/insert_history.rs:57`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/insert_history.rs:88`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/insert_history.rs:95`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/insert_history.rs:113`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/insert_history.rs:131`

### custom terminal 状态模型
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/custom_terminal.rs:117`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/custom_terminal.rs:161`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/custom_terminal.rs:227`
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/custom_terminal.rs:265`

### App 层历史入队触发点
- `/Users/ericw/work/code/ai/codex/codex-rs/tui/src/app.rs:1545`
