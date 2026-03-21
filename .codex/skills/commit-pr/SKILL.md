---
name: commit-pr
description: 完整提交并提 PR 技能。同步最新 main、提交本地改动、推送分支、创建 PR，一步完成。适用于功能开发或文档修改完成后需要提交并开 PR 的场景。
---

# commit-pr - 提交并提 PR

自动完成从同步 main 到创建 PR 的完整流程：同步最新 main → 暂存改动 → 生成提交信息 → 提交 → 推送 → 创建 PR。

## 使用方式

```
/commit-pr
```

## 执行步骤

### 步骤 1：确认当前分支

运行 `git branch --show-current`，确认不在 `main` 分支上。如果在 `main` 上，终止并提示用户切换到功能分支。

### 步骤 2：同步最新 main

```bash
git fetch origin main
```

检查是否有未提交改动（`git status`）：
- 有改动：执行 `git stash`，记录需要 pop
- 无改动：跳过 stash

执行 rebase：
```bash
git rebase origin/main
```

若 rebase 有冲突，终止流程并告知用户手动解决冲突后重新运行。
若之前做了 stash，执行：
```bash
git stash pop
```

### 步骤 3：查看改动

并行执行以下命令，了解本次改动内容：
```bash
git status
git diff
git diff --staged
git log --oneline -5
```

如果没有任何改动（working tree clean 且无 staged），终止并提示"没有需要提交的内容"。

### 步骤 4：分析改动，生成提交信息

根据改动文件路径和 diff 内容判断：

**提交类型：**
- `feat` ✨：新增功能
- `fix` 🐛：修复问题
- `docs` 📝：仅文档变更
- `refactor` ♻️：代码重构（无新功能/无 bug 修复）
- `chore` 🔧：构建/配置/脚本
- `test` ✅：测试相关

**范围（scope）：** 从改动文件路径提取主模块名，多个模块取前 2 个，用逗号分隔。

**格式：**
```
<emoji> <type>(<scope>): <中文简短描述>
```

示例：
- `✨ feat(gateway): 新增 SSE 断线重连支持`
- `🐛 fix(core, tools): 修复沙箱路径越界检查`
- `📝 docs(install): 更新安装说明与 Quick Start`

### 步骤 5：提交

使用 `git add` 逐一添加改动文件（**禁止使用 `git add .` 或 `git add -A`**，避免误提交无关文件）：
```bash
git add <file1> <file2> ...
```

创建提交：
```bash
git commit -m "$(cat <<'EOF'
<生成的提交信息>

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### 步骤 6：推送

```bash
git push -u origin <当前分支名>
```

若推送失败（非首次推送冲突），告知用户错误信息，不强制推送。

### 步骤 7：创建 PR

```bash
gh pr create \
  --title "<与 commit message 一致的标题>" \
  --base main \
  --body "$(cat <<'EOF'
## Summary

<根据 diff 提炼 1-3 条改动要点>

## Test plan

- [ ] <验证步骤 1>
- [ ] <验证步骤 2>

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

PR title 控制在 70 字以内，与 commit message 风格一致。

### 步骤 8：输出结果

打印 PR 链接，流程结束。

## 注意事项

- 若当前分支已有同名远程 PR，`gh pr create` 会报错，此时告知用户 PR 已存在并给出链接
- 不执行 `git push --force`，任何冲突由用户手动处理
- rebase 冲突时立即终止，不尝试自动解决
- commit message 和 PR title 统一使用中文描述，保持与项目历史风格一致
