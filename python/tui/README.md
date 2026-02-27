# openjax_tui (Textual)

基于 `Textual` 的 OpenJax Python TUI（重构版），通过 `openjax_sdk` 连接 `openjaxd`，当前已支持流式对话、斜杠命令候选、审批弹窗、Markdown 助手消息渲染、简洁工具结果显示与日志可观测。

## 项目结构

```
python/tui/
├── README.md
├── pyproject.toml
├── src/
│   └── openjax_tui/
│       ├── __init__.py
│       ├── __main__.py
│       ├── app.py                 # 主应用与事件编排
│       ├── sdk_runtime.py         # SDK 生命周期与事件循环
│       ├── event_mapper.py        # daemon 事件 -> UI/状态操作映射
│       ├── state.py               # 应用状态模型
│       ├── logging_setup.py       # TUI 日志初始化与异常 hook
│       ├── styles.tcss            # Textual 样式
│       ├── commands/
│       │   └── __init__.py        # 命令面板命令
│       ├── screens/
│       │   └── chat.py            # 主聊天界面
│       └── widgets/
│           ├── command_palette.py # 内联命令候选组件
│           ├── approval_popup.py  # 内联审批弹窗组件
│           └── markdown_message.py # 助手 Markdown 渲染包装
└── tests/
    ├── test_approval_popup.py
    ├── test_app.py
    ├── test_command_palette.py
    ├── test_event_mapper.py
    ├── test_init.py
    ├── test_logging_setup.py
    ├── test_sdk_runtime.py
    └── test_state.py
```

## 核心能力

1. **SDK 全链路接入**：`start_session` / `submit_turn` / `stream_events` / `shutdown_session`。
2. **流式响应渲染**：`assistant_delta` 增量更新；`assistant_message`/`turn_completed` 最终收敛。
3. **斜杠命令候选**：仅在输入以 `/` 开头时触发候选层，支持模糊匹配和上下键切换。
4. **审批弹窗交互**：收到 `approval_requested` 自动弹窗并接管焦点，支持 `approve/deny/cancel`。
5. **助手 Markdown 渲染**：最终助手消息按 Markdown 渲染，代码块带高亮主题。
6. **工具结果简洁显示**：`⏺ Label (target)` 单行样式，成功绿色、失败红色。
7. **日志与异常可观测**：日志写入 `.openjax/logs/openjax_tui.log`，支持轮转与调试级别。

## 运行方式

推荐从仓库根目录执行。

### 使用 Makefile（推荐）

```bash
make setup-new
make dev-new
```

### 直接运行

```bash
PYTHONPATH=python/openjax_sdk/src:python/tui/src \
.venv/bin/python -m openjax_tui
```

## 环境变量

- `OPENJAX_DAEMON_CMD`：自定义 daemon 启动命令。
- `OPENJAX_TUI_LOG_DIR`：日志目录（默认 `.openjax/logs`）。
- `OPENJAX_TUI_LOG_MAX_BYTES`：单文件最大字节数（默认 2 MiB）。
- `OPENJAX_TUI_DEBUG=1`：启用 debug 日志。
- `OPENJAX_TUI_ENABLE_MOUSE=1`：启用 Textual 鼠标上报（默认关闭，便于终端直接选中文本复制）。

## 开发与测试

运行测试：

```bash
make test-new
```

或：

```bash
PYTHONPATH=python/openjax_sdk/src:python/tui/src \
.venv/bin/python -m pytest python/tui/tests -v
```

## 交互说明（当前）

- 启动后默认焦点在输入框，可直接输入消息。
- 用户消息前缀：`❯`
- 工具消息前缀：`⏺`（成功绿色 / 失败红色）
- UI 背景默认透明，继承终端主题背景（含终端透明度/模糊效果）。
- 命令候选命令：`/help`、`/clear`、`/exit`、`/pending`、`/approve`、`/deny`
- 候选层仅 `/` 输入态可创建；非 `/` 输入态禁止打开候选层。
- 审批交互：收到审批事件后自动在输入框上方弹出审批面板并抢焦点，输入框暂停输入；支持 `Up/Down + Enter` 选择 `approve/deny/cancel`，`Esc` 等价 `cancel`。
- UI 默认不显示 Textual 内置 command palette 入口，也不显示 Footer 快捷键条。

## 已知说明

- 本包依赖 `openjax_sdk`，若导入失败请执行 `make setup-new` 或设置正确的 `PYTHONPATH`。
