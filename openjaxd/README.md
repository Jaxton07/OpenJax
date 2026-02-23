# openjaxd

OpenJax daemon 进程，负责通过 JSONL over stdio 暴露会话与 turn 执行接口，并将 `openjax-core` 事件映射为协议事件流。

## 项目结构

```
openjaxd/
├── README.md                  # 项目文档
├── Cargo.toml                 # crate 配置与依赖
├── src/
│   └── main.rs                # daemon 主程序（协议处理、会话管理、事件转发）
└── tests/
    └── protocol_integration.rs # 协议集成测试（start/stream/submit/shutdown）
```

## 各模块功能介绍

### 核心模块

| 模块 | 功能描述 |
|------|----------|
| `src/main.rs` | 解析 request envelope、校验协议版本、分发方法（`start_session`、`stream_events`、`submit_turn`、`resolve_approval`、`shutdown_session`） |
| `SessionState` | 每个会话维护 `Agent`、`streaming_enabled` 状态与审批处理器 |
| `DaemonApprovalHandler` | 守护进程内审批桥接器，缓存待决审批请求并接收 `resolve_approval` 回传 |
| `map_event(...)` | 将 `openjax_protocol::Event` 映射为对外 JSON 事件（`assistant_delta`、`tool_call_completed` 等） |
| `send_ok/send_error/send_event` | 统一响应/事件编码与 stdout 写出 |

### 测试模块

| 模块 | 功能描述 |
|------|----------|
| `tests/protocol_integration.rs` | 启动真实 `openjaxd` 子进程，覆盖协议 happy path、非法请求错误、长耗时 turn 的流式事件时序 |

## 支持的 RPC 方法

| 方法 | 说明 |
|------|------|
| `start_session` | 创建会话并返回 `session_id` |
| `stream_events` | 打开当前会话的事件流推送 |
| `submit_turn` | 提交用户输入，立即返回 `turn_id`，随后异步推送事件 |
| `resolve_approval` | 对待决审批请求回传 `approved` 决策 |
| `shutdown_session` | 关闭会话并触发 `Op::Shutdown` |

## 运行

在仓库根目录执行：

```bash
zsh -lc "cargo run -q -p openjaxd"
```

可配合 JSONL 请求进行手工联调（stdin 输入一行一个 envelope）。

## 环境变量配置

`openjaxd` 运行时会加载 `openjax-core` 配置，常用变量包括：

| 变量 | 说明 |
|------|------|
| `OPENAI_API_KEY` | 模型访问密钥 |
| `OPENJAX_MODEL` | 模型标识 |
| `OPENJAX_SANDBOX_MODE` | 沙箱模式 |
| `OPENJAX_APPROVAL_POLICY` | 审批策略 |

## 测试

运行 daemon 集成测试：

```bash
zsh -lc "cargo test -p openjaxd"
```

## 架构特点

- **协议边界清晰**：request/response/event envelope 统一处理
- **会话隔离**：每个 `session_id` 独立维护 Agent 与审批状态
- **流式优先**：`submit_turn` 与事件推送解耦，保证交互及时性
- **可观测性**：包含审批生命周期与 turn 阶段日志
