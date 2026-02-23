# openjax-core

OpenJax 的核心 Agent 引擎，负责模型调用、工具编排、审批流程、沙箱策略与事件流输出。

## 项目结构

```text
openjax-core/
├── README.md                              # 当前文档
├── Cargo.toml                             # crate 配置
├── src/
│   ├── lib.rs                             # 对外入口与核心类型定义（薄入口）
│   ├── config.rs                          # 配置加载（工作区/用户目录）
│   ├── approval.rs                        # 审批接口与默认 stdin 审批器
│   ├── logger.rs                          # 日志初始化与滚动归档
│   ├── tests.rs                           # crate 内部单元测试
│   ├── agent/                             # Agent 生命周期/回合/执行/状态模块
│   │   ├── mod.rs                         # agent 子模块导出
│   │   ├── bootstrap.rs                   # Agent 构造与 runtime 初始化
│   │   ├── lifecycle.rs                   # thread/depth/sub-agent 生命周期接口
│   │   ├── turn.rs                        # submit/submit_with_sink 回合入口
│   │   ├── planner.rs                     # 自然语言规划循环与 final 流式回复
│   │   ├── execution.rs                   # 单工具执行、重试、live events
│   │   ├── state.rs                       # 速率限制、历史、重复调用记录
│   │   ├── events.rs                      # 事件聚合与 sink 推送
│   │   ├── decision.rs                    # 模型 decision 解析/规范化
│   │   ├── prompt.rs                      # planner/final/repair prompt 构造
│   │   └── runtime_policy.rs              # approval/sandbox 解析与优先级
│   ├── model/                             # 模型客户端抽象与多后端实现
│   │   ├── mod.rs                         # model 子模块导出
│   │   ├── client.rs                      # ModelClient trait
│   │   ├── echo.rs                        # Echo fallback
│   │   ├── factory.rs                     # backend 选择与 fallback 顺序
│   │   └── chat_completions.rs            # OpenAI 兼容 ChatCompletions 实现（含流式解析）
│   └── tools/                             # 工具系统
│       ├── mod.rs                         # tools 模块导出
│       ├── router.rs                      # tool: 指令解析与运行时配置
│       ├── router_impl.rs                 # ToolRouter 执行入口
│       ├── orchestrator.rs                # 工具执行编排（hooks/审批/沙箱/派发）
│       ├── registry.rs                    # ToolRegistry 与 ToolHandler trait
│       ├── tool_builder.rs                # 默认工具注册与 ToolSpec 构建
│       ├── context.rs                     # ToolInvocation/ToolOutput/Policy 类型
│       ├── spec.rs                        # 工具 JSON Schema 规范
│       ├── sandboxing.rs                  # 沙箱策略与变更型工具识别
│       ├── apply_patch_interceptor.rs     # shell 中 apply_patch 拦截
│       ├── handlers/                      # 具体工具处理器实现
│       │   ├── read_file.rs
│       │   ├── list_dir.rs
│       │   ├── grep_files.rs
│       │   ├── shell.rs
│       │   ├── apply_patch.rs
│       │   └── edit_file_range.rs
│       └── apply_patch/                   # apply_patch 语法与执行引擎
│           ├── parser.rs
│           ├── planner.rs
│           ├── applier.rs
│           ├── matcher.rs
│           ├── heredoc.rs
│           ├── types.rs
│           └── grammar.lark
└── tests/
    ├── m3_sandbox.rs                      # 沙箱策略测试
    ├── m4_apply_patch.rs                  # apply_patch 集成测试
    ├── m5_approval_handler.rs             # 审批策略测试
    ├── m5_edit_file_range.rs              # edit_file_range 测试
    ├── m6_submit_stream.rs                # submit_with_sink 事件流测试
    ├── m7_backward_compat_submit.rs       # submit 兼容性测试
    ├── m8_approval_event_emission.rs      # 审批事件发射测试
    └── tool/test_apply_patch_e2e.sh       # CLI 端到端冒烟脚本
```

## 各模块功能介绍

### 核心模块

| 模块 | 功能描述 |
|------|----------|
| `lib.rs` | 对外 API 薄入口：导出 `Agent`、配置/审批/模型构建器与协议类型 |
| `agent/*` | Agent 内核实现拆分：构造、回合分发、工具执行、规划循环、状态管理与事件推送 |
| `model/*` | 统一 `ModelClient` 抽象，支持 OpenAI Chat Completions、GLM、MiniMax 与 Echo fallback |
| `config.rs` | 读取 `.openjax/config/config.toml` 或 `~/.openjax/config.toml`，解析 model/sandbox/agent/tools 配置 |
| `approval.rs` | 审批抽象 `ApprovalHandler`，默认实现 `StdinApprovalHandler`（`y` 同意） |
| `logger.rs` | tracing 日志初始化，支持单文件按行数轮转与归档清理 |

### 工具系统模块

| 模块 | 功能描述 |
|------|----------|
| `tools/router.rs` | 解析 `tool:<name> key=value` 文本协议，定义 `SandboxMode` 与 `ToolRuntimeConfig` |
| `tools/router_impl.rs` | `ToolRouter::execute` 执行入口，将参数封装为 `ToolInvocation` 并调度 orchestrator |
| `tools/orchestrator.rs` | 工具执行总控：前后 hooks、审批请求/回传、沙箱选择、调用注册表 |
| `tools/registry.rs` | 工具注册与分发中心，定义 `ToolHandler` trait 与 payload 匹配机制 |
| `tools/spec.rs` | 构建工具规范（`read_file`/`shell`/`apply_patch` 等）的输入输出 Schema |
| `tools/handlers/*.rs` | 各工具 handler 实现，处理参数解析、路径校验、命令执行与结果格式化 |
| `tools/apply_patch/*` | apply_patch 的解析、动作规划、hunk 匹配与原子化落盘能力 |

