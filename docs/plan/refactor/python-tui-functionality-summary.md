# Python TUI 功能点与需求梳理

## 1. 项目概述

`openjax_tui` 是基于 `openjax_sdk` 的 Python TUI MVP，为 OpenJax 提供交互式命令行界面。采用模块化架构，支持双后端输入（prompt_toolkit / basic），具备完整的事件驱动渲染、审批工作流、状态动画等能力。

---

## 2. 核心功能模块

### 2.1 应用生命周期管理 (app.py)

| 功能点 | 描述 |
|--------|------|
| 启动流程 | 初始化 logger → 选择输入后端 → 创建客户端 → 启动会话 → 打印 Logo → 启动事件循环 |
| 事件循环 | `_event_loop()` 轮询守护进程事件，超时 0.5s，处理协议错误 |
| 优雅关闭 | 处理 KeyboardInterrupt、CancelledError，关闭动画，清理会话 |
| 键盘增强 | 支持 Kitty/WezTerm 键盘协议（CSI u），可通过环境变量启用 |
| 回调组装 | 将各模块函数组装为闭包，降低模块间耦合 |

**关键配置:**
- `OPENJAX_DAEMON_CMD`: 守护进程启动命令
- `OPENJAX_TUI_ENABLE_KEYBOARD_ENHANCEMENT`: 启用键盘增强协议

---

### 2.2 状态管理 (state.py)

集中式状态管理，使用 `AppState` 类跟踪所有运行时状态：

| 状态类别 | 包含字段 |
|----------|----------|
| 运行状态 | `running`, `session_id`, `input_backend`, `turn_phase` |
| 审批状态 | `pending_approvals`, `approval_order`, `approval_focus_id`, `approval_selected_action` |
| 流式状态 | `stream_turn_id`, `stream_text_by_turn`, `turn_block_index` |
| 工具统计 | `tool_turn_stats`, `active_tool_starts`, `active_tool_display_label_by_turn` |
| 历史视口 | `history_blocks`, `history_auto_follow`, `history_manual_scroll`, `stream_block_index` |
| 动画状态 | `animation_lifecycle`, `animation_task`, `animation_frame_index` |
| Live Viewport | `view_mode`, `live_viewport_owner_turn_id`, `live_viewport_turn_ownership` |
| UI 回调 | `prompt_invalidator`, `history_setter` |

**视图模式 (ViewMode):**
- `LIVE_VIEWPORT` (默认): scrollback-first 模式，活动 turn 保留在视口
- `SESSION`: 稳定兼容模式，会话块视图

---

### 2.3 事件处理系统

#### 2.3.1 事件分发 (event_dispatch.py)

处理来自守护进程的各类事件：

| 事件类型 | 处理逻辑 |
|----------|----------|
| `assistant_delta` | 流式内容增量渲染 |
| `assistant_message` | 最终消息渲染（权威性校验）|
| `tool_call_started` | 记录工具开始时间，更新状态 |
| `tool_call_completed` | 计算耗时，打印结果行 |
| `approval_requested` | 触发审批 UI，打印请求信息 |
| `approval_resolved` | 清理审批状态 |
| `turn_started` | 标记 turn 开始 |
| `turn_completed` | 打印工具统计摘要 |

#### 2.3.2 事件状态管理 (event_state_manager.py)

统一处理 turn phase 转换、审批状态更新、live viewport ownership 管理：

- **Phase 转换**: `idle` → `thinking` → `tool_wait` → `idle`
- **审批状态**: 跟踪 pending/resolving/resolved 状态
- **Live Viewport**: 管理 turn 对视口的占用和释放

---

### 2.4 输入系统

#### 2.4.1 后端选择 (input_backend.py)

| 后端 | 触发条件 | 特性 |
|------|----------|------|
| `prompt_toolkit` | TTY 环境 + 依赖可用 | 富 TUI，圆角边框，快捷键 |
| `basic` | 非 TTY / 依赖缺失 / 强制指定 | 基础 input()，兼容性好 |

**环境变量控制:**
- `OPENJAX_TUI_INPUT_BACKEND`: 强制指定后端
- `OPENJAX_TUI_INPUT_BOTTOM_OFFSET`: 输入框距底部行数（默认 10）

#### 2.4.2 Basic 输入循环 (input_loops.py)

