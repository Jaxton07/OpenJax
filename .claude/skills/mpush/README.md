# mpush Skill

智能代码提交技能，帮助你自动化 Git 提交流程。

## 使用方法

在 Claude Code 中运行：

```
/mpush
```

或者：

```
请使用 mpush 技能提交代码
```

## 功能说明

mpush 会自动执行以下步骤：

1. ✅ 查看当前 Git 仓库状态
2. ✅ 分析所有修改的文件
3. ✅ 查看代码差异内容
4. ✅ 智能识别修改类型（feat/fix/refactor/docs/test/chore）
5. ✅ 识别涉及的功能模块
6. ✅ 生成简洁的中文提交信息
7. ✅ 添加所有修改到暂存区
8. ✅ 创建 Git 提交
9. ✅ 推送到远程仓库

## 提交信息格式

```
<emoji> <type>: <模块>相关修改 - <简短描述>

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

### 支持的提交类型

| 类型 | Emoji | 描述 | 关键词 |
|------|-------|------|--------|
| feat | ✨ | 新增功能 | 添加、新增、feat |
| fix | 🐛 | 修复问题 | 修复、fix、bug |
| refactor | ♻️ | 重构代码 | 重构、refactor、优化 |
| docs | 📝 | 文档修改 | 文档、doc、README |
| test | ✅ | 测试相关 | 测试、test |
| chore | 🔧 | 配置/构建 | 配置、依赖、chore |

## 示例输出

```
📊 正在分析代码变更...

📝 检测到 2 个文件修改：
   M ui/screens/HomeScreen.kt
   M core/components/grid/JiuGongGrid.kt

🔍 正在分析代码差异...

💬 提交信息：✨ feat: HomeScreen、grid相关修改 - 新增占卜计算功能

➕ 正在添加文件到暂存区...
✅ 文件已添加

🎯 正在提交代码...
✅ 提交成功

🚀 正在推送到远程仓库 (main 分支)...
✅ 推送成功！
```

## 工作原理

mpush 通过以下方式智能分析代码变更：

1. 使用 `git status` 获取修改的文件列表
2. 使用 `git diff` 获取详细的代码差异
3. 分析文件路径识别涉及的模块
4. 搜索 diff 内容中的关键词确定修改类型
5. 根据修改类型和模块生成规范的提交信息

## 注意事项

- 确保在 Git 仓库中运行
- 确保有代码变更需要提交
- 如果推送失败，代码仍会提交到本地仓库
- 提交前会显示所有修改的文件供确认
