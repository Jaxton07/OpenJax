# OpenJax 与 Codex 技术对比分析

本文档对比 OpenJax 项目与参考实现 Codex 的架构差异，帮助理解项目定位和后续扩展方向。

## 1. 整体架构方向

**结论：两者大体方向一致，OpenJax 是 Codex 的简化实现版本。**

两者采用相同的核心架构模式：
- Agent 循环：模型 → 工具调用 → 结果 → 模型（多轮迭代）
- 工具路由器：统一分发接口
- 沙箱 + 审批策略：安全保障
- 协议层：Op → Event 事件流

---

## 2. 架构对比表

| 层面 | Codex | OpenJax | 状态 |
|------|-------|---------|------|
| **Agent 循环** | 复杂的 orchestrator + control + guards | 简化的同步循环（最多5次） | ✅ 已实现 |
| **模型通信** | WebSocket 流式（首选）+ HTTP 降级 | HTTP 轮询 | ✅ 已实现（简化版） |
| **工具数量** | 10+ (含 MCP, BM25, JS REPL, Shell) | 5 个核心工具 | 🔶 可扩展 |
| **沙箱机制** | 平台特定实现（Seatbelt/Seccomp/RestrictedToken） | 白名单 + 语法检查 | 🔶 简化实现 |
| **审批策略** | Policy 规则文件 + Decision 引擎 | 三档策略（AlwaysAsk/OnRequest/Never） | ✅ 已实现 |
| **协议层** | Submission/Event 异步队列 | Op/Event 枚举 | ✅ 已实现 |
| **TUI** | Ratatui 完整交互界面 | 暂无 | ❌ 待实现 |
| **MCP 集成** | 完整 MCP 客户端 | 暂无 | ❌ 待实现 |
| **会话管理** | ~/.codex/ 持久化（sessions/events） | 内存 history | 🔶 可扩展 |

---

## 3. 已实现能力详解

### 3.1 Agent 循环

**Codex**: `core/src/agent/orchestrator.rs`
- 复杂的状态机（TurnStarted → ToolCall → TurnCompleted）
- 深度限制检查（guards）
- 支持中断和恢复

**OpenJax**: `openjax-core/src/lib.rs:82-303`
```rust
// 核心循环：最多 5 次工具调用
for executed_count in 0..MAX_TOOL_CALLS_PER_TURN {
    // 1. 构建 planner 输入
    // 2. 调用模型获取决策
    // 3. 解析 JSON 响应
    // 4. 执行工具
    // 5. 收集输出，继续循环
}
```

支持两种输入模式：
- 直接工具调用：`tool:read_file path=src/lib.rs`
- 自然语言：模型自主决策工具调用

### 3.2 工具系统

**Codex**: `core/src/tools/`
- `apply_patch.rs` - 代码补丁
- `shell.rs` / `unified_exec.rs` - 命令执行
- `read_file.rs` - 读取文件
- `list_dir.rs` - 列出目录
- `grep_files.rs` - 文件搜索
- `search_tool_bm25.rs` - BM25 搜索
- `mcp.rs` - MCP 工具扩展

**OpenJax**: `openjax-core/src/tools.rs`

| 工具 | 路径验证 | 功能 |
|------|----------|------|
| `read_file` | ✅ 防止逃逸 | 读取文件内容 |
| `list_dir` | ✅ 防止逃逸 | 列出目录 |
| `grep_files` | ✅ 防止逃逸 | 文本搜索 |
| `exec_command` | ✅ 审批 + 白名单 | Shell 命令执行 |
| `apply_patch` | ✅ 路径验证 + 回滚 | Add/Update/Delete 文件 |

### 3.3 沙箱机制

**Codex**: 平台特定实现
- macOS: `sandbox-exec` (Seatbelt)
- Linux: bubblewrap + seccomp
- Windows: Restricted Token

**OpenJax**: `openjax-core/src/tools.rs:697-754`

