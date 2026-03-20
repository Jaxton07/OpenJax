# LoopDetector 循环检测机制设计

**日期：** 2026-03-20
**状态：** 设计中

## 1. 背景与目标

当前单回合工具调用上限为 10 次，planner 轮次上限为 20 次，均为硬截断。这对长线任务支持不足，正常任务可能被上限卡住；但完全去掉上限又有模型陷入重复调用的风险。

**设计目标：**
- 将上限提升至 300 次作为绝对兜底，确保长线任务不被轻易截断
- 引入 LoopDetector 组件，通过检测连续重复调用模式主动干预，而非依赖简单计数
- 在真正出现循环时给模型一次自我纠正的机会，避免粗暴的硬截断

## 2. 决策总结

| 问题 | 决策 |
|------|------|
| 上限值 | 300 次（工具调用与 planner 轮次共用同一常量） |
| 检测粒度 | 相同工具 + 相同参数连续 N 次 |
| 触发阈值 N | 5 次 |
| 干预方式 | 注入手动恢复 prompt，让模型自我纠正 |
| 恢复后再触发 | 硬终止（6 次为上限） |
| 组件位置 | 新建 `openjax-core/src/agent/loop_detector.rs` |
| 状态托管 | 通过 `Agent` 持有 `LoopDetector` 实例 |

## 3. 上限常量统一

### 3.1 常量定义

**文件：** `openjax-core/src/agent/runtime_policy.rs`

```rust
/// 单回合最大工具调用次数，也是单回合最大规划轮次。
/// 同时作为 LoopDetector 滑动窗口的基础容量。
pub const MAX_TURN_BUDGET: usize = 300;
```

- `DEFAULT_MAX_TOOL_CALLS_PER_TURN` 和 `DEFAULT_MAX_PLANNER_ROUNDS_PER_TURN` 均复用 `MAX_TURN_BUDGET`
- 两个 resolve 函数通过同一个常量读取，保证值始终相等

### 3.2 滑动窗口扩容

**文件：** `openjax-core/src/agent/state.rs`

`recent_tool_calls` 的滑动窗口大小从 10 调整为 16（大于触发阈值 5，确保窗口够用）。

```rust
if self.recent_tool_calls.len() > 16 {  // 原为 10
    self.recent_tool_calls.remove(0);
}
```

## 4. LoopDetector 组件

### 4.1 模块职责

**新建文件：** `openjax-core/src/agent/loop_detector.rs`

- 维护一个滑动窗口（VecDeque），记录最近 16 次工具调用的 `(tool_name, args_hash)`
- 每次工具调用后检查是否触发连续相同调用阈值（5次）
- 若触发，将状态置为 `LoopSignal::Warned`，记录已进入警告状态
- 若在警告状态下再次触发同一调用，立即返回 `LoopSignal::Halt`
- 对外暴露 `check_and_advance(tool_name, args_hash) -> LoopSignal`

### 4.2 核心类型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopSignal {
    /// 正常，无重复
    None,
    /// 触发软中断，已注入恢复 prompt
    Warned,
    /// 警告后再次触发，必须硬终止
    Halt,
}

pub struct LoopDetector {
    window: VecDeque<(String, String)>,        // (tool_name, args_hash)
    state: LoopSignal,
    warned_tool: Option<(String, String)>,     // 触发警告时的调用
    window_capacity: usize,
    warn_threshold: usize,                      // 默认 5
}
```

### 4.3 检测算法

```rust
impl LoopDetector {
    pub fn new() -> Self {
        Self {
            window: VecDeque::with_capacity(16),
            state: LoopSignal::None,
            warned_tool: None,
            window_capacity: 16,
            warn_threshold: 5,
        }
    }

