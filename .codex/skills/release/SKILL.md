---
name: release
description: 发布新版本技能。从 main 分支读取 Cargo.toml 版本号，验证环境与 CI 状态，打 tag 并推送以触发 GitHub Release 工作流。
---

# release - 发布新版本

从 main 分支读取当前 Cargo.toml 版本号，完成环境检查、CI 验证、用户确认后，打 `v{version}` tag 并推送，触发 GitHub Actions release 工作流自动构建并发布。

## 使用方式

```
/release
```

## 前置条件（人工完成）

1. 已更新根目录 `Cargo.toml` 中的 `version` 字段
2. 相关 PR 已合并到 `main`

## 执行步骤

### 步骤 1：记录当前分支，切换到 main

```bash
git branch --show-current
```

记录当前分支名为 `{origin_branch}`，后续用于切回。

若当前已在 `main`，直接进入步骤 2。

若不在 `main`：

1. 检查是否有未提交改动：
   ```bash
   git status --porcelain
   ```
2. 若有改动，**暂存**：
   ```bash
   git stash push -m "release: auto stash before switching to main"
   ```
   记录 stash 是否执行（`{did_stash}` = true/false），供步骤 7 恢复使用。
3. 切换到 main：
   ```bash
   git checkout main
   ```
   若切换失败，**立即终止**，若已 stash 则提示用户手动执行 `git stash pop` 恢复。

### 步骤 2：同步最新 main

```bash
git pull origin main
```

若 pull 失败（冲突或网络问题），终止并提示用户手动处理。

### 步骤 3：读取版本号

从根目录 `Cargo.toml` 提取版本号：

```bash
grep -m1 '^version' Cargo.toml
```

解析出版本号字符串（格式 `major.minor.patch`），构造 tag 名：`v{version}`。

检查该 tag 是否已存在于远端：

```bash
git ls-remote --tags origin "refs/tags/v{version}"
```

若 tag 已存在，**终止**并提示：
> "tag v{version} 已存在于远端，请检查 Cargo.toml 版本号是否已更新。"

### 步骤 4：验证 CI 状态

查看 main 分支最近的 CI 运行情况：

```bash
gh run list --branch main --limit 5 --json status,conclusion,displayTitle,url
```

分析最近一次 CI 运行的 `conclusion` 字段：
- `success`：继续
- 其他（`failure` / `cancelled` / 进行中）：**警告**用户并输出具体状态，告知 CI 未通过，建议确认后再决定是否继续

### 步骤 5：输出确认摘要，等待用户确认

打印以下发布摘要，**暂停并明确询问用户是否确认发布**：

```
即将执行以下操作：

  版本：v{version}
  Tag：v{version}
  推送至：origin
  将触发：GitHub Actions release 工作流（构建 linux-x86_64 + macos-aarch64 产物）

请确认是否继续发布？
```

**等待用户明确回复 yes / y / 确认 等肯定性内容后再继续。**
若用户回复否定或未确认，终止流程，不执行任何 tag 操作。

### 步骤 6：打 tag 并推送

```bash
git tag v{version}
git push origin v{version}
```

若 push 失败，提示错误原因，**不重试、不强制推送**。

### 步骤 7：切回原始分支并恢复暂存

tag 推送成功后，切回步骤 1 记录的 `{origin_branch}`：

```bash
git checkout {origin_branch}
```

若 `{origin_branch}` 为 `main`，则执行：

```bash
git checkout dev
git pull origin dev
```

若步骤 1 中执行了 stash（`{did_stash}` = true），恢复暂存：

```bash
git stash pop
```

若 `stash pop` 出现冲突，**不强制处理**，提示用户手动解决：
> "stash pop 出现冲突，请手动执行 `git stash pop` 并解决冲突。"

### 步骤 8：输出结果

打印以下信息：

```
v{version} 已发布！已切回 {origin_branch} 分支。

GitHub Actions 进度：
https://github.com/{owner}/{repo}/actions

Release 页面（构建完成后可见）：
https://github.com/{owner}/{repo}/releases/tag/v{version}
```

用 `gh repo view --json nameWithOwner` 获取 `{owner}/{repo}`。

## 注意事项

- **严禁** `git push --force` 或覆盖已有 tag
- **严禁** 在非 `main` 分支打 release tag
- tag 一旦推送即触发构建，无法撤回（只能删除 tag + release，但产物可能已被下载）
- 若需要回滚，手动执行：`git push origin :refs/tags/v{version}` 并在 GitHub 删除 release
