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
│       ├── app.py              # 主应用编排（事件循环、生命周期管理）
│       ├── state.py            # 应用状态管理（审批、流式、工具统计）
│       ├── event_dispatch.py   # 事件处理和路由
│       ├── approval.py         # 审批工作流管理
│       ├── input_backend.py    # 输入处理（prompt_toolkit vs basic）
│       ├── tool_runtime.py     # 工具执行跟踪
│       ├── assistant_render.py # 助手消息渲染
│       ├── slash_commands.py   # 斜杠命令处理（/help, /exit 等）
│       ├── startup_ui.py       # 启动 Logo 和显示工具
│       ├── render_utils.py     # 文本渲染工具
│       ├── prompt_runtime.py   # 提示 UI 运行时管理
│       ├── prompt_keybindings.py # 键盘快捷键配置
│       ├── tui_logging.py      # 日志基础设施
│       └── session_logging.py  # 会话事件日志
└── tests/                      # 测试套件（16 个测试文件）
    ├── test_approval_flow.py
    ├── test_approval_module.py
    ├── test_assistant_render_module.py
    ├── test_input_backend.py
    ├── test_input_normalize.py
    ├── test_logo_select.py
    ├── test_logging.py
    ├── test_prompt_keybindings_module.py
    ├── test_prompt_runtime_module.py
    ├── test_smoke.py
    ├── test_startup_config.py
    ├── test_state_module.py
    ├── test_stream_render.py
    ├── test_tool_summary.py
    └── test_user_prompt_render.py
```

## 各模块功能介绍

### 核心模块

| 模块 | 功能描述 |
|------|----------|
| `app.py` | 主应用编排，初始化异步事件循环、管理 OpenJaxAsyncClient 连接、处理用户输入和守护进程事件的主循环 |
| `state.py` | 集中式状态管理，包含 `AppState` 类跟踪审批队列、流式状态、工具统计、UI 历史等 |
| `event_dispatch.py` | 事件路由系统，处理 `assistant_delta`、`tool_call_started`、`approval_requested` 等事件 |
| `approval.py` | 审批工作流管理，支持多审批队列、焦点导航、特定 ID 或最新审批解析 |
| `input_backend.py` | 双后端输入系统，TTY 环境下使用 `prompt_toolkit`，非 TTY 回退到基础 `input()` |

### 渲染与 UI 模块

| 模块 | 功能描述 |
|------|----------|
| `assistant_render.py` | 助手消息渲染，处理流式内容更新和最终消息显示 |
| `tool_runtime.py` | 工具执行监控，跟踪开始/完成时间、计算持续时间、彩色结果显示 |
| `startup_ui.py` | 启动界面，包含响应式 ASCII Logo、版本信息、会话 ID 显示 |
| `render_utils.py` | 文本处理工具，多行对齐、工具结果标签映射 |
| `prompt_runtime.py` | 提示 UI 生命周期管理，UI 刷新、历史视图更新 |
| `prompt_keybindings.py` | `prompt_toolkit` 键盘快捷键配置 |

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
3. `⏺ tools: ...` 工具调用摘要（成功/失败颜色提示）
4. `approval>` 审批提示

输入区域默认在 TTY 环境下使用 `prompt_toolkit`，可将输入提示固定在底部，同时让事件输出在上方滚动。

若需回退基础输入模式，可设置：

```bash
OPENJAX_TUI_INPUT_BACKEND=basic
```

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
| `OPENJAX_TUI_DEBUG` | 启用调试日志（`1` 开启）| 未设置 |
| `OPENJAX_TUI_LOG_DIR` | 日志目录 | `.openjax/logs` |
| `OPENJAX_TUI_LOG_MAX_BYTES` | 单个日志文件大小限制 | `2097152` (2MB) |
| `OPENJAX_DAEMON_CMD` | 守护进程启动命令 | `cargo run -q -p openjaxd` |

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
