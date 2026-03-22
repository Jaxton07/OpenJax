# OpenJax Overview

本文档是 OpenJax 的项目总览与导航入口（更新于 2026-03-08）。

## 1. 项目概览

OpenJax 是一个以 Rust 为内核、同时提供外层能力的 Agent 框架。  
当前仓库同时维护：

- Rust workspace 核心模块（协议、内核、daemon、Rust TUI、gateway）
- Web 前端模块（React + Vite）
- Python SDK 模块（连接 daemon）
- 协议、工具与重构计划文档

## 2. Workspace 与子模块

Rust workspace（`Cargo.toml`）成员：

| 模块 | 职责 | README |
|------|------|--------|
| `openjax-protocol` | 协议类型与共享数据结构（`Op/Event`） | [openjax-protocol/README.md](openjax-protocol/README.md) |
| `openjax-core` | Agent 主循环、工具系统、模型客户端、沙箱与审批 | 暂无独立 README（可先看 [openjax-core/src/tools/README.md](openjax-core/src/tools/README.md)） |
| `openjaxd` | Rust daemon（JSONL 协议入口） | [openjaxd/README.md](openjaxd/README.md) |
| `openjax-gateway` | HTTP/SSE 网关（会话、turn、审批、事件流） | [openjax-gateway/README.md](openjax-gateway/README.md) |
| `tui_next` | Rust TUI 交互层（事件渲染、审批弹层） | [ui/tui/README.md](ui/tui/README.md) |

仓库内 Python 模块：

| 模块 | 职责 | README |
|------|------|--------|
| `python/openjax_sdk` | Python 异步 SDK（连接 `openjaxd`） | [python/openjax_sdk/README.md](python/openjax_sdk/README.md) |

仓库内 Web 模块：

| 模块 | 职责 | README |
|------|------|--------|
| `ui/web` | React Web 客户端（连接 `openjax-gateway`） | [ui/web/README.md](ui/web/README.md) |

## 3. 关键目录结构

```text
openjax-protocol/
openjax-core/
openjaxd/
openjax-gateway/
ui/tui/
ui/web/
python/
  openjax_sdk/
docs/
```

## 4. 当前架构关系

```text
User
 ├─ tui_next (Rust)
 ├─ ui/web (React)
 └─ openjax-gateway (HTTP + SSE)
          │
          ▼
     openjax-core
          │
          ▼
     openjax-protocol

Web/SDK
 └─ openjax-gateway (HTTP + SSE gateway)
          │
          ▼
     openjax-core

Daemon/SDK
 ├─ openjaxd (Rust daemon, JSONL over stdio)
 │        │
 │        ▼
 │   openjax-core
 └─ python/openjax_sdk
```

## 5. 子模块 README 导航

优先阅读以下文档以快速进入对应模块上下文：

- [openjax-protocol/README.md](openjax-protocol/README.md)
- [openjax-core/README.md](openjax-core/README.md)
- [openjaxd/README.md](openjaxd/README.md)
- [openjax-gateway/README.md](openjax-gateway/README.md)
- [ui/tui/README.md](ui/tui/README.md)
- [ui/web/README.md](ui/web/README.md)
- [python/openjax_sdk/README.md](python/openjax_sdk/README.md)
- [openjax-core/src/tools/README.md](openjax-core/src/tools/README.md)

## 6. 相关文档入口

- 协议文档： [docs/protocol/v1/README.md](docs/protocol/v1/README.md)
- 工具系统： [openjax-core/src/tools/docs/README.md](openjax-core/src/tools/docs/README.md)
- 重构计划索引： [docs/plan/refactor/README.md](docs/plan/refactor/README.md)


## 7. 常用构建与测试命令

统一在仓库根目录执行：

```bash
zsh -lc "cargo build"
zsh -lc "cargo test --workspace"
zsh -lc "cargo test -p openjax-core"
zsh -lc "cargo test -p openjaxd"
zsh -lc "cargo test -p openjax-gateway"
zsh -lc "cargo test -p tui_next"
zsh -lc "PYTHONPATH=python/openjax_sdk/src python3 -m unittest discover -s python/openjax_sdk/tests -v"
zsh -lc "cd ui/web && pnpm test"
```
