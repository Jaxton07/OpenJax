# Inline Runtime Notes

## 1) 技术结构方案

`ui/tui` 采用「inline viewport + scrollback 插入」方案，而不是默认走 alternate screen。核心分层如下：

- `app/*`：协议事件到展示数据（history cell / live message / approval state）的转换层。
- `tui.rs`：渲染编排层，负责 viewport 尺寸策略、history 插入时机和逐帧绘制顺序。
- `terminal/*`：底层终端抽象，包含 buffer diff、ANSI draw、cursor/viewport 状态。
- `insert_history.rs`：单一历史插入路径，使用受控 scroll region 将新历史写入 viewport 上方。

渲染主流程：

1. 读取屏幕尺寸与当前 viewport。
2. 根据 `desired_height` 生成 `ViewportPlan`（目标 area + 需要上滚的行数）。
3. 若发生底部溢出，先执行受控 `scroll_region_up` 腾空间，再更新 viewport。
4. 先插入 `pending_history_lines` 到 scrollback。
5. 再绘制 live/input/approval/footer 到当前 viewport。
6. 恢复并同步 cursor 状态。

## 2) 关键细节与设计取舍

- 启动锚点策略：初始 viewport 使用零高度锚点 `Rect(0, cursor_y, 0, 0)`，首帧再扩张到目标高度。
- 连贯性策略：默认不清整屏、不 purge scrollback，保持 shell 历史可见。
- 安全清理策略：仅在 viewport 变化时做局部 `clear()`，避免脏帧叠加。
- 底部腾空间策略：当 viewport 扩张后 `bottom > screen.height` 时，先对上方区域执行 `scroll_region_up`，再贴底。
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
- 终端底部启动时，viewport 扩张没有先“滚动腾空间”，导致直接覆盖与残留混叠，shell prompt 字符（如 `❯`）会进入 TUI 可见区。

解决：
- 改为零高度锚点初始化，首帧扩张前先执行 `scroll_region_up`。
- 仅在 viewport 变化时局部 clear，并确保 history 插入在统一路径执行。

### 坑 4：每次启动都贴底，顶部留大块空白

现象：
- 进入后输入框固定底部，缺少和 shell 的视觉连续性。

根因：
- viewport 每帧都按 `screen.height - desired_height` 计算，强制贴底。

解决：
- 改为「保留当前 y，溢出才贴底」策略。

### 坑 5：终端底部启动时覆盖下方历史内容

现象：
- 当 shell 光标已在终端底部，启动 TUI 会覆盖/破坏下方历史显示。

根因：
- 初始化阶段直接占用固定高度 viewport，且 draw 只做 y 贴底，不做“先滚动再显示”。

解决：
- 初始化改为零高度锚点。
- draw 阶段使用 `ViewportPlan`；在 overflow 分支先 `scroll_region_up(0..area.top(), overflow)`，再更新 viewport。
- 为 overflow/non-overflow 分支补充单元测试。

## 4) 稳定性注意点

- 不要增加第二条 history 插入路径（例如在 reducer 中直接写终端）。
- 修改 `terminal/diff.rs` 时必须保留空白行 `x=0` 清理语义。
- 任何全屏清理动作（`ClearType::All` / scrollback purge）都要谨慎，只能用于显式 reset 场景。
- `Terminal::scroll_region_up` 只用于 viewport 扩张溢出场景，避免与 history 插入路径职责重叠。
- 变更 viewport 策略后，必须至少回归：
  - 审批打开/确认/继续输入
  - 连续重启会话
  - 底部无多余空间启动（重点）
  - 中部启动后长输出贴底

## 5) 当前验证建议

- 自动化：`cargo test -p tui_next`
- 手工：
  1. shell 连续回车制造多个 prompt 后启动 TUI，确认无污染字符。
  2. shell 光标在终端底部时启动，确认表现为“先滚动腾空间，再呈现 TUI”，且历史连续。
  3. 在中部行启动 TUI，确认不是立即贴底。
  4. 输出增多后确认平滑贴底且无残影。
  5. 审批流程全链路确认输入提示符与历史区域稳定。
