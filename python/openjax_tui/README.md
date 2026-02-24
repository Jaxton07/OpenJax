# openjax_tui

基于 `openjax_sdk` 的 Python TUI MVP，为 OpenJax 提供交互式命令行界面。

## 项目结构

```
python/openjax_tui/
├── README.md                    # 项目文档
├── pyproject.toml              # Python 包配置
├── pyrightconfig.json          # 类型检查配置
├── src/
│   └── openjax_tui/            # 主包源代码
│       ├── __init__.py         # 包入口（导出 run）
│       ├── __main__.py         # CLI 入口（asyncio main）
│       ├── app.py                 # 主应用编排（事件循环、生命周期管理）
│       ├── approval.py            # 审批工作流管理
│       ├── assistant_render.py    # 助手消息渲染
│       ├── debug_utils.py         # 调试日志与历史文本规范化工具
│       ├── event_dispatch.py      # 事件处理和路由
│       ├── event_handlers.py      # 事件处理适配器（闭包工厂）
│       ├── event_state_manager.py # 事件状态更新管理器
│       ├── input_backend.py       # 输入处理（prompt_toolkit vs basic）
│       ├── input_loops.py         # 输入循环和命令处理
│       ├── prompt_runtime_loop.py # prompt_toolkit 运行时组装与 fallback
│       ├── prompt_ui.py           # 提示 UI 运行时和键盘快捷键
│       ├── session_logging.py     # 会话事件日志
│       ├── slash_commands.py      # 斜杠命令处理（/help, /exit 等）
│       ├── startup_ui.py          # 启动 Logo 和显示工具
│       ├── state.py               # 应用状态管理（审批、流式、工具统计）
│       ├── status_animation.py    # 状态动画系统（thinking/tool_wait）
│       ├── tool_runtime.py        # 工具执行跟踪
│       ├── tui_logging.py         # 日志基础设施
│       └── viewport_adapter.py    # 视口适配器（Pilot/TextArea 双实现）
└── tests/                         # 测试套件（26 个测试文件）
    ├── test_app_event_wiring.py
    ├── test_approval_flow.py
    ├── test_approval.py
    ├── test_assistant_render.py
    ├── test_debug_utils.py
    ├── test_event_handlers.py
    ├── test_event_state_manager.py
    ├── test_event_state_manager_integration.py
    ├── test_history_viewport_adapter.py
    ├── test_input_backend.py
    ├── test_input_loops_commands.py
    ├── test_input_normalize.py
    ├── test_logo_select.py
    ├── test_logging.py
    ├── test_prompt_keybindings.py
    ├── test_prompt_runtime_loop.py
    ├── test_prompt_ui.py
    ├── test_scrollback_live_mode.py
    ├── test_smoke.py
    ├── test_startup_config.py
    ├── test_state.py
    ├── test_status_animation.py
    ├── test_stream_render.py
    ├── test_timeline_unicode_width.py
    ├── test_tool_summary.py
    └── test_user_prompt_render.py
```

## 各模块功能介绍

### 核心模块

| 模块 | 功能描述 |
|------|----------|
| `app.py` | 主应用编排与兼容层，负责生命周期管理，并将输入循环、事件状态更新、prompt runtime 委托到子模块 |
| `state.py` | 集中式状态管理，包含 `AppState` 类跟踪审批队列、流式状态、工具统计、UI 历史等 |
| `event_dispatch.py` | 事件路由系统，处理 `assistant_delta`、`tool_call_started`、`approval_requested` 等事件 |
| `event_handlers.py` | 事件处理适配器工厂，创建渲染/工具运行统计相关闭包，降低 `app.py` 组装复杂度 |
| `event_state_manager.py` | 事件状态管理器，统一处理 turn phase、审批状态和 live viewport ownership 更新 |
| `approval.py` | 审批工作流管理，支持多审批队列、焦点导航、特定 ID 或最新审批解析 |
| `input_backend.py` | 双后端输入系统，TTY 环境下使用 `prompt_toolkit`，非 TTY 回退到基础 `input()` |
| `input_loops.py` | 输入循环实现，包含 basic 输入循环和命令行处理逻辑 |
| `debug_utils.py` | 调试辅助工具，提供事件日志格式化、调试预览截断与 prompt 历史 ANSI 清理 |

### 渲染与 UI 模块

| 模块 | 功能描述 |
|------|----------|
| `assistant_render.py` | 助手消息渲染，处理流式内容更新、最终消息显示、文本对齐、工具标签 |
| `tool_runtime.py` | 工具执行监控，跟踪开始/完成时间、计算持续时间、彩色结果显示 |
| `viewport_adapter.py` | 视口适配器层次结构，支持 Pilot（scrollback-first）和 TextArea（兼容）双实现 |
| `status_animation.py` | 状态动画系统，提供 thinking 和 tool_wait 状态的动态指示器 |
| `startup_ui.py` | 启动界面，包含响应式 ASCII Logo、版本信息、会话 ID 显示 |
| `prompt_ui.py` | 提示 UI 运行时管理和键盘快捷键配置 |
| `prompt_runtime_loop.py` | prompt_toolkit 运行时组装模块，负责历史窗口维护、scrollback flush、fallback 到 basic |

### 命令与日志模块

