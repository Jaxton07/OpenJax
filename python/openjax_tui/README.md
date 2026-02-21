# openjax_tui

Python TUI MVP based on `openjax_sdk`.

默认输出为精简会话视图：
1. `you>` 用户输入
2. `assistant>` 助手流式或最终输出
3. `tool>` 工具调用摘要
4. `approval>` 审批提示

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
