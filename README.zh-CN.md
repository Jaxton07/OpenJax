# OpenJax

<p align="center">
  <strong>一个以 Rust 为主的全能 AI 助理运行时，支持本地与云端多场景工作流。</strong><br/>
  基于 tool calling、sandbox 和 approval 控制机制构建。
</p>

<p align="center">
  <a href="https://github.com/Jaxton07/OpenJax"><img alt="GitHub Repo" src="https://img.shields.io/badge/GitHub-Repo-181717?logo=github"></a>
  <a href="https://github.com/Jaxton07/OpenJax/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/License-MIT-green.svg"></a>
  <a href="https://github.com/Jaxton07/OpenJax/commits/main"><img alt="Last Commit" src="https://img.shields.io/github/last-commit/Jaxton07/OpenJax"></a>
  <a href="https://github.com/Jaxton07/OpenJax/stargazers"><img alt="Stars" src="https://img.shields.io/github/stars/Jaxton07/OpenJax?style=social"></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <a href="OVERVIEW.md">项目总览</a> |
  <a href="CONTRIBUTING.md">参与贡献</a> |
  <a href="SECURITY.md">安全策略</a> |
  <a href="docs/deployment.zh-CN.md">部署文档</a>
</p>

## 目录

- [核心能力](#核心能力)
- [快速开始](#快速开始)
- [安装方式](#安装方式)
- [配置项](#配置项)
- [架构概览](#架构概览)
- [仓库结构](#仓库结构)
- [开发与测试](#开发与测试)
- [文档导航](#文档导航)
- [安全](#安全)
- [贡献](#贡献)

## 核心能力

- 面向编码、自动化与日常任务的通用助理循环
- 文件读取/搜索、Shell 执行、补丁应用等工具能力
- 沙箱模式与审批策略
- Rust TUI（`tui_next`）作为主交互界面
- 可扩展的多模型 Provider 配置

## 快速开始

### 前置条件

- Rust 工具链（`cargo`、`rustup`）
- `zsh`
- `OPENAI_API_KEY`（或你配置中支持的其他模型 Key）

### 源码运行

```bash
make doctor
make run-tui
```

等价命令：

```bash
cargo run -q -p tui_next
```

## 安装方式

### 方式 A：源码安装（本地仓库，一键命令）

```bash
make install-source
```

适用于你已经 `git clone` 到本地并进入仓库目录的场景。

### 方式 B：预编译包安装（仅 macOS ARM）

本地打包：

```bash
make doctor
make build-release-mac
make package-mac
```

在解压后的包目录安装：

```bash
./install.sh --prefix "$HOME/.local/openjax"
```

添加到 `PATH` 并启动：

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
tui_next
```

完整部署说明见 [docs/deployment.zh-CN.md](docs/deployment.zh-CN.md)。

## 配置项

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `OPENJAX_MODEL` | 模型后端 | `gpt-4.1-mini` |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `OPENJAX_KIMI_API_KEY` | Kimi API key | - |
| `OPENJAX_GLM_API_KEY` | GLM API key | - |
| `OPENJAX_ANTHROPIC_API_KEY` | Claude API key | - |
| `OPENJAX_APPROVAL_POLICY` | 审批策略 | `on_request` |
| `OPENJAX_SANDBOX_MODE` | 沙箱模式 | `workspace_write` |

若不存在配置文件，启动时会自动生成模板：
- `./.openjax/config/config.toml`（项目内）
- 回退到 `~/.openjax/config.toml`

## 架构概览

```text
用户 (CLI / Rust TUI / Python TUI MVP)
          |
          v
openjaxd (daemon)
          |
          v
openjax-core (agent loop, tools, sandbox, approval)
          |
          v
openjax-protocol (shared types/events)
```

## 仓库结构

- `openjax-core/`：agent loop、tools、sandbox、approval
- `openjax-protocol/`：协议/事件/数据类型
- `openjaxd/`：守护进程
- `openjax-cli/`：CLI 入口
- `ui/tui/`：Rust TUI（`tui_next`）
- `python/openjax_sdk/`：Python 异步 SDK
- `python/tui/`：Python TUI（备用 MVP）
- `smoke_test/`：冒烟脚本

## 开发与测试

在仓库根目录执行：

```bash
zsh -lc "cargo build"
zsh -lc "cargo fmt -- --check"
zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"
zsh -lc "cargo test --workspace"
```

对 `tests/` 下集成测试，建议使用明确的 `--test` 形式：

```bash
zsh -lc "cargo test -p openjax-core --test m3_sandbox"
```

## 文档导航

- 总览： [OVERVIEW.md](OVERVIEW.md)
- 部署： [docs/deployment.md](docs/deployment.md)
- 中文部署： [docs/deployment.zh-CN.md](docs/deployment.zh-CN.md)
- 安全模型： [docs/security.md](docs/security.md)

## 安全

漏洞上报与安全策略请见 [SECURITY.md](SECURITY.md)。

## 贡献

请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

MIT，详见 [LICENSE](LICENSE)。
