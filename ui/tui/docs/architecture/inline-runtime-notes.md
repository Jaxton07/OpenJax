# Inline Runtime Notes

## 1) 技术结构方案

`ui/tui` 采用「inline viewport + scrollback 插入」方案，而不是默认走 alternate screen。核心分层如下：

- `app/*`：协议事件到展示数据（history cell / live message / approval state）的转换层。
- `tui.rs`：渲染编排层，负责 viewport 尺寸策略、history 插入时机和逐帧绘制顺序。
- `terminal/*`：底层终端抽象，包含 buffer diff、ANSI draw、cursor/viewport 状态。
- `insert_history.rs`：单一历史插入路径，使用受控 scroll region 将新历史写入 viewport 上方。

渲染主流程：

1. 读取屏幕尺寸与当前 viewport。
2. 根据 `desired_height` 计算下一帧 viewport（保持当前位置，溢出才贴底）。
3. 先插入 `pending_history_lines` 到 scrollback。
4. 再绘制 live/input/approval/footer 到当前 viewport。
5. 恢复并同步 cursor 状态。

## 2) 关键细节与设计取舍

- 启动锚点策略：初始 viewport 锚在当前 shell 光标行，而不是强制底部。
- 连贯性策略：默认不清整屏、不 purge scrollback，保持 shell 历史可见。
- 安全清理策略：仅在 viewport 变化时做局部 `clear()`，避免脏帧叠加。
- 单通道历史插入：所有 committed history 都通过 `pending_history_lines -> insert_history_lines`，避免重复路径导致次序错乱。

## 3) 常见坑点与问题根因

### 坑 1：审批确认后出现孤立字符（如 `-`）

现象：
- Approval panel 关闭后，live 区出现不应保留的符号，输入区视觉异常。

根因：
- `pending_approval` 清掉了，但审批提示写入的 `live_messages` 未同步清理。

解决：
- 在审批 submit（approve/deny）和 `ApprovalResolved` 事件路径都清理 `live_messages`。

### 坑 2：重启后残留 `›` / `─`（首列幽灵字符）

现象：
- 某一行被清空后，首列旧字符仍残留。

根因：
- diff 算法在「整行变空白」时 `ClearToEnd` 从 `x=1` 开始，漏清 `x=0`。

解决：
- 空白行强制从 `x=0` 发 `ClearToEnd`。
- 增加回归测试覆盖该场景。

### 坑 3：shell prompt 箭头混入 TUI

现象：
- zsh/p10k 的 `❯` 提示符出现在 TUI 区域内。

根因：
- viewport 锚点/清理策略和历史插入顺序不稳定时，旧终端内容被误带入当前 viewport。

解决：
- 采用 cursor 锚点初始化。
- 仅在 viewport 变化时局部 clear，并确保 history 插入在统一路径执行。

### 坑 4：每次启动都贴底，顶部留大块空白

现象：
- 进入后输入框固定底部，缺少和 shell 的视觉连续性。

根因：
- viewport 每帧都按 `screen.height - desired_height` 计算，强制贴底。

解决：
- 改为「保留当前 y，溢出才贴底」策略。

## 4) 稳定性注意点

- 不要增加第二条 history 插入路径（例如在 reducer 中直接写终端）。
- 修改 `terminal/diff.rs` 时必须保留空白行 `x=0` 清理语义。
- 任何全屏清理动作（`ClearType::All` / scrollback purge）都要谨慎，只能用于显式 reset 场景。
- 变更 viewport 策略后，必须至少回归：
  - 审批打开/确认/继续输入
  - 连续重启会话
  - 中部启动后长输出贴底

## 5) 当前验证建议

- 自动化：`cargo test -p tui_next`
- 手工：
  1. shell 连续回车制造多个 prompt 后启动 TUI，确认无污染字符。
  2. 在中部行启动 TUI，确认不是立即贴底。
  3. 输出增多后确认平滑贴底且无残影。
  4. 审批流程全链路确认输入提示符与历史区域稳定。
