# OpenJax 部署指南（运行态优先）

英文版请见：[deployment.md](deployment.md)

开发者发布 SOP 请见：[release-workflow.zh-CN.md](release-workflow.zh-CN.md)

本指南面向运行态用户，当前支持：

- macOS arm64（Apple Silicon）
- Linux x86_64

运行态安装不依赖 Rust、Node、Python 开发环境。

## 包内容

每个 release 包默认包含：

- `bin/tui_next`
- `bin/openjaxd`
- `bin/openjax-gateway`
- `web/`（预构建前端静态资源）
- `install.sh`
- `uninstall.sh`
- `README-install.md`

`openjax-gateway` 默认从 `<install_prefix>/web` 托管前端页面。

## A. 离线安装（推荐）

1. 从 GitHub Releases 下载对应包：
- `openjax-v<version>-macos-aarch64.tar.gz`
- `openjax-v<version>-linux-x86_64.tar.gz`

2. 校验摘要（可选但推荐）：

```bash
shasum -a 256 openjax-v<version>-<platform>.tar.gz
```

3. 解压并安装：

```bash
tar -xzf openjax-v<version>-<platform>.tar.gz
cd openjax-v<version>-<platform>
./install.sh --prefix "$HOME/.local/openjax"
```

4. 配置 PATH 并运行：

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
openjax-gateway
```

在浏览器打开 `http://127.0.0.1:8765/login`，并填入 gateway 终端输出的 Owner Key。
Web 会通过 `/api/v1/auth/login` 换取 access/refresh token，其中 refresh token 存在 HttpOnly Cookie。

## B. 从 GitHub Release 在线安装（可选）

一行命令：

```bash
curl -fsSL https://raw.githubusercontent.com/Jaxton07/OpenJax/main/scripts/release/install_from_github.sh | bash -s -- --yes
```

在仓库目录执行：

```bash
bash scripts/release/install_from_github.sh --yes
```

常用参数：

- `--version 0.2.6` 安装指定版本（对应 tag `v0.2.6`）
- `--prefix <path>` 指定安装目录
- `--repo owner/name` 指定仓库

脚本会下载包与 `SHA256SUMS`，完成校验后调用包内 `install.sh`。

## C. 卸载

在包目录执行：

```bash
./uninstall.sh
```

保留用户数据目录（若存在）：

```bash
./uninstall.sh --keep-user-data
```

## D. 升级

在线升级（最新版本）：

```bash
bash scripts/release/upgrade.sh --yes
```

使用本地安装包离线升级：

```bash
bash scripts/release/upgrade.sh --from-package /path/to/openjax-v<version>-<platform>.tar.gz --yes
```

升级脚本默认会先停止 `openjax-gateway` / `openjaxd` / `tui_next`，再执行安装与可执行检查。

## E. 本地构建与打包（维护者）

macOS arm64：

```bash
make doctor
make build-release-mac
make package-mac
```

Linux x86_64：

```bash
make doctor
make build-release-linux
make package-linux
```

## F. CI/CD 发布流程

- CI（`.github/workflows/ci.yml`）校验 Rust、Web、以及 Linux 包安装/卸载冒烟测试。
- Release（`.github/workflows/release.yml`）在 `v*` tag 触发，构建 macOS/Linux 包、执行安装验收并上传到 GitHub Release。

## G. 开发环境说明（运行态用户可忽略）

只有开发者需要 Rust/Node。中国大陆网络下可选镜像示例：

```bash
# cargo
export CARGO_NET_RETRY=5
export CARGO_HTTP_MULTIPLEXING=false

# npm/pnpm 示例
npm config set registry https://registry.npmmirror.com
pnpm config set registry https://registry.npmmirror.com
```