| 模块 | 功能描述 |
|------|----------|
| `slash_commands.py` | 斜杠命令解析和自动补全，支持 `/approve`、`/pending`、`/help`、`/exit` |
| `tui_logging.py` | 调试日志基础设施，支持日志轮转（默认 2MB，保留 5 个备份）|
| `session_logging.py` | 结构化会话事件日志，记录启动摘要和审批决策审计 |

## 运行

在仓库根目录执行：

```bash
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

可选：指定 daemon 命令（默认 `cargo run -q -p openjaxd`）

```bash
OPENJAX_DAEMON_CMD="target/debug/openjaxd" \
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

## 会话视图

默认输出为精简会话视图：

1. `❯` 用户输入
2. `⏺` 助手流式或最终输出
3. 状态栏展示工具进行中动画（如 `status: Reading...`）
4. 工具完成后仅保留一行历史（如 `⏺ Read 1 file (test.txt) · 1ms`，成功/失败用红绿颜色区分）
5. `approval>` 审批提示

输入区域默认在 TTY 环境下使用 `prompt_toolkit`，可将输入提示固定在底部，同时让事件输出在上方滚动。

滚动历史区域默认采用 scrollback-first 路线：历史消息优先写入终端 scrollback，视口中只保留当前活动 turn，降低长会话下的截断风险。

`OPENJAX_TUI_VIEW_MODE` 控制历史渲染模式：

- `live` / `live_viewport`（默认）：滚动优先模式，实时更新活动 turn 并尽快下沉已完成内容到终端 scrollback。
- `session`：稳定兼容模式，使用会话块视图，适合作为保底回退配置。

若需回退基础输入模式，可设置：

```bash
OPENJAX_TUI_INPUT_BACKEND=basic
```

`OPENJAX_TUI_HISTORY_VIEWPORT_IMPL` 控制 prompt_toolkit 历史视口适配器：

- `pilot`（默认）：当前推荐实现，配合 `OPENJAX_TUI_VIEW_MODE=live` 提供 scrollback-first 行为。
- `textarea`：兼容回退实现，保留旧 TextArea 视口路径，便于现场快速规避新适配器相关问题。

## 交互命令

| 命令 | 说明 |
|------|------|
| 直接输入文本 | 提交一个 turn |
| `/approve <id> y\|n` | 回传指定审批 |
| `y` 或 `n` | 快速回传最新待处理审批 |
| `/pending` | 查看待处理审批队列 |
| `/help` | 显示命令帮助 |
| `/exit` | 退出应用 |

## 环境变量配置

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `OPENJAX_TUI_INPUT_BACKEND` | 输入后端：`prompt_toolkit` 或 `basic` | 自动检测 TTY |
| `OPENJAX_TUI_VIEW_MODE` | 会话视图模式：`live`/`live_viewport`（scrollback-first 默认）或 `session`（兼容回退） | `live` |
| `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL` | prompt_toolkit 历史视口实现：`pilot`（推荐）或 `textarea`（回退） | `pilot` |
| `OPENJAX_TUI_DEBUG` | 启用调试日志（`1` 开启）| 未设置 |
| `OPENJAX_TUI_LOG_DIR` | 日志目录 | `.openjax/logs` |
| `OPENJAX_TUI_LOG_MAX_BYTES` | 单个日志文件大小限制 | `2097152` (2MB) |
| `OPENJAX_DAEMON_CMD` | 守护进程启动命令 | `cargo run -q -p openjaxd` |

## Rollout 与回退策略

当前运行策略建议按以下顺序排障：

1. 默认使用 `OPENJAX_TUI_VIEW_MODE=live`（或兼容别名 `live_viewport`）+ `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=pilot`。
2. 若发现 live 视口异常，先回退 `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=textarea`。
3. 若仍异常，再整体回退到 `OPENJAX_TUI_VIEW_MODE=session`，必要时同时 `OPENJAX_TUI_INPUT_BACKEND=basic`。

推荐试点启动命令：

```bash
OPENJAX_TUI_VIEW_MODE=live \
OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=pilot \
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

推荐保底回退命令：

```bash
OPENJAX_TUI_VIEW_MODE=session \
OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=textarea \
OPENJAX_TUI_INPUT_BACKEND=basic \
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

操作员排障要点：

- 先区分输入后端：TTY 默认 `prompt_toolkit`，非 TTY 或显式 `OPENJAX_TUI_INPUT_BACKEND=basic` 使用 basic 后端。
- 若 `prompt_toolkit` 路径出现视口相关问题，优先切换 `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=textarea` 复测。
- 若仍异常，切换 `OPENJAX_TUI_VIEW_MODE=session` 并开启 `OPENJAX_TUI_DEBUG=1` 收集 `.openjax/logs/openjax_tui.log`。

## 调试启动

```bash
OPENJAX_TUI_DEBUG=1 \
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

## 测试

运行全部测试：

```bash
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m unittest discover -s python/openjax_tui/tests -v
```

运行单个测试文件：

```bash
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m unittest python/openjax_tui/tests/test_input_backend.py -v
```

运行单个测试方法：

```bash
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m unittest openjax_tui.tests.test_input_backend.InputBackendTest.test_force_basic_by_env -v
```

## 架构特点

- **模块化设计**：每个关注点分离到独立模块
- **双后端支持**：从富 TUI 优雅降级到基础 CLI
- **事件驱动**：异步事件循环处理用户输入和守护进程事件
- **状态管理**：使用类型化数据类的集中式状态
- **完整类型注解**：Python 3.10+ 联合类型语法
- **全面测试**：所有模块的单元和集成测试
