# openjax-core

OpenJax 核心引擎总览页。  
建议阅读顺序：先看本页，再按需进入子模块 README。

## 模块总览

- `agent/`: Agent 生命周期、回合执行、规划循环、事件流。
- `model/`: 模型抽象、配置注册、多模型路由、协议适配器。
- `tools/`: 工具注册与调用编排（含审批、沙箱联动、apply_patch）。
- `sandbox/`: shell 工具的沙箱策略、后端执行、降级与审计。
- `skills/`: Claude/OpenClaw 兼容技能加载、匹配与 prompt 注入。

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
│   ├── sandbox/
│   └── skills/
└── tests/
```

## 子模块文档入口

- [Agent 模块文档](./src/agent/README.md)
- [Model 模块文档](./src/model/README.md)
- [Tools 模块文档](./src/tools/README.md)
- [Sandbox 模块文档](./src/sandbox/README.md)
- [Skills 模块文档](./src/skills/docs/README.md)

## 核心入口（代码）

- `src/lib.rs`: 对外导出 `Agent`、模型构建与策略类型。
- `src/config.rs`: 加载 `~/.openjax/config.toml`。
- `src/approval.rs`: 审批抽象 `ApprovalHandler` 与默认实现。

## 最小使用示例

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

## 常用命令

```bash
zsh -lc "cargo build -p openjax-core"
zsh -lc "cargo test -p openjax-core"
zsh -lc "cargo test -p openjax-core --test m3_sandbox"
zsh -lc "cargo test -p openjax-core --test m4_apply_patch"
zsh -lc "cargo test -p openjax-core --test m5_approval_handler"
zsh -lc "cargo test -p openjax-core --test m6_submit_stream"
zsh -lc "cargo test -p openjax-core --test m7_backward_compat_submit"
zsh -lc "cargo test -p openjax-core --test m8_approval_event_emission"
zsh -lc "cargo test -p openjax-core --test m9_system_tools"
zsh -lc "bash openjax-core/tests/tool/test_apply_patch_e2e.sh"
```