- 使用线程队列异步读取输入
- 支持 readline 键位绑定（macOS libedit 兼容）
- ANSI 序列清理（箭头键、功能键）

#### 2.4.3 Prompt Toolkit 运行时 (prompt_runtime_loop.py)

复杂 UI 布局组装：

```
┌─────────────────────────────────────┐
│  History Viewport (scrollable)      │  ← 历史消息区域
├─────────────────────────────────────┤
│  Status Line                        │  ← 状态指示器
├─────────────────────────────────────┤
│  ╭─────────────────────────────╮   │
│  │ Input Area (multiline)      │   │  ← 输入框（圆角边框）
│  ╰─────────────────────────────╯   │
├─────────────────────────────────────┤
│  Slash Hint                         │  ← 命令提示
├─────────────────────────────────────┤
│  Approval Panel                     │  ← 审批区域
└─────────────────────────────────────┘
```

**视口适配器:**
- `PilotHistoryViewportAdapter`: scrollback-first 推荐实现
- `TextAreaHistoryViewportAdapter`: 兼容回退实现

---

### 2.5 审批工作流 (approval.py)

完整的权限审批系统：

| 功能 | 描述 |
|------|------|
| 多审批队列 | 支持同时存在多个 pending 审批请求 |
| 焦点导航 | 上下切换审批焦点，Enter 确认 |
| 快速响应 | `y`/`n` 快速回传最新审批 |
| 过期处理 | 自动检测超时/已处理审批 |
| 内联面板 | prompt_toolkit 模式下底部显示审批详情 |

**审批面板显示:**
```
────────────────────────────────────────
 Permission Request
 Action: <target>
 Reason: <reason>
 Choose an option:
 ❯ 1. Yes
   2. No
 Up/Down switch · Enter confirm · Esc reject
```

---

### 2.6 渲染系统

#### 2.6.1 助手消息渲染 (assistant_render.py)

| 功能 | 描述 |
|------|------|
| 流式增量 | `assistant_delta` 实时追加内容 |
| 最终消息 | `assistant_message` 权威性覆盖 |
| 多行对齐 | 自动添加续行前缀（2空格）|
| Turn 块管理 | `_upsert_turn_block` 更新或追加块 |

#### 2.6.2 工具运行时渲染 (tool_runtime.py)

- 工具开始/完成时间跟踪
- 执行时长格式化（ms/s/m+s 自动切换）
- 彩色状态指示器（绿/红）
- 工具结果标签（Read/Update/Search/Shell 等）
- Unicode 宽度正确处理（CJK、Emoji）

**输出示例:**
```
⏺ Read 1 file (test.txt) · 1ms
⏺ Update(src/lib.rs) · 15ms
⏺ Search files · 45ms
```

---

### 2.7 状态动画 (status_animation.py)

动态状态指示器系统：

| 阶段 | 动画帧 | 触发条件 |
|------|--------|----------|
| `thinking` | `""`, `"."`, `".."`, `"..."` | turn_phase == "thinking" |
| `tool_wait` | `"."`, `".."`, `"..."` | turn_phase == "tool_wait" |

- 动画间隔: 1/7 秒（约 7fps）
- 生命周期管理: `IDLE` → `PREPARING` → `ACTIVE` → `SETTLING` → `IDLE`
- 与 prompt_toolkit 集成，触发重绘

---

### 2.8 斜杠命令 (slash_commands.py)

| 命令 | 功能 |
|------|------|
| `/approve <id> y\|n` | 指定审批响应 |
| `/pending` | 查看待处理审批队列 |
| `/help` | 显示帮助信息 |
| `/exit` | 退出应用 |

- Tab 自动补全支持
- 输入时实时提示可用命令

---

### 2.9 日志系统

#### 2.9.1 TUI 日志 (tui_logging.py)

- RotatingFileHandler，默认 2MB，保留 5 个备份
- 日志路径: `.openjax/logs/openjax_tui.log`
- 调试模式: `OPENJAX_TUI_DEBUG=1`

#### 2.9.2 会话日志 (session_logging.py)

结构化事件日志：
- 启动摘要（版本、会话 ID、后端类型）
- 审批事件审计（请求、响应、过期）
- 格式: `action=<action> request_id=<id> turn_id=<id> ...`

---

## 3. 视图模式详解

### 3.1 Live Viewport 模式（默认）

