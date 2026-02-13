---
name: mpush
description: 智能代码提交技能。
context: fork
---

# mpush - 智能代码提交

这个 skill 帮助你智能地提交代码变更到 Git 仓库。
自动分析代码变更，生成简洁的中文提交信息，并提交到 Git 仓库。支持识别修改类型（feat/fix/refactor 等）和相关模块，生成符合规范的提交信息。

## 功能

1. 分析所有修改的文件
2. 查看代码差异内容
3. 根据修改内容智能生成简洁的中文提交信息
4. 添加文件到暂存区
5. 创建提交
6. 推送到远程仓库

## 执行步骤

### 步骤 1：查看修改状态

首先运行 `git status` 查看所有修改的文件：
```bash
git status
```

### 步骤 2：查看代码差异

运行 `git diff` 查看具体的代码修改内容：
```bash
git diff
```

如果需要查看已暂存的修改：
```bash
git diff --staged
```

### 步骤 3：分析修改内容

根据以下规则分析修改类型：

**修改类型判断：**
- **feat (✨)**: 新增功能，关键词：`添加`, `新增`, `feat`, `feature`
- **fix (🐛)**: 修复问题，关键词：`修复`, `fix`, `bug`, `错误`
- **refactor (♻️)**: 重构代码，关键词：`重构`, `refactor`, `优化`
- **docs (📝)**: 文档修改，关键词：`文档`, `doc`, `README`
- **test (✅)**: 测试相关，关键词：`测试`, `test`
- **chore (🔧)**: 配置/构建，关键词：`配置`, `依赖`, `chore`

**涉及的模块识别：**
- 从文件路径提取主要模块名称（如 `ui/`, `core/` 等）
- 如果涉及多个模块，取前 2 个

### 步骤 4：生成提交信息

根据分析结果生成提交信息，格式：

```
<emoji> <type>: <模块>相关修改 - <简短描述>

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

示例：
- `✨ feat: HomeScreen相关修改 - 新增占卜计算功能`
- `🐛 fix: grid组件修复 - 修复九宫格显示问题`
- `📝 docs: README更新 - 更新项目文档`

### 步骤 5：执行提交

```bash
# 添加所有修改
git add .

# 创建提交（使用生成的提交信息）
git commit -m "<提交信息>"

# 推送到远程仓库
git push
```

## 示例

假设修改了 `ui/screens/HomeScreen.kt` 和 `core/components/grid/JiuGongGrid.kt`，添加了新的占卜计算功能：

1. 查看 git status 确认修改
2. 查看 git diff 了解具体改动
3. 分析：新增功能 → feat，涉及 HomeScreen 和 grid 模块
4. 生成提交信息：`✨ feat: HomeScreen、grid相关修改 - 新增占卜计算功能`
5. 执行 git add, git commit, git push

## 注意事项

- 如果没有修改，提示用户"没有需要提交的代码"
- 如果推送失败，说明代码已提交到本地，可以稍后手动推送
- 提交信息应该简洁明了，突出主要改动
- 始终包含 Co-Authored-By 标记
