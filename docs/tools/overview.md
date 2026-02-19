# OpenJax 工具系统概述

本文档提供了 OpenJax 工具系统的概述，包括核心特性和当前状态。

## 核心特性

OpenJax 工具系统是一个模块化、可扩展的工具框架，支持动态注册、统一接口、丰富的输出格式和完整的执行流程。该系统参考了 Codex 的架构设计，提供了与 Codex 一致的体验。

### 主要特性

- **统一的接口抽象**：所有工具实现相同的 `ToolHandler` trait
- **动态注册和分发**：支持运行时注册和分发工具
- **丰富的输出格式**：支持 Text 和 Json 输出，包含成功标志
- **工具规范定义**：JSON Schema 定义工具的输入和输出
- **Hooks 系统**：支持工具执行前后的钩子
- **集中的沙箱和批准管理**：统一的沙箱策略和批准流程
- **支持多种工具类型**：Function、Mcp、Custom、LocalShell
- **动态工具支持**：支持运行时注册自定义工具

## 当前状态

### 已完成的优化阶段

#### 第一阶段：引入核心抽象层 ✅
- ✅ 定义核心类型（context.rs）
- ✅ 定义 ToolHandler trait（registry.rs）
- ✅ 实现工具注册表（registry.rs）
- ✅ 迁移现有工具到 ToolHandler

#### 第二阶段：实现工具规范和输出格式 ✅
- ✅ 定义工具规范（spec.rs）
- ✅ 实现工具规范注册（tool_builder.rs）

#### 第三阶段：实现 Hooks 系统 ✅
- ✅ 定义 Hooks 类型（events.rs）
- ✅ 实现 Hooks 执行器（hooks.rs）

#### 第四阶段：集中管理沙箱和批准逻辑 ✅
- ✅ 定义沙箱策略（sandboxing.rs）
- ✅ 实现工具编排器（orchestrator.rs）

#### 第五阶段：支持动态注册和扩展性 ✅
- ✅ 实现动态工具支持（dynamic.rs）
- ✅ 更新工具路由器（router_impl.rs）

### 编译状态

✅ **编译成功**，无警告无错误：
```bash
cargo build -p openjax-core
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.17s
```

## 系统总结

OpenJax 工具系统已经完成了全面的优化，达到了与 Codex 一致的架构水平。系统具备：

- ✅ 统一的接口抽象（ToolHandler trait）
- ✅ 动态注册和分发能力（ToolRegistry）
- ✅ 丰富的输出格式（Text、Json）
- ✅ 工具规范定义（JSON Schema）
- ✅ Hooks 系统（BeforeToolUse、AfterToolUse）
- ✅ 集中的沙箱和批准管理（ToolOrchestrator）
- ✅ 支持多种工具类型（Function、Mcp、Custom、LocalShell）
- ✅ 动态工具支持（DynamicToolManager）
- ✅ 清晰的模块化架构

这个系统为 OpenJax 的后续扩展打下了坚实的基础，支持快速添加新工具、集成外部服务和实现复杂的工具链。

## 相关文档

- [架构设计](architecture.md) - 了解工具系统的架构设计
- [核心组件](core-components.md) - 深入了解核心组件
- [工具列表](tools-list.md) - 查看所有可用工具
- [使用指南](usage-guide.md) - 学习如何使用工具系统
- [扩展指南](extension-guide.md) - 学习如何扩展新工具
