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
3. 本地已切换到 `main` 分支

## 执行步骤

### 步骤 1：确认当前分支为 main

```bash
git branch --show-current
```

若不在 `main` 分支，**立即终止**并提示：
> "release 只能从 main 分支发布，请先切换到 main 并确认 PR 已合并。"

### 步骤 2：同步最新 main

```bash
git pull origin main
```

若 pull 失败（冲突或网络问题），终止并提示用户手动处理。

检查 working tree 是否干净：

```bash
git status --porcelain
```

若有未提交改动，**终止**并提示：
> "存在未提交改动，请先处理后再发布。"

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

### 步骤 7：切回 dev 分支

tag 推送成功后，立即切回 dev 分支，避免后续操作误在 main 上进行：

```bash
git checkout dev
git pull origin dev
```

### 步骤 8：输出结果

打印以下信息：

```
v{version} 已发布！已切回 dev 分支。

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
