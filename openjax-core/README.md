# openjax-core

OpenJax 核心引擎总览页。  
建议阅读顺序：先看本页，再按需进入子模块 README。

## 模块总览

- `agent/`: Agent 生命周期、回合执行、规划循环、事件流。
- `model/`: 模型抽象、配置注册、多模型路由、协议适配器。
- `tools/`: 工具注册与调用编排（含审批、沙箱联动、apply_patch）。
- `streaming/`: 流式事件子系统（parser/orchestrator/sink/replay）。
- `sandbox/`: shell 工具的沙箱策略、后端执行、降级与审计。
- `skills/`: Claude/OpenClaw 兼容技能加载、匹配与 prompt 注入。

## Native Tool Calling 当前状态（2026-03-28）

- Phase 3 的 native loop 已完成，`openjax-core` 默认主路径为 native tool calling。
- `planner_tool_action.rs` 继续保持独立模块，承载工具执行守卫与失败收敛逻辑。
- 工具面已包含 `write_file` 和 `glob_files`。
- shell 工具结果语义已拆分：模型消费 `model_content`，事件/UI 展示使用 `display_output` 与 `shell_metadata`。

## 一级文件树

```text
openjax-core/
├── README.md
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── approval.rs
│   ├── logger.rs
│   ├── tests.rs
│   ├── agent/
│   ├── model/
│   ├── tools/
│   ├── streaming/
│   ├── sandbox/
│   └── skills/
└── tests/
```

## 子模块文档入口

- [Agent 模块文档](./src/agent/README.md)
- [Model 模块文档](./src/model/README.md)
- [Tools 模块文档](./src/tools/README.md)
- [Streaming 模块文档](./src/streaming/README.md)
- [Sandbox 模块文档](./src/sandbox/README.md)
- [Skills 模块文档](./src/skills/docs/README.md)

## 核心入口（代码）

- `src/lib.rs`: 对外导出 `Agent`、模型构建与策略类型。
- `src/config.rs`: 加载 `~/.openjax/config.toml`。
- `src/approval.rs`: 审批抽象 `ApprovalHandler` 与默认实现。

## 最小使用示例

```rust
use openjax_core::{Agent, SandboxMode};
use openjax_protocol::Op;

let cwd = std::env::current_dir().unwrap();
let mut agent = Agent::with_runtime(SandboxMode::WorkspaceWrite, cwd);
// 需要审批控制时，注入 PolicyRuntime：
// use openjax_policy::{runtime::PolicyRuntime, schema::DecisionKind, store::PolicyStore};
// agent.set_policy_runtime(Some(PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]))));

let events = agent
    .submit(Op::UserTurn {
        input: "tool:read_file path=README.md".to_string(),
    })
    .await;
```

## 测试分层（推荐）

日常开发建议按 `smoke -> feature -> full` 的顺序执行。

下面的 `make core-*` target 已落地，可直接使用。`make test-rust` 仍然保留为 workspace 级全量 Rust 测试入口。

| 层级 | Makefile target | 建议用途 | 等价命令 |
| --- | --- | --- | --- |
| smoke | `make core-smoke` | 关键路径快速验证：tools/sandbox + streaming 冒烟 | `zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite system_tools_are_registered_in_specs"`<br/>`zsh -lc "cargo test -p openjax-core --test streaming_suite submit_with_sink_emits_events_in_same_order_as_submit_result"` |
| feature | `make core-feature-skills` 等 | 按领域回归：skills/tools/streaming/approval/history | `zsh -lc "cargo test -p openjax-core --test skills_suite"`<br/>`zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"`<br/>`zsh -lc "cargo test -p openjax-core --test streaming_suite"`<br/>`zsh -lc "cargo test -p openjax-core --test approval_suite"`<br/>`zsh -lc "cargo test -p openjax-core --test approval_events_suite"`<br/>`zsh -lc "cargo test -p openjax-core --test core_history_suite"` |
| full | `make core-full` | openjax-core 全量测试 | `zsh -lc "cargo test -p openjax-core --tests"` |

## 常用命令

```bash
zsh -lc "cargo build -p openjax-core"
zsh -lc "cargo test -p openjax-core --tests"
zsh -lc "cargo test -p openjax-core --test skills_suite"
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
zsh -lc "cargo test -p openjax-core --test approval_suite"
zsh -lc "cargo test -p openjax-core --test approval_events_suite"
zsh -lc "cargo test -p openjax-core --test streaming_suite"
zsh -lc "cargo test -p openjax-core --test core_history_suite"
zsh -lc "make core-smoke"
zsh -lc "make core-feature-skills"
zsh -lc "make core-feature-tools"
zsh -lc "make core-feature-streaming"
zsh -lc "make core-feature-approval"
zsh -lc "make core-feature-history"
zsh -lc "make core-baseline"
```

如果需要单独定位回归，可以继续直接使用上面的单测/集成测试命令；这些命令不会被分层说明替代。
