# 项目结构索引

本文档分为两部分：
1. 当前结构（截至 2026-02-20）
2. 重构目标结构（Rust 内核 + Python 外层）

---

## 1. 当前结构（As-Is）

OpenJax 当前是 Rust workspace，核心已包含 CLI 与 TUI 两条交互入口。

### 1.1 Workspace 包

| 包名 | 职责 |
|------|------|
| `openjax-protocol` | 协议类型与共享数据结构（`Op/Event`） |
| `openjax-core` | Agent 主循环、工具系统、模型客户端、沙箱与审批 |
| `openjaxd` | Rust daemon（JSONL 协议入口，MVP） |
| `python/openjax_sdk` | Python SDK（daemon 客户端与事件分发，MVP） |
| `python/openjax_tui` | Python TUI（MVP，命令行交互） |
| `openjax-cli` | CLI 入口与 REPL 交互 |
| `openjax-tui` | Rust TUI 交互层（事件流渲染、审批弹层） |
| `smoke_test` | 冒烟测试 |

### 1.2 关键目录

```text
openjax-protocol/
openjax-core/
openjaxd/
python/
  openjax_sdk/
  openjax_tui/
openjax-cli/
openjax-tui/
smoke_test/
docs/
```

### 1.3 当前架构关系

```text
User
 ├─ openjax-cli (Rust)
 └─ openjax-tui (Rust)
        │
        ▼
   openjaxd (Rust, MVP)
        │
        ▼
   openjax-core (Rust)
        │
        ▼
 openjax-protocol (Rust types)
```

---

## 2. 重构目标结构（To-Be）

目标是演进为“Rust 内核稳定承载 + Python 外层快速迭代”。

### 2.1 目标分层

| 层 | 模块 | 说明 |
|---|---|---|
| 内核层（Rust） | `openjax-core` | 保留 Agent/Tools/Sandbox/Approval/Model |
| 协议层（Rust+Schema） | `openjax-protocol` + `docs/protocol/v1/schema` | 跨语言协议与校验 |
| 服务层（Rust） | `openjaxd`（MVP 已落地） | 对外提供会话与事件流接口 |
| 外层能力（Python） | `python/openjax_sdk`（MVP 已落地） | Daemon 客户端、事件分发、delta 聚合 |
| 外层应用（Python） | `python/openjax_tui`（MVP） | 终端交互与审批命令入口 |
| 外层应用（Python） | `python/openjax_telegram`（规划中） | Bot 与平台集成 |

### 2.2 目标目录草案

```text
openjax-protocol/
openjax-core/
openjaxd/                       # 规划中：Rust daemon
openjax-cli/
openjax-tui/                    # 迁移期保留，作为回退实现
python/
  openjax_sdk/                  # 规划中
  openjax_tui/                  # 规划中
  openjax_telegram/             # 规划中
docs/
  protocol/
    v1/
      README.md
      schema/
```

### 2.3 目标架构关系

```text
Python TUI / Telegram / other integrations
                │
                ▼
        openjax_sdk (Python)
                │
                ▼
           openjaxd (Rust)
                │
                ▼
          openjax-core (Rust)
                │
                ▼
       openjax-protocol + schema
```

---

## 3. 模块职责边界（重构期约束）

1. Rust 内核负责安全边界与执行边界：
   - Tool Router
   - Sandbox / Approval
   - Agent Loop
2. Python 外层负责交互与生态接入：
   - TUI 体验
   - Bot/第三方平台连接
3. 跨语言仅通过协议通信，不在 Python 重写内核逻辑。

---

## 4. 文档与计划路径

### 4.1 重构主计划

- `docs/plan/refactor/phase-plan-and-todo.md`
- `docs/plan/rust-kernel-python-expansion-plan.md`

### 4.2 TUI 技术方向

- `docs/tui/technical-direction.md`

### 4.3 工具系统文档

- `docs/tools/overview.md`
- `docs/tools/architecture.md`

---

## 5. 构建与测试（当前可执行）

统一使用 `zsh`：

```bash
zsh -lc "cargo build"
zsh -lc "cargo build -p openjax-cli"
zsh -lc "cargo build -p openjax-tui"
zsh -lc "cargo test"
zsh -lc "cargo test -p openjax-core"
zsh -lc "cargo test -p openjax-tui"
zsh -lc "cargo test -p openjax-cli"
```

---

## 6. 重构进行中的文档约定

1. 只要新增跨语言接口，先更新协议文档再落代码。
2. 目录索引必须同步反映“已落地/规划中”状态，不混写。
3. Python 模块未落地前，统一标注“规划中”，避免误导。
