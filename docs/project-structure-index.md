## 项目结构索引

### 总览
- **工作区**: `openJax`
- **核心包**: `openjax-protocol/`, `openjax-core/`, `openjax-cli/`
- **辅助**: `smoke_test/`（冒烟测试用例）
- **文档**: `docs/`

### 根目录文件
- **`Cargo.toml`**: 工作区级别依赖与成员配置
- **`Cargo.lock`**: 依赖锁定文件
- **`README.md`**: 项目简介与使用说明
- **`CLAUDE.md`**: 本仓库工作指南与约定
- **`test.txt`**: 临时测试文件

### 子项目与源码
- **`openjax-protocol/`**: 协议类型与共享数据结构
  - **`src/lib.rs`**: 协议类型定义入口
- **`openjax-core/`**: 代理编排、工具与模型客户端
  - **`src/lib.rs`**: 核心库入口与代理流程
  - **`src/model.rs`**: 模型客户端实现
  - **`src/tools.rs`**: 工具路由器与执行逻辑
  - **`src/config.rs`**: 配置结构与解析
  - **`tests/`**: 核心模块测试（`m3_sandbox.rs`, `m4_apply_patch.rs`）
- **`openjax-cli/`**: CLI 入口与交互显示
  - **`src/main.rs`**: CLI 入口
  - **`tests/e2e_cli.rs`**: CLI 端到端测试
  - **`config.toml.example`**: 配置示例


### 测试与构建产物
- **`smoke_test/`**: 冒烟测试项目
  - **`src/main.rs`**: 测试入口
- **`target/`**, **`smoke_test/target/`**: 构建产物目录（可忽略）
