# openjax_tui (Textual)

基于 `Textual` 的 OpenJax Python TUI（重构版），通过 `openjax_sdk` 连接 `openjaxd`，提供流式对话、命令面板、审批状态展示与日志能力。

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
│           └── command_palette.py # 内联命令候选组件
└── tests/
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
2. **流式响应渲染**：`assistant_delta` 增量更新，`assistant_message` 权威覆盖，`turn_completed` 收尾。
3. **命令面板**：输入 `/` 触发候选，支持模糊匹配和上下键切换。
4. **审批状态同步**：处理 `approval_requested` / `approval_resolved`，通过 `/pending` 查看待处理项。
5. **日志与异常可观测**：日志写入 `.openjax/logs/openjax_tui.log`，支持轮转与调试级别。

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

- 用户消息前缀：`❯`
- 助手消息前缀：`⏺`
- 命令面板命令：`/help`、`/clear`、`/exit`、`/pending`、`/approve`、`/deny`
- 审批交互：通过 `/pending` 查看，使用 `/approve` 或 `/deny` 回传审批结果

## 已知说明

- 本包依赖 `openjax_sdk`，若导入失败请执行 `make setup-new` 或设置正确的 `PYTHONPATH`。
