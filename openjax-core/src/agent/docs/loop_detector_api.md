# LoopDetector API 参考

## LoopSignal

```rust
#[derive(Debug, Copy, PartialEq, Eq)]
pub enum LoopSignal {
    None,    // 正常，无重复
    Warned,  // 触发软中断，已注入恢复 prompt
    Halt,    // 警告后再次触发，必须硬终止
}
```

## LoopDetector

### `LoopDetector::new() -> Self`

创建新的 LoopDetector 实例，配置：
- `window_capacity`: 16
- `warn_threshold`: 5

### `fn check_and_advance(&mut self, tool_name: &str, args_hash: &str) -> LoopSignal`

每次工具调用后调用。检测是否触发循环信号。

### `fn recovery_prompt(&self) -> Option<&'static str>`

当状态为 `Warned` 时返回恢复 prompt 内容，否则返回 `None`。

### `fn reset(&mut self)`

重置所有状态和滑动窗口。每回合开始时调用。

### `fn warn_threshold(&self) -> usize`

返回警告阈值（固定为 5）。

## 配置

上限通过以下途径配置（优先级从高到低）：
1. 环境变量 `OPENJAX_MAX_TOOL_CALLS_PER_TURN` / `OPENJAX_MAX_PLANNER_ROUNDS_PER_TURN`
2. 配置文件 `config.agent.max_tool_calls_per_turn` / `config.agent.max_planner_rounds_per_turn`
3. 默认值 `MAX_TURN_BUDGET = 300`