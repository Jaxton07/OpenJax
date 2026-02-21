# openjax_tui

Python TUI MVP based on `openjax_sdk`.

默认输出为精简会话视图：
1. `❯` 用户输入
2. `⏺` 助手流式或最终输出
3. `tool>` 工具调用摘要
4. `approval>` 审批提示

输入区域默认在 TTY 环境下使用 `prompt_toolkit`，可将输入提示固定在底部，同时让事件输出在上方滚动。
若需回退基础输入模式，可设置：

```bash
OPENJAX_TUI_INPUT_BACKEND=basic
```

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

## 交互

1. 直接输入文本：提交一个 turn
2. `/approve <approval_request_id> y|n`：回传指定审批
3. `y` 或 `n`：快速回传最新待处理审批
4. `/pending`：查看待处理审批
5. `/help`：显示命令帮助
6. `/exit`：退出

## 测试

```bash
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m unittest discover -s python/openjax_tui/tests -v
```
