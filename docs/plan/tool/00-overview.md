# 工具重构总体计划

## 概述

本计划旨在将 OpenJax 的 `read_file`、`list_dir`、`grep_files` 三个工具重构到与 Codex 相同的功能水平。

## 目标

- ✅ 达到与 Codex 相同的功能水平
- ✅ 保持代码简洁和可维护性
- ✅ 复用 Codex 的成熟实现（如果可行）
- ✅ 保持与 OpenJax 现有架构的兼容性

## 重构范围

| 工具 | 当前状态 | 目标状态 | 优先级 |
|-----|---------|---------|--------|
| `grep_files` | 基础实现 | 使用 ripgrep，支持正则、glob、分页 | P0 |
| `read_file` | 基础实现 | 支持分页、行号、缩进感知 | P0 |
| `list_dir` | 基础实现 | 支持递归、分页、文件类型标记 | P0 |

## 架构决策

### 方案 A：直接复用 Codex 代码

**优点：**
- 成熟稳定，经过充分测试
- 功能完整
- 维护成本低

**缺点：**
- 依赖 Codex 的协议类型
- 需要适配 OpenJax 的架构
- 可能引入不必要的依赖

### 方案 B：参考实现，独立开发

**优点：**
- 完全控制代码
- 依赖最小化
- 更好地适配 OpenJax 架构

**缺点：**
- 开发时间较长
- 需要充分测试

### 决策：采用混合方案

- **grep_files**：直接使用 ripgrep（参考 Codex 实现）
- **read_file**：参考 Codex 实现，适配 OpenJax 架构
- **list_dir**：参考 Codex 实现，适配 OpenJax 架构

## 依赖需求

### 新增依赖

```toml
[dependencies]
# 用于 grep_files
# ripgrep 通过系统调用，无需额外依赖

# 用于 read_file 和 list_dir
# 无需新增依赖
```

### 可选依赖（后续优化）

```toml
[dependencies]
# 用于工具注册和编排（后续）
async-trait = { workspace = true }  # 已有
```

## 实施阶段

### 阶段 1：grep_files（P0）

**目标：** 使用 ripgrep 替代 walkdir，大幅提升性能

**工作量：** 1-2 天

**交付物：**
- ✅ 使用 ripgrep 的 grep_files 实现
- ✅ 支持正则表达式
- ✅ 支持 glob 模式过滤
- ✅ 支持分页（limit 参数）
- ✅ 支持超时控制（30 秒）
- ✅ 完整的单元测试

### 阶段 2：read_file（P0）

**目标：** 支持分页读取和行号显示

**工作量：** 2-3 天

**交付物：**
- ✅ 支持 offset 和 limit 参数
- ✅ 显示行号
- ✅ 超长行截断（500 字符）
- ✅ CRLF 处理
- ✅ 非 UTF8 字符处理
- ✅ 完整的单元测试

**可选（后续优化）：**
- ⏸️ 缩进感知模式
- ⏸️ 注释识别

### 阶段 3：list_dir（P0）

**目标：** 支持递归列出和文件类型标记

**工作量：** 2-3 天

**交付物：**
- ✅ 支持 depth 参数（递归深度）
- ✅ 支持 offset 和 limit 参数（分页）
- ✅ 文件类型标记（`/` 目录、`@` 符号链接）
- ✅ 缩进显示层级
- ✅ 超长条目名截断（500 字符）
- ✅ 显示绝对路径
- ✅ 超过限制时提示
- ✅ 完整的单元测试

### 阶段 4：集成测试（P0）

**目标：** 确保所有工具正常工作

**工作量：** 1 天

**交付物：**
- ✅ 端到端集成测试
- ✅ 性能基准测试
- ✅ 文档更新

## 风险与缓解

### 风险 1：ripgrep 不可用

**影响：** grep_files 无法工作

**缓解：**
- 检测 ripgrep 是否安装
- 提供友好的错误提示
- 考虑回退到 walkdir 实现（性能较差）

### 风险 2：路径验证逻辑不一致

**影响：** 安全漏洞或路径逃逸

**缓解：**
- 复用 OpenJax 现有的路径验证逻辑
- 充分的单元测试
- 安全审查

### 风险 3：性能回归

**影响：** 用户体验下降

**缓解：**
- 性能基准测试
- 对比 Codex 性能
- 优化关键路径

## 成功标准

- ✅ 所有工具通过单元测试
- ✅ 所有工具通过集成测试
- ✅ grep_files 性能提升 10 倍以上
- ✅ read_file 支持分页读取
- ✅ list_dir 支持递归列出
- ✅ 文档完整更新
- ✅ 代码审查通过

## 后续优化

### 短期（1-2 周）

1. 添加 read_file 缩进感知模式
2. 添加 read_file 注释识别
3. 添加工具遥测支持
4. 添加钩子支持

### 中期（1-2 月）

1. 重构 exec_command（参考 Codex）
2. 添加 ToolRegistry
3. 添加 ToolOrchestrator
4. 添加 ToolRuntime 抽象

### 长期（3-6 月）

1. 支持动态工具加载
2. 支持 MCP 工具
3. 支持工具插件系统

## 参考资料

- [Codex Tool System](/Users/ericw/work/code/ai/codex/docs/tool-system.md)
- [Codex vs OpenJax 工具对比](/Users/ericw/work/code/ai/openJax/docs/tools-comparison.md)
- [OpenJax 工具文档](/Users/ericw/work/code/ai/openJax/docs/tools.md)

## 附录

### A. 工具参数对比

详见各工具的具体实施计划。

### B. 测试计划

详见各工具的具体实施计划。

### C. 性能基准

详见各工具的具体实施计划。
