# OpenJax TODO（按顺序执行）

## Step 1：接入真实模型调用（完成）

- 状态：已完成
- 已实现：
  - `ModelClient` 抽象
  - Chat Completions 客户端，优先支持 MiniMax：
    - `OPENJAX_MINIMAX_API_KEY`
    - `OPENJAX_MINIMAX_BASE_URL`（默认 `https://api.minimaxi.com/v1`）
    - `OPENJAX_MINIMAX_MODEL`（默认 `codex-MiniMax-M2.1`）
  - OpenAI 兼容后备客户端（如设置 `OPENAI_API_KEY`）
  - 无 API key 时自动 fallback 到 `EchoModelClient`
- 验收：通过（本地 fallback 模式已验证）

## Step 2：接入基础 tools + ToolRouter（完成）

- 状态：已完成
- 已实现：
  - `ToolRouter` 统一路由
  - `tool:` 命令解析（支持引号参数）
  - `read_file`
  - `list_dir`
  - `grep_files`
  - 工具事件：`ToolCallStarted` / `ToolCallCompleted`
- 验收：通过（3 个工具均已冒烟验证）

## Step 3：接入 exec_command + approval + sandbox（第一版，收尾中）

- 状态：部分完成（2026-02-13）
- 已实现：
  - `exec_command` 工具
  - 审批策略：
    - `OPENJAX_APPROVAL_POLICY=always_ask|on_request|never`
  - 沙箱模式：
    - `OPENJAX_SANDBOX_MODE=workspace_write|danger_full_access`
  - `workspace_write` 第一版拦截策略（网络/提权/高危命令关键字）
  - 工作区路径守卫（拒绝绝对路径、`..` 逃逸、符号链接逃逸）
  - `workspace_write` 下 `exec_command` 收紧（禁用 shell 操作符、命令白名单、路径参数校验）
- 已验证：
  - `pwd` / `ls -la` 在 `workspace_write` 下可执行
  - `curl` 在 `workspace_write` 下被拦截
  - `read_file path=/etc/hosts` 被拦截
  - `read_file path=../test.txt` 被拦截
  - `exec_command` 重定向写入（`>`）被拦截
- 自动化测试：
  - `openjax-core/src/tools.rs` 新增 7 个单元测试并通过
  - `openjax-core/tests/m3_sandbox.rs` 新增 5 个集成测试并通过
  - 覆盖点：绝对路径拦截、`..` 逃逸拦截、符号链接逃逸拦截、禁网拦截、shell 操作符拦截、父路径参数拦截、安全只读命令放行
- 遗留：
  - 缺少 CLI crate 端到端交互测试（当前已覆盖 core 单测+集成测试）
  - `exec_command` 沙箱仍是应用层策略，不是系统级沙箱

## Step 4：补丁写入能力与回合稳定性（已启动）

- 状态：进行中（2026-02-13）
- 已实现：
  - `apply_patch` 工具接入 `ToolRouter`
  - 支持 `Add File` / `Update File` / `Delete File`
  - 补丁解析与基础校验（`*** Begin Patch` / `*** End Patch`）
  - 写入路径安全校验（工作区内、拒绝绝对路径与 `..` 逃逸）
  - 失败回滚：中途失败时恢复已修改文件
  - `patch` 参数兼容 `\n` 转真实换行（便于 `tool:` 单行输入）
- 自动化测试：
  - `openjax-core/src/tools.rs` 新增 `apply_patch` 单元测试（add/update/escape）
  - `openjax-core/tests/m4_apply_patch.rs` 新增 3 个集成测试并通过
  - 覆盖点：成功应用、非法补丁不污染、中途失败回滚
- 遗留：
  - 暂未支持完整 `apply_patch` 语法子集（如 move/rename、更多高级 hunk 变体）
  - 仍需补充 CLI 层 `apply_patch` 端到端交互测试

## 本轮新增文件/核心改动

- `openjax-protocol/src/lib.rs`
- `openjax-core/src/model.rs`
- `openjax-core/src/tools.rs`
- `openjax-core/src/lib.rs`
- `openjax-cli/src/main.rs`
- `Cargo.toml`
- `openjax-core/Cargo.toml`

---

## 下一步建议（M4 -> M5）

1. 补齐 CLI 级最小 e2e 测试脚本（`apply_patch` + 审批交互 + 事件输出一致性）。
2. 扩展 `apply_patch` 语法覆盖面，并补相应失败回滚用例。
3. 把工具调用从 `tool:` 文本协议升级为结构化 tool call 协议。
4. 开始整理 M5 所需参数体系与 `config.toml`。
