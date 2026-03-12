# OpenJax 部署指南（短期）

英文版请见：[deployment.md](deployment.md)

当前部署标准：

1. 预编译安装：支持 **macOS ARM / Linux x86_64 / Windows x86_64**
2. 源码安装：支持 **macOS / Linux / Windows**
3. 一键卸载：支持 `--keep-user-data`

## 约束与决策

- 预编译目标：
  - `macOS arm64 (Apple Silicon)`
  - `Linux x86_64`
  - `Windows x86_64`
- 默认安装目录：`~/.local/openjax`
- 分发方式：手工打包 + 手工上传
- 默认卸载策略：删除 `~/.local/openjax` 下所有文件
- 向前兼容参数：`--keep-user-data`

## A. 预编译安装（macOS ARM）

预编译包中自带 `install.sh`，用于实际安装。

### Step A：获取预编译包

方式 1：在仓库本地打包

```bash
make doctor
make build-release-mac
make package-mac
```

产物：

- `dist/openjax-v<version>-macos-aarch64.tar.gz`
- `dist/SHA256SUMS`

方式 2：从发布渠道下载 `openjax-v<version>-macos-aarch64.tar.gz`。

### Step B：解压并进入目录

```bash
cd dist
TAR_FILE=$(ls openjax-v*-macos-aarch64.tar.gz | head -n1)
tar -xzf "$TAR_FILE"
DIR_NAME=$(basename "$TAR_FILE" .tar.gz)
cd "$DIR_NAME"
```

### Step C：执行安装脚本

```bash
./install.sh
```

可选：自定义安装前缀

```bash
./install.sh --prefix "$HOME/.local/openjax"
```

### Step D：配置 PATH 并启动

```bash
export PATH="$HOME/.local/openjax/bin:$PATH"
tui_next
```

如果要长期生效，把上面的 `export` 写入 `~/.zshrc` 等 shell 配置文件。

### 校验

```bash
test -x "$HOME/.local/openjax/bin/tui_next"
openjaxd --help
```

## B. 预编译安装（Linux x86_64）

本地打包：

```bash
make build-release-linux
make package-linux
```

从包目录安装：

```bash
cd dist
TAR_FILE=$(ls openjax-v*-linux-x86_64.tar.gz | head -n1)
tar -xzf "$TAR_FILE"
DIR_NAME=$(basename "$TAR_FILE" .tar.gz)
cd "$DIR_NAME"
./install.sh
```

## C. 预编译安装（Windows x86_64）

在 PowerShell 打包：

```powershell
cargo build --release --locked -p tui_next -p openjaxd
powershell -ExecutionPolicy Bypass -File scripts/release/package_windows.ps1
```

从包目录安装：

```powershell
Expand-Archive .\dist\openjax-v<version>-windows-x86_64.zip -DestinationPath .\dist -Force
cd .\dist\openjax-v<version>-windows-x86_64
.\install.ps1
```

## D. 源码安装（本地仓库，一键命令）

适用于已经在本地仓库目录中开发的场景：

```bash
make install-source
```

## E. 源码安装（从 Git 克隆，手工步骤）

### macOS / Linux (bash/zsh)

```bash
git clone <your-repo-url> openJax
cd openJax
cargo build --release --locked -p tui_next -p openjaxd
mkdir -p "$HOME/.local/openjax/bin"
cp target/release/tui_next "$HOME/.local/openjax/bin/tui_next"
cp target/release/openjaxd "$HOME/.local/openjax/bin/openjaxd"
chmod +x "$HOME/.local/openjax/bin/tui_next" "$HOME/.local/openjax/bin/openjaxd"
```

### Windows (PowerShell)

```powershell
git clone <your-repo-url> openJax
cd openJax
cargo build --release --locked -p tui_next -p openjaxd
$prefix = Join-Path $HOME ".local/openjax/bin"
New-Item -ItemType Directory -Force -Path $prefix | Out-Null
Copy-Item "target/release/tui_next.exe" (Join-Path $prefix "tui_next.exe") -Force
Copy-Item "target/release/openjaxd.exe" (Join-Path $prefix "openjaxd.exe") -Force
```

## F. 卸载

### 默认全清理

```bash
./uninstall.sh
```

或在仓库中：

```bash
make uninstall-local
```

通过 Makefile 保留用户数据目录：

```bash
make uninstall-local KEEP_USER_DATA=1
```

### 保留用户数据（未来兼容）

```bash
./uninstall.sh --keep-user-data
```

当前行为：

- 若 `<prefix>/userdata` 存在，则保留该目录。
- 若不存在，则行为等价于全清理。

## G. 弱网建议

```bash
export CARGO_NET_RETRY=5
export CARGO_HTTP_MULTIPLEXING=false
```

可选预热：

```bash
cargo fetch --locked
```

## H. 手工发布 SOP

1. `make doctor`
2. 选择目标平台打包：
- macOS ARM：`make build-release-mac && make package-mac`
- Linux x86_64：`make build-release-linux && make package-linux`
- Windows x86_64：`cargo build --release --locked -p tui_next -p openjaxd` 后执行 `powershell -ExecutionPolicy Bypass -File scripts/release/package_windows.ps1`
3. 在干净目录验证：
- 解压包
- 执行安装脚本（`install.sh` 或 `install.ps1`）
- 校验 `tui_next` 可执行，`openjaxd --help` 正常
- 执行卸载脚本（`uninstall.sh` 或 `uninstall.ps1`）
4. 上传安装包（`.tar.gz`/`.zip`）与 `SHA256SUMS`
