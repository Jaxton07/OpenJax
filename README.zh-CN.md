# OpenJax

<p align="center">
  <strong>一个安全优先、Rust 原生的 AI 助理运行时，面向真实生产自动化场景。</strong><br/>
  以可控执行为核心，强调沙箱隔离、严格审批和低依赖部署。
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

## 为什么选择 OpenJax

- 更安全的沙箱边界，降低高风险文件系统和环境副作用
- 更严格的权限审批，避免高影响操作被静默执行
- Rust-first 预编译交付，环境要求低，不需要安装一大堆额外依赖
- 兼容 Claude Code/OpenClaw 风格 `SKILL.md`（公共子集），已有 skill 可低成本复用
- 网关、守护进程、核心执行层边界清晰，更易审计与运维

OpenJax 追求的是安全可控、轻量易部署的自动化，而不是放任式自动执行。

## 目录

- [核心能力](#核心能力)
- [为什么选择 OpenJax](#为什么选择-openjax)
- [快速开始](#快速开始)
- [Web UI 截图](#web-ui-截图)
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
- 安全优先的沙箱机制与更严格的审批策略
- Web UI 作为默认上手入口，Rust TUI（`tui_next`）作为可选交互方式
- 可扩展的多模型 Provider 配置
- Rust-first 架构，部署依赖更轻、环境要求更低
- 兼容 Claude Code/OpenClaw 的 `SKILL.md` 约定（公共子集）

## 快速开始

### 推荐新用户：Web UI

**1. 安装**
```bash
curl -fsSL https://raw.githubusercontent.com/Jaxton07/OpenJax/main/scripts/release/install_from_github.sh | bash -s -- --yes
```

**2. 重新加载 PATH**
```bash
source ~/.zshrc   # 或重启终端
```

**3. 启动 gateway**
```bash
openjax-gateway
```

然后在浏览器打开 `http://127.0.0.1:8765`。

安装脚本会自动将 `~/.local/openjax/bin` 写入 `~/.zshrc`（或 `~/.bashrc`）。如需跳过此步骤，可传入 `--no-modify-path` 参数手动配置。

Gateway 首次启动时会在终端打印一个随机 Owner Key。在 `/login` 页面填写该 key 后，Web 会换取 access/refresh token，且不会在本地持久化 owner key。

LLM Provider 和 API Key 在启动后通过 **Web UI 设置页面** 配置。本地开发模式（`make run-web-dev`）前端地址为 `http://127.0.0.1:5173`。

### 可选：Rust TUI

若你偏好终端交互：

```bash
tui_next
```

## Web UI 截图

![Web UI 首页](docs/assets/screenshots/web-ui/chat_page_01.png)
![Web UI 会话页](docs/assets/screenshots/web-ui/chat_page_02.png)

## TUI 截图

![Rust TUI](docs/assets/screenshots/tui/openjax_tui.png)

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

安装完成后启动（PATH 已自动写入，重启终端或先执行 `source ~/.zshrc`）：

```bash
tui_next
```

预编译包默认包含 web 运行时（`~/.local/openjax/web`），`openjax-gateway` 会自动托管。

升级到最新版本：

```bash
openjax update
```

升级到指定版本：

```bash
openjax update --version 1.2.3
```

Linux/macOS 的打包命令与完整部署说明见 [docs/deployment.zh-CN.md](docs/deployment.zh-CN.md)。

## 配置项

LLM Provider 和 API Key 主要通过启动 `openjax-gateway` 后的 **Web UI 设置页面** 进行配置。

首次启动时会在 `~/.openjax/config.toml` 自动生成配置模板，支持多模型路由、按模型配置 API Key 及 fallback 链——高级用户可直接编辑该文件。

以下环境变量可在运行时覆盖配置文件中的对应值：

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `OPENAI_API_KEY` | OpenAI API key 覆盖 | - |
| `OPENJAX_KIMI_API_KEY` | Kimi API key 覆盖 | - |
| `OPENJAX_GLM_API_KEY` | GLM API key 覆盖 | - |
| `OPENJAX_ANTHROPIC_API_KEY` | Claude API key 覆盖 | - |
| `OPENJAX_SANDBOX_MODE` | 沙箱模式 | `workspace_write` |

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