    pub fn check_and_advance(&mut self, tool_name: &str, args_hash: &str) -> LoopSignal {
        let key = (tool_name.to_string(), args_hash.to_string());

        // 如果处于警告状态，检查是否继续重复同一调用
        if self.state == LoopSignal::Warned {
            if Some(&key) == self.warned_tool.as_ref() {
                self.state = LoopSignal::Halt;
                return LoopSignal::Halt;
            } else {
                // 警告后换了其他调用，重置状态
                self.state = LoopSignal::None;
                self.warned_tool = None;
            }
        }

        // 滑动窗口更新
        self.window.push_back(key.clone());
        if self.window.len() > self.window_capacity {
            self.window.pop_front();
        }

        // 检测连续相同调用次数
        let consecutive_count = self.window.iter().rev()
            .take_while(|k| *k == &key)
            .count();

        if consecutive_count >= self.warn_threshold {
            self.state = LoopSignal::Warned;
            self.warned_tool = Some(key);
            return LoopSignal::Warned;
        }

        LoopSignal::None
    }

    pub fn recovery_prompt(&self) -> Option<&'static str> {
        if self.state == LoopSignal::Warned {
            Some("[系统警告] 检测到你最近连续多次以完全相同的参数调用了同一工具，这可能是陷入了循环。请评估当前执行策略是否有效，并明确下一步将如何调整（可更换工具、改变参数、或给出阶段性结论）。")
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.window.clear();
        self.state = LoopSignal::None;
        self.warned_tool = None;
    }

    pub fn current_state(&self) -> LoopSignal {
        self.state
    }

    /// 返回当前配置的警告阈值（供事件构造使用）
    pub fn warn_threshold(&self) -> usize {
        self.warn_threshold
    }
}
```

### 4.4 Halt 时机说明

**关键设计决策：工具先执行，Halt 信号阻止下一次调用。**

- 检测发生在**工具执行后**（`check_and_advance` 在 `execution` 流程内部调用）
- 触发 Halt 的那一次工具调用**已经执行完毕**，其结果正常返回给模型
- Halt 信号的作用是**阻止模型在下一轮继续调用同一工具**，而非中止当前调用本身
- 这确保了循环检测不会导致工具调用被中途打断，避免出现半完成状态

### 4.5 状态机转换

```
[None] --5次相同调用--> [Warned]
[Warned] --再次相同调用--> [Halt]
[Warned] --调用不同工具--> [None]
[None/Halt] --reset()--> [None]
```

## 5. Agent 集成

### 5.1 字段添加

**文件：** `openjax-core/src/agent/state.rs`

`Agent` 结构体增加字段：

```rust
loop_detector: LoopDetector,
```

在 `bootstrap.rs` 中初始化：

```rust
loop_detector: LoopDetector::new(),
```

### 5.2 Planner 流程集成

**文件：** `openjax-core/src/agent/planner.rs`

在 `execute_natural_language_turn` 循环中：

```rust
while executed_count < self.max_tool_calls_per_turn
    && planner_rounds < self.max_planner_rounds_per_turn
{
    // ... skill 选取 ...

    // 在 build_planner_input 之前检查 loop detector
    let loop_recovery = self.loop_detector.recovery_prompt();
    let planner_input = build_planner_input(
        user_input,
        &self.history,
        &tool_traces,
        remaining,
        &skills_context,
        loop_recovery,  // 新增参数
    );

    // ... LLM 调用和工具执行 ...

    // 工具执行后检查信号
    // 注意：turn_id 来自 execute_natural_language_turn 函数签名，已在作用域内
    let signal = self.loop_detector.check_and_advance(tool_name, &args_hash);
    match signal {
        LoopSignal::Warned => {
            info!(turn_id, tool_name, "loop_detected: soft interrupt");
            self.push_event(events, Event::LoopWarning {
                turn_id,
                tool_name: tool_name.to_string(),
                consecutive_count: self.loop_detector.warn_threshold(),
            });
        }
        LoopSignal::Halt => {
            warn!(turn_id, tool_name, "loop_detected: hard halt after recovery failure");
            self.push_event(events, Event::ResponseError {
                turn_id,
                code: "loop_halt".to_string(),
                message: "检测到持续重复调用，已强制终止本回合。".to_string(),
                retryable: true,
            });
            turn_engine.on_failed();
            return;
        }
        LoopSignal::None => {}
    }
}
```

每回合开始时重置：

```rust
self.loop_detector.reset();
```

## 6. Prompt 注入接口

### 6.1 build_planner_input 扩展

**文件：** `openjax-core/src/agent/prompt.rs`

```rust
pub fn build_planner_input(
    user_input: &str,
    history: &[HistoryItem],
    tool_traces: &[String],
    remaining: usize,
    skills_context: &str,
    loop_recovery: Option<&str>,  // 新增
) -> String {
    let mut prompt = /* ... 现有逻辑 ... */;

    // 如果有 loop recovery prompt，追加到末尾
    if let Some(recovery) = loop_recovery {
        prompt.push_str("\n\n");
        prompt.push_str(recovery);
    }

    prompt
}
```

## 7. 事件扩展

### 7.1 新增 LoopWarning 事件

**文件：** `openjax-protocol/src/event.rs`

在 `Event` 枚举中添加：

```rust
LoopWarning {
    turn_id: u64,
    tool_name: String,
    consecutive_count: usize,
},
```

在 `planner.rs` 发出 `LoopSignal::Warned` 时推送此事件，供前端/日志感知。

## 8. 文档

**新建目录：** `openjax-core/src/agent/docs/`

- `loop_detector_design.md` — 本文档（设计说明）
- `loop_detector_api.md` — API 参考和使用说明

在 `openjax-core/src/agent/README.md` 的"扩展建议"章节添加链接：

```markdown
## 扩展建议

