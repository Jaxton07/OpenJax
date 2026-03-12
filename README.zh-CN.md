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
- [开发者入口](#开发者入口)
- [文档导航](#文档导航)
- [安全](#安全)
- [贡献](#贡献)

## 核心能力

- 面向编码、自动化与日常任务的通用助理循环
- 文件读取/搜索、Shell 执行、补丁应用等工具能力
- 沙箱模式与审批策略
- Web UI 作为默认上手入口，Rust TUI（`tui_next`）作为可选交互方式
- 可扩展的多模型 Provider 配置

## 快速开始

### 推荐新用户：Web UI

```bash
curl -fsSL https://raw.githubusercontent.com/Jaxton07/OpenJax/main/scripts/release/install_from_github.sh | bash -s -- --yes
export PATH="$HOME/.local/openjax/bin:$PATH"
export OPENAI_API_KEY="<your_api_key>"
openjax-gateway
```

然后在浏览器打开 `http://127.0.0.1:8765`。
如果未配置 API Key 环境变量，gateway 启动时会在终端打印一个随机 Owner Key。
在 `/login` 页面填写该 key 后，Web 会换取 access/refresh token，且不会在本地持久化 owner key。
本地开发模式（`make run-web-dev`）前端地址仍是 `http://127.0.0.1:5173`。

### 可选：Rust TUI

若你偏好终端交互：

```bash
tui_next
```

## 安装方式

### 方式 A：从 GitHub Release 在线安装（推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/Jaxton07/OpenJax/main/scripts/release/install_from_github.sh | bash -s -- --yes
```

### 方式 B：预编译包安装（macOS ARM / Linux x86_64）

本地打包（以下示例为 macOS ARM）：

```bash
make doctor
make build-release-mac
make package-mac
```

在解压后的包目录安装：

```bash
./install.sh --prefix "$HOME/.local/openjax"
```

或直接从 GitHub Release 在线安装：

```bash
curl -fsSL https://raw.githubusercontent.com/Jaxton07/OpenJax/main/scripts/release/install_from_github.sh | bash -s -- --yes
```

添加到 `PATH` 并启动：

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
tui_next
```

预编译包默认包含 web 运行时（`~/.local/openjax/web`），`openjax-gateway` 会自动托管。

升级到最新版本：

```bash
bash scripts/release/upgrade.sh --yes
```

Linux/macOS 的打包命令与完整部署说明见 [docs/deployment.zh-CN.md](docs/deployment.zh-CN.md)。

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
用户 (Rust TUI / Web UI)
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
- `ui/tui/`：Rust TUI（`tui_next`）
- `openjax-gateway/`：面向 Web 的 HTTP/SSE 网关
- `ui/web/`：React Web UI
- `python/openjax_sdk/`：Python 异步 SDK
- `smoke_test/`：冒烟脚本

## 开发者入口

开发前置依赖、构建/格式化/测试命令，以及源码开发流程请查看 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 文档导航

- 总览： [OVERVIEW.md](OVERVIEW.md)
- 部署： [docs/deployment.md](docs/deployment.md)
- 中文部署： [docs/deployment.zh-CN.md](docs/deployment.zh-CN.md)
- 开发者发布流程： [docs/release-workflow.zh-CN.md](docs/release-workflow.zh-CN.md)
- 安全模型： [docs/security.md](docs/security.md)

## 安全

漏洞上报与安全策略请见 [SECURITY.md](SECURITY.md)。

## 贡献

开发者请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

MIT，详见 [LICENSE](LICENSE)。
