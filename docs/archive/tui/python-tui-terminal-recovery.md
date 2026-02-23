# Python TUI 终端恢复说明

## 1. 设计现状

当前 Python TUI 不启用 raw mode，也不切换 alternate screen。  
因此异常退出后不会像传统全屏 TUI 那样遗留终端状态污染。

## 2. 异常路径处理

1. 输入循环捕获 `EOFError` / `KeyboardInterrupt` 并退出。
2. 退出流程始终执行：
   - `shutdown_session`（若会话存在）
   - 关闭 daemon 子进程
3. 事件流断开会输出错误并退出主循环。

## 3. 验证步骤

1. 正常退出验证：
   - 启动 TUI
   - 输入 `/exit`
   - 确认 shell 提示符恢复正常
2. Ctrl-C 验证：
   - 启动 TUI
   - 按 `Ctrl-C`
   - 确认 shell 提示符恢复正常
3. daemon 中断验证：
   - 启动 TUI
   - 手工结束 `openjaxd`
   - 确认 TUI 提示 event stream closed 并退出