两种模式：
1. **WorkspaceWrite**（默认）
   - 白名单命令：`pwd`, `ls`, `cat`, `rg`, `find`, `head`, `tail`, `wc`, `sed`, `awk`, `echo`, `stat`, `uname`, `which`, `env`, `printf`
   - 阻止网络命令：`curl`, `wget`, `ssh`, `scp`, `nc`, `nmap`, `ping`, `sudo`
   - 阻止 shell 操作符：`&&`, `||`, `|`, `;`, `>`, `<`, `` ` ``, `$()`
   - 路径验证：禁止绝对路径、父目录遍历

2. **DangerFullAccess**
   - 无限制（仅审批策略生效）

### 3.4 审批策略

**Codex**: `core/src/exec_policy.rs`
```rust
// .rules 文件格式
allow prefix "cargo test"
prompt prefix "rm -rf"
forbidden "sudo"
```

**OpenJax**: `openjax-core/src/tools.rs:14-39`

```rust
pub enum ApprovalPolicy {
    AlwaysAsk,   // 始终询问
    OnRequest,  // 仅高风险命令询问
    Never,      // 从不询问
}
```

### 3.5 模型客户端

**Codex**: `core/src/client.rs`
- WebSocket 流式（首选，responses_websockets=2026-02-04）
- HTTP 降级
- 粘性路由状态

**OpenJax**: `openjax-core/src/model.rs`
- HTTP Chat Completions API
- 多后端支持：MiniMax → OpenAI → Echo
- 兼容各 provider 的 content 格式

---

## 4.

根据 Codex 架构，建议后续扩展建议按以下顺序扩展 OpenJax：

### 4.1 高优先级

1. **TUI 交互界面**
   - 使用 ratatui 库
   - 参考 Codex: `codex-rs/tui/`
   - 实现：聊天窗口、命令执行、审批确认

2. **会话持久化**
   - `~/.openjax/sessions/<thread_id>/`
   - 存储：meta.json, config.toml, events/

### 4.2 中优先级

3. **WebSocket 流式**
   - 改进 `model.rs` 支持 WebSocket
   - 降低工具调用延迟
   - 参考 Codex: `core/src/client.rs`

4. **MCP 集成**
   - 连接 MCP 服务器
   - 动态工具注册
   - 参考 Codex: `core/src/mcp/`

### 4.3 低优先级（可选）

5. **平台沙箱**
   - macOS Seatbelt
   - Linux Seccomp/Landlock
   - 当前白名单方式已够用

6. **更多工具**
   - BM25 搜索
   - JS REPL

### 4.4 多代理支持（已预留接口）

参考 Codex 的 `AgentControl` + `Guards` 机制，OpenJax 已预留以下扩展接口：

```rust
// openjax-protocol/src/lib.rs
pub const MAX_AGENT_DEPTH: i32 = 1;  // 最大子代理深度

pub enum Op {
    SpawnAgent { input: String, source: AgentSource },
    SendToAgent { thread_id: ThreadId, input: String },
    InterruptAgent { thread_id: ThreadId },
    ResumeAgent { rollout_path: String, source: AgentSource },
}

pub enum AgentStatus {
    PendingInit, Running, Completed, Errored, Interrupted, Shutdown, NotFound
}
```

```rust
// openjax-core/src/lib.rs
pub struct Agent {
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
    depth: i32,
}

impl Agent {
    pub fn can_spawn_sub_agent(&self) -> bool;
    pub fn spawn_sub_agent(&self, input: &str) -> Result<Agent>;
}
```

**扩展路径**：
1. 实现 `spawn_sub_agent` 创建子代理实例
2. 实现多代理调度器（类似 Codex 的 ThreadManager）
3. 实现槽位管理和状态订阅
4. 实现会话持久化和恢复

---

## 5. 总结

| 维度 | Codex | OpenJax |
|------|-------|---------|
| **定位** | 生产级 CLI 编程代理 | 精简版个人助理框架 |
| **复杂度** | 高（完整功能） | 低（核心能力） |
| **可扩展性** | 成熟 | 预留接口 |
| **适用场景** | 专业开发环境 | 个人助理/学习参考 |

OpenJax 保留了 Codex 的核心能力：
- ✅ Agent 循环
- ✅ 工具系统 + 路径安全
- ✅ 沙箱机制
- ✅ 审批策略
- 🔶 多代理接口（预留扩展）

同时简化了：
- ❌ TUI（待实现）
- ❌ MCP（待实现）
- ❌ 会话持久化（待实现）
- ❌ 平台沙箱（白名单已够用）

这个架构非常适合作为"定制化个人助理"的基础，可在后续迭代中添加 TUI、MCP 等高级功能。

---

## 6. 参考文档

- [Codex 架构参考](codex-architecture-reference.md)
- [Codex 快速参考](codex-quick-reference.md)
- OpenJax 源码：
  - [lib.rs](../openjax-core/src/lib.rs)
  - [tools.rs](../openjax-core/src/tools.rs)
  - [model.rs](../openjax-core/src/model.rs)
