# Python TUI 回归清单（tmux / zellij）

本文档用于阶段 E 的手工回归，重点验证 Python TUI 在多终端复用器下的稳定性。

建议先执行：

```bash
zsh smoke_test/python_tui_mux_check.sh
```

## 1. 基础启动

在 tmux/zellij 内执行：

```bash
PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
python3 -m openjax_tui
```

检查项：
1. 能正常显示 `session started`
2. 输入 `/help` 有返回
3. 输入 `/exit` 可退出

## 2. 事件流回归

输入：

```text
tool:list_dir dir_path=.
```

检查项：
1. 出现 `submitted`
2. 出现 `turn_started`
3. 出现 `tool start`
4. 出现 `tool done`
5. 出现 `turn_completed`
6. 在输入期间即使有事件到达，也不出现行内乱码或输入残留

## 3. 审批回归

输入需要审批的命令（按当前策略）后，检查：
1. 出现 `approval requested`
2. 输入 `y` 或 `n` 能回传
3. 出现 `approval resolved`

## 4. 复用器行为

tmux：
1. 分离会话再恢复，确认 TUI 进程可继续交互
2. 切换 pane 后返回，确认输出与输入正常
3. 方向键编辑输入不应出现 `^[[A` 等转义残留

zellij：
1. 切换 tab/pane 后返回，确认输入焦点正常
2. 大量输出后滚动查看，确认不阻塞输入
3. 方向键编辑输入不应出现乱码或删除不完整

## 5. 回归结论模板

请填写：

- `docs/tui/python-tui-regression-report-template.md`
