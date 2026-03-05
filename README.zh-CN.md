# OpenJax

<p align="center">
  <strong>一个以 Rust 为主的 CLI/TUI Agent 框架，用于 AI 辅助编码工作流。</strong><br/>
  参考 Codex 风格的 tool calling、sandbox 和 approval 控制机制。
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">简体中文</a>
</p>

## 核心能力

- Agent 循环与工具调用编排
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

### 方式 A：源码安装（macOS / Linux / Windows）

```bash
make install-source
```

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
- OpenJax 与 Codex 对比： [docs/openjax-vs-codex-comparison.md](docs/openjax-vs-codex-comparison.md)

## 安全

漏洞上报与安全策略请见 [SECURITY.md](SECURITY.md)。

## 贡献

请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

MIT，详见 [LICENSE](LICENSE)。