### 默认工具能力

| 工具 | 说明 |
|------|------|
| `read_file` | 按行分页读取，支持 `slice` 和 `indentation` 模式 |
| `list_dir` | 目录树分页列出，支持深度限制 |
| `grep_files` | 基于 `rg` 搜索文件路径（按修改时间排序） |
| `shell` | 执行 shell 命令，支持 `require_escalated` 审批触发和超时控制 |
| `exec_command` | `shell` 的兼容别名 |
| `apply_patch` | 基于补丁语法安全修改文件（ADD/UPDATE/DELETE/MOVE） |
| `edit_file_range` | 按闭区间行号替换文本（1-indexed） |

## 运行与集成

在仓库根目录执行：

```bash
zsh -lc "cargo build -p openjax-core"
```

在代码中使用：

```rust
use openjax_core::{Agent, ApprovalPolicy, SandboxMode};
use openjax_protocol::Op;

let cwd = std::env::current_dir().unwrap();
let mut agent = Agent::with_runtime(
    ApprovalPolicy::OnRequest,
    SandboxMode::WorkspaceWrite,
    cwd,
);

let events = agent
    .submit(Op::UserTurn {
        input: "tool:read_file path=README.md".to_string(),
    })
    .await;
```

## 事件与回合处理

默认一个用户回合的事件序列包含：

1. `TurnStarted`
2. `ToolCallStarted`（命中工具时）
3. `ApprovalRequested` / `ApprovalResolved`（按策略触发）
4. `ToolCallCompleted` 或 `AssistantMessage`
5. `TurnCompleted`

若需要实时消费事件流，可使用 `submit_with_sink(op, tx)`，其返回值与 sink 流出的事件序列一致（见 `m6_submit_stream.rs`）。

## 环境变量配置

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `OPENJAX_APPROVAL_POLICY` | 审批策略：`always_ask` / `on_request` / `never` | `on_request` |
| `OPENJAX_SANDBOX_MODE` | 沙箱模式：`workspace_write` / `danger_full_access` | `workspace_write` |
| `OPENJAX_MODEL` | OpenAI 模型名 | `gpt-4.1-mini` |
| `OPENAI_API_KEY` | OpenAI API Key | 未设置 |
| `OPENAI_BASE_URL` | OpenAI 兼容接口地址 | `https://api.openai.com/v1` |
| `OPENJAX_GLM_API_KEY` | GLM API Key | 未设置 |
| `OPENJAX_GLM_MODEL` | GLM 模型名 | `GLM-4.7` |
| `OPENJAX_GLM_BASE_URL` | GLM 接口地址 | `https://open.bigmodel.cn/api/coding/paas/v4` |
| `OPENJAX_MINIMAX_API_KEY` | MiniMax API Key | 未设置 |
| `OPENJAX_MINIMAX_MODEL` | MiniMax 模型名 | `codex-MiniMax-M2.1` |
| `OPENJAX_MINIMAX_BASE_URL` | MiniMax 接口地址 | `https://api.minimaxi.com/v1` |
| `OPENJAX_LOG` | 关闭日志写入（设为 `off`） | 启用 |
| `OPENJAX_LOG_LEVEL` | 日志级别：`trace/debug/info/warn/error` | `info` |
| `OPENJAX_LOG_MAX_LINES` | 单文件最大行数 | `10000` |
| `OPENJAX_LOG_MAX_ARCHIVES` | 最大归档文件数 | `4` |

## 测试

运行核心测试：

```bash
zsh -lc "cargo test -p openjax-core"
```

运行关键集成测试：

```bash
zsh -lc "cargo test -p openjax-core --test m3_sandbox"
zsh -lc "cargo test -p openjax-core --test m4_apply_patch"
zsh -lc "cargo test -p openjax-core --test m5_approval_handler"
zsh -lc "cargo test -p openjax-core --test m6_submit_stream"
zsh -lc "cargo test -p openjax-core --test m7_backward_compat_submit"
```

运行 apply_patch e2e 脚本：

```bash
zsh -lc "bash openjax-core/tests/tool/test_apply_patch_e2e.sh"
```

## 架构特点

- **Agent 与工具解耦**：`Agent` 只关注回合和事件，工具执行交由 `tools` 子系统。
- **策略可控**：审批和沙箱由 runtime 配置统一控制，支持环境变量覆盖。
- **事件驱动**：所有关键阶段都可观察（开始、审批、完成、失败）。
- **补丁优先编辑**：`apply_patch` 提供可审计、可回滚的结构化文件修改路径。
- **兼容性设计**：保留 `exec_command` 别名和 `submit` 行为，降低上层迁移成本。

## 最近重构说明

- `src/lib.rs` 已收敛为薄入口；主业务逻辑迁移至 `src/agent/`。
- `src/model.rs` 已拆分为 `src/model/` 目录模块（`client/echo/factory/chat_completions`）。
- crate 内单元测试已从 `lib.rs` 内联模块迁移到 `src/tests.rs`，便于后续持续拆分。