**设计理念:** scrollback-first，降低长会话截断风险

**工作流程:**
1. 活动 turn 内容实时更新在视口
2. turn 完成后，内容 flush 到终端 scrollback
3. 视口仅保留当前活动 turn
4. 历史块定期压缩，防止内存无限增长

**环境变量:**
- `OPENJAX_TUI_VIEW_MODE=live` (或 `live_viewport`)
- `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=pilot` (推荐) / `textarea` (回退)
- `OPENJAX_TUI_HISTORY_WINDOW_LINES`: 历史窗口最大行数（默认 500）

### 3.2 Session 模式

**设计理念:** 稳定兼容，会话块视图

- 所有历史消息保留在视口
- 适合作为保底回退配置
- 无 scrollback flush 行为

---

## 4. Rollout 与回退策略

### 4.1 推荐启动命令

**标准模式:**
```bash
OPENJAX_TUI_VIEW_MODE=live \
OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=pilot \
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

**保底回退:**
```bash
OPENJAX_TUI_VIEW_MODE=session \
OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=textarea \
OPENJAX_TUI_INPUT_BACKEND=basic \
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

### 4.2 排障顺序

1. 默认使用 `live` + `pilot`
2. 视口异常 → 回退 `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=textarea`
3. 仍异常 → 整体回退到 `OPENJAX_TUI_VIEW_MODE=session`
4. 必要时 `OPENJAX_TUI_INPUT_BACKEND=basic`

---

## 5. 环境变量汇总

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `OPENJAX_TUI_INPUT_BACKEND` | 输入后端 | 自动检测 |
| `OPENJAX_TUI_VIEW_MODE` | 视图模式 | `live` |
| `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL` | 历史视口实现 | `pilot` |
| `OPENJAX_TUI_INPUT_BOTTOM_OFFSET` | 输入框底部偏移 | `10` |
| `OPENJAX_TUI_HISTORY_WINDOW_LINES` | 历史窗口最大行数 | `500` |
| `OPENJAX_TUI_DEBUG` | 调试日志 | 未设置 |
| `OPENJAX_TUI_LOG_DIR` | 日志目录 | `.openjax/logs` |
| `OPENJAX_TUI_LOG_MAX_BYTES` | 日志文件大小限制 | `2097152` (2MB) |
| `OPENJAX_TUI_ENABLE_KEYBOARD_ENHANCEMENT` | 键盘增强协议 | 未设置 |
| `OPENJAX_DAEMON_CMD` | 守护进程命令 | `cargo run -q -p openjaxd` |

---

## 6. 测试覆盖

26 个测试文件覆盖以下方面：

| 测试类别 | 测试文件 |
|----------|----------|
| 应用集成 | `test_app_event_wiring.py`, `test_smoke.py` |
| 审批流程 | `test_approval_flow.py`, `test_approval.py` |
| 渲染 | `test_assistant_render.py`, `test_stream_render.py`, `test_user_prompt_render.py` |
| 事件处理 | `test_event_handlers.py`, `test_event_state_manager*.py` |
| 输入系统 | `test_input_backend.py`, `test_input_loops_commands.py`, `test_input_normalize.py` |
| 视口适配 | `test_history_viewport_adapter.py`, `test_scrollback_live_mode.py` |
| 状态管理 | `test_state.py` |
| 动画 | `test_status_animation.py` |
| 工具统计 | `test_tool_summary.py` |
| 日志 | `test_logging.py` |
| 启动配置 | `test_startup_config.py`, `test_logo_select.py` |
| 其他 | `test_debug_utils.py`, `test_timeline_unicode_width.py`, `test_keyboard_enhancement.py` |

---

## 7. 架构特点总结

1. **模块化设计**: 每个关注点分离到独立模块，职责清晰
2. **双后端支持**: prompt_toolkit 富 TUI 优雅降级到基础 CLI
3. **事件驱动**: 异步事件循环处理用户输入和守护进程事件
4. **状态集中**: 类型化数据类 `AppState` 统一管理所有状态
5. **完整类型**: Python 3.10+ 联合类型语法，类型注解覆盖
6. **全面测试**: 单元测试 + 集成测试，核心流程全覆盖
7. **可配置性**: 丰富的环境变量控制，支持灵活部署
8. **回退机制**: 多层降级策略，确保稳定性
