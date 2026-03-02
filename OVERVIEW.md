# OpenJax Overview

本文档是 OpenJax 的项目总览与导航入口（更新于 2026-02-23）。

## 1. 项目概览

OpenJax 是一个以 Rust 为内核、同时提供 Python 外层能力的 Agent 框架。  
当前仓库同时维护：

- Rust workspace 核心模块（协议、内核、daemon、CLI、Rust TUI）
- Python MVP 模块（SDK、Python TUI）
- 协议、工具与重构计划文档

## 2. Workspace 与子模块

Rust workspace（`Cargo.toml`）成员：

| 模块 | 职责 | README |
|------|------|--------|
| `openjax-protocol` | 协议类型与共享数据结构（`Op/Event`） | [openjax-protocol/README.md](openjax-protocol/README.md) |
| `openjax-core` | Agent 主循环、工具系统、模型客户端、沙箱与审批 | 暂无独立 README（可先看 [openjax-core/src/tools/README.md](openjax-core/src/tools/README.md)） |
| `openjaxd` | Rust daemon（JSONL 协议入口） | [openjaxd/README.md](openjaxd/README.md) |
| `openjax-cli` | CLI 入口与 REPL 交互 | 暂无独立 README |
| `tui_next` | Rust TUI 交互层（事件渲染、审批弹层） | [ui/tui/README.md](ui/tui/README.md) |

仓库内 Python 模块：

| 模块 | 职责 | README |
|------|------|--------|
| `python/openjax_sdk` | Python 异步 SDK（连接 `openjaxd`） | [python/openjax_sdk/README.md](python/openjax_sdk/README.md) |
| `python/openjax_tui` | Python TUI MVP | [python/openjax_tui/README.md](python/openjax_tui/README.md) |

## 3. 关键目录结构

```text
openjax-protocol/
openjax-core/
openjaxd/
openjax-cli/
ui/tui/
python/
  openjax_sdk/
  openjax_tui/
smoke_test/
docs/
```

## 4. 当前架构关系

```text
User
 ├─ openjax-cli (Rust)
 ├─ tui_next (Rust)
 └─ python/openjax_tui (Python MVP)
          │
          ▼
   openjaxd (Rust daemon, JSONL over stdio)
          │
          ▼
   openjax-core (Agent loop / tools / sandbox / approval)
          │
          ▼
   openjax-protocol (shared Op/Event types)
```

## 5. 子模块 README 导航

优先阅读以下文档以快速进入对应模块上下文：

- [openjax-protocol/README.md](openjax-protocol/README.md)
- [openjax-core/README.md](openjax-core/README.md)
- [openjaxd/README.md](openjaxd/README.md)
- [ui/tui/README.md](ui/tui/README.md)
- [python/openjax_sdk/README.md](python/openjax_sdk/README.md)
- [python/openjax_tui/README.md](python/openjax_tui/README.md)
- [openjax-core/src/tools/README.md](openjax-core/src/tools/README.md)

## 6. 相关文档入口

- 协议文档： [docs/protocol/v1/README.md](docs/protocol/v1/README.md)
- 工具系统： [docs/tools/README.md](docs/tools/README.md)
- 重构计划索引： [docs/plan/refactor/README.md](docs/plan/refactor/README.md)


## 7. 常用构建与测试命令

统一在仓库根目录执行：

```bash
zsh -lc "cargo build"
zsh -lc "cargo test --workspace"
zsh -lc "cargo test -p openjax-core"
zsh -lc "cargo test -p openjaxd"
zsh -lc "cargo test -p tui_next"
zsh -lc "PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest discover -s python/openjax_tui/tests -v"
```
