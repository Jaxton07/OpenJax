# OpenJax 工具系统

OpenJax 工具系统的详细文档已迁移到 [docs/tools/](../../docs/tools/README.md) 目录。

## 快速链接

- [工具系统文档首页](../../docs/tools/README.md)
- [概述](../../docs/tools/overview.md) - 工具系统的核心特性和当前状态
- [架构设计](../../docs/tools/architecture.md) - 工具系统的架构设计
- [工具列表](../../docs/tools/tools-list.md) - 所有可用工具的详细说明
- [使用指南](../../docs/tools/usage-guide.md) - 如何使用工具系统

## 文档目录

详细的文档已拆分为以下部分：

### 核心文档
- [概述](../../docs/tools/overview.md)
- [架构设计](../../docs/tools/architecture.md)
- [核心组件](../../docs/tools/core-components.md)

### 使用文档
- [工具列表](../../docs/tools/tools-list.md)
- [使用指南](../../docs/tools/usage-guide.md)
- [最佳实践](../../docs/tools/best-practices.md)
- [故障排除](../../docs/tools/troubleshooting.md)

### 扩展文档
- [扩展指南](../../docs/tools/extension-guide.md)
- [Hooks 系统](../../docs/tools/hooks-system.md)
- [动态工具](../../docs/tools/dynamic-tools.md)

### 安全文档
- [沙箱和批准](../../docs/tools/sandbox-and-approval.md)

### 参考文档
- [参考资源](../../docs/tools/references.md)

## 快速开始

### 搜索代码
```bash
tool:grep_files pattern=fn main path=src include=*.rs
```

### 读取文件
```bash
tool:read_file file_path=src/lib.rs offset=1 limit=50
```

### 列出目录
```bash
tool:list_dir dir_path=src depth=2
```

### 执行命令
```bash
tool:shell cmd='cargo test' require_escalated=true
```

### 应用补丁
```bash
tool:apply_patch patch='*** Begin Patch\n*** Add File: new.rs\n+// content\n*** End Patch'
```

## 系统特性

- **统一的接口抽象**：所有工具实现相同的 `ToolHandler` trait
- **动态注册和分发**：支持运行时注册和分发工具
- **丰富的输出格式**：支持 Text 和 Json 输出，包含成功标志
- **工具规范定义**：JSON Schema 定义工具的输入和输出
- **Hooks 系统**：支持工具执行前后的钩子
- **集中的沙箱和批准管理**：统一的沙箱策略和批准流程
- **支持多种工具类型**：Function、Mcp、Custom、LocalShell
- **动态工具支持**：支持运行时注册自定义工具

## 获取帮助

如需更多详细信息，请访问 [docs/tools/](../../docs/tools/README.md) 查看完整文档。
