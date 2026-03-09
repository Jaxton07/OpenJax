# openjax-protocol

OpenJax 协议类型定义 crate，提供 Agent 操作（`Op`）、运行事件（`Event`）与多 Agent 相关基础类型。

## 项目结构

```
openjax-protocol/
├── README.md                  # 项目文档
├── Cargo.toml                 # crate 配置
└── src/
    └── lib.rs                 # 协议类型定义（ThreadId / Op / Event / AgentStatus）
```

## 各模块功能介绍

### 核心模块

| 模块 | 功能描述 |
|------|----------|
| `ThreadId` | 线程/agent 唯一标识，使用原子计数器生成 |
| `AgentSource` | agent 来源信息（`Root` 或 `SubAgent`） |
| `Op` | 上行操作类型（`UserTurn`、`SpawnAgent`、`SendToAgent`、`InterruptAgent`、`ResumeAgent`、`Shutdown`） |
| `Event` | 下行事件类型（turn 生命周期、工具调用、assistant 输出、审批事件、agent 状态） |
| `AgentStatus` | agent 运行状态枚举（`Running`、`Completed`、`Errored` 等） |

## 协议类型说明

### 操作（`Op`）

| 操作 | 说明 |
|------|------|
| `UserTurn { input }` | 提交用户输入并启动一次 turn |
| `SpawnAgent { ... }` | 预留：创建子 agent |
| `SendToAgent { ... }` | 预留：向现有 agent 发送输入 |
| `InterruptAgent { ... }` | 预留：中断运行中 agent |
| `ResumeAgent { ... }` | 预留：从持久化状态恢复 agent |
| `Shutdown` | 关闭当前 agent/会话 |

### 事件（`Event`）

| 事件 | 说明 |
|------|------|
| `TurnStarted` / `TurnCompleted` | turn 生命周期边界 |
| `ToolCallStarted` / `ToolCallCompleted` | 工具调用开始/完成及执行结果 |
| `AssistantDelta` / `AssistantMessage` | 助手流式增量与完整消息 |
| `ApprovalRequested` / `ApprovalResolved` | 审批请求与决策结果 |
| `AgentSpawned` / `AgentStatusChanged` | 多 agent 预留事件 |
| `ShutdownComplete` | 关闭完成事件 |

## 使用示例

```rust
use openjax_protocol::{Event, Op};

let op = Op::UserTurn {
    input: "tool:list_dir dir_path=.".to_string(),
};

let evt = Event::TurnStarted { turn_id: 1 };
```

### `tool_call_id` 语义约束（Tool 事件）

- `ToolCallStarted.tool_call_id` 与对应 `ToolCallCompleted.tool_call_id` 必须完全一致。
- 同一个 `tool_call_id` 代表一次完整工具调用生命周期（开始到结束）。
- 不同工具调用必须使用不同 `tool_call_id`，即使在同一 turn 内工具名相同。

## 测试

当前 crate 未单独提供 `tests/` 目录；通常通过依赖它的 crate（如 `openjax-core`、`openjaxd`、`tui_next`）进行集成验证。

可运行：

```bash
zsh -lc "cargo test -p openjax-protocol"
```

## 架构特点

- **协议中心化**：跨 crate 共享同一套操作与事件类型
- **序列化友好**：核心枚举均实现 `Serialize` / `Deserialize`
- **前向扩展**：多 agent 相关操作与事件已预留
- **类型约束清晰**：减少字符串协议字段漂移风险