- [LoopDetector 循环检测机制](./docs/loop_detector_design.md)
- 增加新回合阶段时，优先在 `planner.rs` 维护状态机
```

## 9. 测试计划

### 9.1 单元测试（`loop_detector.rs`）

| 场景 | 输入 | 预期 |
|------|------|------|
| 正常调用 | 5 次不同工具 | 均为 `None` |
| 触发警告 | 5 次相同调用 | 第 5 次返回 `Warned` |
| 警告后换工具 | 警告状态下调用其他工具 | 返回 `None`，状态重置 |
| 硬终止 | 警告后再次相同调用 | 返回 `Halt` |
| reset | 任意状态下调用 reset | 状态回到 `None` |
| 边界：刚好 5 次 | 4 次相同 + 1 次其他 + 5 次相同 | 第 5 次相同返回 `Warned` |

### 9.2 集成测试

新增 `tests/m8_loop_detector.rs`，验证：
- 触发软中断后 recovery prompt 被注入
- 硬终止后回合正确结束
- 上限 300 次不受影响

## 10. 涉及文件清单

| 文件 | 操作 |
|------|------|
| `openjax-core/src/agent/runtime_policy.rs` | 修改：常量统一为 MAX_TURN_BUDGET = 300 |
| `openjax-core/src/agent/state.rs` | 修改：滑动窗口扩容至 16，新增 loop_detector 字段 |
| `openjax-core/src/agent/loop_detector.rs` | 新建：LoopDetector 组件 |
| `openjax-core/src/agent/planner.rs` | 修改：集成 LoopDetector 调用和信号处理 |
| `openjax-core/src/agent/prompt.rs` | 修改：build_planner_input 支持 recovery_prompt 注入 |
| `openjax-protocol/src/event.rs` | 修改：新增 LoopWarning 事件 |
| `openjax-core/src/agent/docs/loop_detector_design.md` | 新建：本设计文档 |
| `openjax-core/src/agent/docs/loop_detector_api.md` | 新建：API 参考文档 |
| `openjax-core/src/agent/README.md` | 修改：添加文档链接 |
