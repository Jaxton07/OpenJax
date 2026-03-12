# OpenJax 开发者发布流程（Feature -> Release）

本文面向维护者，描述从“功能开发完成”到“发布新版本（macOS arm64 + Linux x86_64）”的完整可执行流程。

## 0. 发布前提

- 默认分支：`main`
- 发布触发：推送 tag `v*`（例如 `v0.2.7`）
- 自动发布工作流：`.github/workflows/release.yml`
- 运行态产物：`tui_next`、`openjaxd`、`openjax-gateway`、`web/`

## 1. 功能合并前检查（开发者本地）

在功能分支完成开发后，先在仓库根目录执行：

```bash
make doctor
cargo fmt -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cd ui/web && pnpm install && pnpm build && pnpm test
```

通过后再发 PR，确保不会把发布问题带到 `main`。

## 2. 合并到 main 后的发布准备

1. 确认 `main` 上 CI 全绿（Rust/Web/Linux 包安装验收）。
2. 检查版本号（`Cargo.toml` workspace version）是否已更新到目标版本。
3. 检查 release 说明草稿（建议提前准备变更摘要、升级提示、已知问题）。

## 3. 本地预演打包（强烈建议）

在发布前先本地模拟一次：

```bash
make clean-dist
make build-release-mac
make package-mac
```

如果在 Linux 机器上，也执行：

```bash
make build-release-linux
make package-linux
```

检查产物：

```bash
ls dist/openjax-v*-macos-aarch64.tar.gz
ls dist/openjax-v*-linux-x86_64.tar.gz
cat dist/SHA256SUMS
```

## 4. 本地安装冒烟验证（解压即用）

以当前平台包为例：

```bash
TAR_FILE=$(ls -t dist/openjax-v*-*.tar.gz | head -n1)
TMP_DIR=$(mktemp -d)
tar -xzf "$TAR_FILE" -C "$TMP_DIR"
PKG_DIR=$(find "$TMP_DIR" -maxdepth 1 -type d -name "openjax-v*" | head -n1)

bash "$PKG_DIR/install.sh" --prefix "$TMP_DIR/openjax" --yes
test -x "$TMP_DIR/openjax/bin/tui_next"
test -x "$TMP_DIR/openjax/bin/openjaxd"
test -x "$TMP_DIR/openjax/bin/openjax-gateway"
test -f "$TMP_DIR/openjax/web/index.html"

"$TMP_DIR/openjax/bin/tui_next" --help
"$TMP_DIR/openjax/bin/openjaxd" --help
"$TMP_DIR/openjax/bin/openjax-gateway" --help

bash "$PKG_DIR/uninstall.sh" --prefix "$TMP_DIR/openjax" --yes
```

## 5. 创建发布 tag（正式触发发布）

确认 `main` HEAD 即发布内容后执行：

```bash
git checkout main
git pull
git tag v0.2.7
git push origin v0.2.7
```

`v0.2.7` 仅为示例，按实际版本替换。

推送 tag 后，GitHub Actions 会自动触发 `.github/workflows/release.yml`。

## 6. 观察自动发布流水线

在 GitHub Actions 中确认：

1. `build-release (linux-x86_64)` 通过
2. `build-release (macos-aarch64)` 通过
3. `publish` 通过

失败时先修复问题，再删除错误 tag 并重打：

```bash
git tag -d v0.2.7
git push --delete origin v0.2.7
```

修复后重新打新 tag（建议递增 patch 版本）。

## 7. 校验 GitHub Release 资产

发布成功后，检查 Release 页面是否包含：

- `openjax-v<version>-macos-aarch64.tar.gz`
- `openjax-v<version>-linux-x86_64.tar.gz`
- `SHA256SUMS`

并人工抽检在线安装：

```bash
curl -fsSL https://raw.githubusercontent.com/Jaxton07/OpenJax/main/scripts/release/install_from_github.sh | bash -s -- --version <version> --yes
```

并抽检在线升级（覆盖安装）：

```bash
bash scripts/release/upgrade.sh --version <version> --yes
```

## 8. 发布公告与升级说明

建议在 Release Notes 中至少包含：

1. 新功能摘要（用户可感知变化）
2. 兼容性说明（是否有破坏性变更）
3. 升级/回滚建议
4. 已知问题与临时规避方式

## 9. 回滚策略（发布后异常）

如果发现严重问题：

1. 立即在 Release Notes 标注问题版本
2. 修复后发布补丁版本（例如 `v0.2.8`）
3. 不建议覆盖已有 tag 对应资产，保持可追溯

## 10. 维护者快速清单（可复制）

```bash
# 1) 质量门
cargo fmt -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cd ui/web && pnpm build && pnpm test && cd ../..

# 2) 预演打包
make clean-dist
make build-release-mac package-mac
# (在 Linux runner/机器执行)
# make build-release-linux package-linux

# 3) 打 tag 触发发布
git checkout main && git pull
git tag vX.Y.Z
git push origin vX.Y.Z
```
