# OpenJax 工具系统优化计划

> 创建时间：2026-03-27
> 基于：Claude Code 工具能力横向对比分析

---

## 背景与动机

对 OpenJax 现有工具系统与 Claude Code 工具能力进行横向对比，识别出以下四类问题：

1. **参数类型约束错误**：规划提示词要求"所有参数必须是 JSON 字符串"，但实际上只有 `edit_file_range` 实现了兼容字符串的 `de_usize` 反序列化器，`read_file`/`list_dir`/`grep_files` 直接反序列化 `usize`，传字符串会报错。提示词规则与代码实现不一致。
2. **工具列表硬编码**：工具名枚举字符串在 `prompt.rs` 两处手写，每次新增工具需手动同步，容易遗漏。
3. **Shell 输出冗余**：模型接收到 10 个字段（含 backend、degrade_reason 等内部调试字段），浪费 token 且干扰模型理解。
4. **工具种类缺口**：缺少 `write_file`（创建文件）和 `glob`（路径模式搜索）专用工具。

---

## 优化项总览

| 编号 | 类别 | 优先级 | 核心改动 |
|------|------|--------|---------|
| P1 | 修复参数类型约束 | 🔴 高 | 提取 de_usize/de_u64 为公共模块，全工具覆盖；删除错误提示词规则 |
| P2 | 工具列表动态化 | 🔴 高 | ToolRouter 新增 tool_names()，prompt.rs 动态注入 |
| P3 | Shell 输出精简 | 🟡 中 | 只暴露 exit_code/stdout/stderr 给模型 |
| P4 | 新增 write_file 工具 | 🟡 中 | 简单创建/覆盖文件 |
| P5 | 工具描述归位 | 🟡 中 | 格式说明从 prompt.rs 移至 spec.rs description |
| P6 | 新增 glob 工具 | 🟢 低 | 路径模式搜索（补齐与 Claude Code 的工具差距） |
| P7 | 主动先读后写约束 | 🟢 低 | prompt.rs 追加一行规则 |

---

## P1：修复参数类型约束

### 问题根因

`openjax-core/src/tools/handlers/edit_file_range.rs` L11-40 中的 `de_usize` 通过 `#[serde(untagged)]` 枚举同时接受数字和字符串：

```rust
fn de_usize<'de, D>(deserializer: D) -> Result<usize, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrString { Num(usize), Str(String) }
    match NumOrString::deserialize(deserializer)? {
        NumOrString::Num(n) => Ok(n),
        NumOrString::Str(s) => s.trim().parse::<usize>().map_err(|_| ...),
    }
}
```

但 `read_file`、`list_dir`、`grep_files`、`shell` 的数值参数直接定义为 `usize`/`u64`，只接受 JSON 数字。
`prompt.rs` L77 的 `"All values inside args MUST be JSON strings"` 规则与代码实现矛盾。

### 修改方案

**Step 1**：新建公共反序列化模块

```
openjax-core/src/tools/handlers/de_helpers.rs
```

```rust
pub fn de_usize<'de, D: Deserializer<'de>>(d: D) -> Result<usize, D::Error> { ... }
pub fn de_u64<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> { ... }
```

**Step 2**：为所有数值参数 handler 应用双向兼容反序列化

| 文件 | 参数 | 类型 |
|------|------|------|
| `handlers/read_file.rs` | offset, limit | de_usize |
| `handlers/list_dir.rs` | offset, limit, depth | de_usize |
| `handlers/grep_files.rs` | limit | de_usize |
| `handlers/shell.rs` | timeout_ms | de_u64 |
| `system/process_snapshot.rs` | limit | de_usize |

**Step 3**：删除 `prompt.rs` L77 错误约束

```diff
- - IMPORTANT: All values inside args MUST be JSON strings (not numbers/booleans). Example: \"start_line\":\"6\".
```

### 涉及文件

- `openjax-core/src/tools/handlers/de_helpers.rs`（新建）
- `openjax-core/src/tools/handlers/mod.rs`（pub mod de_helpers）
- `openjax-core/src/tools/handlers/read_file.rs`
- `openjax-core/src/tools/handlers/list_dir.rs`
- `openjax-core/src/tools/handlers/grep_files.rs`
- `openjax-core/src/tools/handlers/shell.rs`
- `openjax-core/src/tools/system/process_snapshot.rs`
- `openjax-core/src/agent/prompt.rs`（删除 L77）

---

## P2：工具列表动态化

### 问题根因

```
// prompt.rs L67（规划提示词）
"tool\":\"read_file|list_dir|grep_files|process_snapshot|system_load|disk_usage|shell|edit_file_range\""

// prompt.rs L128（JSON 修复提示词）
// 同一份列表再次硬编码
```

`ToolRouter.specs` 字段私有，无法从外部枚举已注册工具名。

### 修改方案

**Step 1**：`ToolRouter` 添加公开访问器

文件：`openjax-core/src/tools/router_impl.rs`

```rust
pub fn tool_names(&self) -> Vec<&str> {
    self.specs.iter().map(|s| s.name.as_str()).collect()
}
```

**Step 2**：`build_planner_input` 新增参数

文件：`openjax-core/src/agent/prompt.rs`

```rust
pub(crate) fn build_planner_input(
    ...
    loop_recovery: Option<&str>,
    available_tools: &[&str],    // 新增
) -> String {
    let tool_enum = available_tools.join("|");
    // format! 中用 {tool_enum} 替换硬编码字符串
}
```

`build_json_repair_prompt` 同步修改，接收相同参数。

**Step 3**：调用处传入动态列表

文件：`openjax-core/src/agent/planner.rs`

```rust
let tool_names = self.tool_router.tool_names();
let prompt = build_planner_input(..., &tool_names);
```

### 涉及文件

- `openjax-core/src/tools/router_impl.rs`（新增 `tool_names()`）
- `openjax-core/src/agent/prompt.rs`（L67、L128 动态化）
- `openjax-core/src/agent/planner.rs`（传入 tool_names）

---

## P3：Shell 输出精简

### 问题根因

`openjax-core/src/sandbox/mod.rs` L232-244 的输出格式：

```
result_class=Success
command=ls -la
exit_code=0
backend=NoneEscalated
degrade_reason=none
policy_decision=Allow
runtime_allowed=true
runtime_deny_reason=none
stdout:
...
stderr:
```

模型只需要 `exit_code`、`stdout`、`stderr`，其他 7 个字段是内部调试信息。

### 修改方案

```rust
// openjax-core/src/sandbox/mod.rs
let model_output = if output.exit_code == 0 {
    format!("exit_code={}\nstdout:\n{}", output.exit_code, output.stdout)
} else {
    format!(
        "exit_code={}\nstdout:\n{}\nstderr:\n{}",
        output.exit_code, output.stdout, output.stderr
    )
};
```

内部字段（result_class, backend 等）保留在事件结构体中，通过 `ToolCallCompleted` 事件流向 TUI/Web，不写入 model_output。

**前置确认**：检查 TUI/Web 是否直接解析 `model_output` 字符串中的 `result_class` 字段，若有需同步修改消费方。

### 涉及文件

- `openjax-core/src/sandbox/mod.rs`（execute_shell，L232-244）

---

## P4：新增 write_file 工具

### 问题根因

当前创建文件需要专门的文件写入工具，对模型认知成本高。

### 工具定义

**名称**：`write_file`

**参数 Schema**：
```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "File path relative to workspace root"
    },
    "content": {
      "type": "string",
      "description": "Full file content to write. Overwrites if file exists."
    }
  },
  "required": ["file_path", "content"]
}
```

**行为**：
- 路径验证：不允许逃逸工作区根目录
- 父目录不存在时自动创建
- 文件已存在时直接覆盖
- 输出：`written <file_path> (<n> bytes)`

### 实现步骤

1. `openjax-core/src/tools/handlers/write_file.rs`（新建 handler）
2. `openjax-core/src/tools/handlers/mod.rs`（pub use WriteFileHandler）
3. `openjax-core/src/tools/spec.rs`（新增 `write_file_spec()`，加入 `build_all_specs()`）
4. `openjax-core/src/tools/tool_builder.rs`（注册 WriteFileHandler）

### 涉及文件

- `openjax-core/src/tools/handlers/write_file.rs`（新建）
- `openjax-core/src/tools/handlers/mod.rs`
- `openjax-core/src/tools/spec.rs`
- `openjax-core/src/tools/tool_builder.rs`

---

## P5：工具描述归位

### 问题根因

`prompt.rs` L83-L98 共 16 行工具格式细节嵌入全局规划提示词，增加 token 消耗并让规划提示词职责混乱（调度策略 vs 工具格式文档）。

### 修改方案

将格式细节从 `prompt.rs` 移至 `spec.rs` 的 description 字段末尾（追加 Format 部分）。

`prompt.rs` 保留简短调度策略：
```
- For multi-file edits or file operations, use appropriate tools.
- For single-file range edits with known line numbers, use edit_file_range.
```

### 涉及文件

- `openjax-core/src/tools/spec.rs`（description 追加格式说明）
- `openjax-core/src/agent/prompt.rs`（L83-98 精简为 3 行）

---

## P6：新增 glob 工具（低优先）

### 工具定义

**名称**：`glob_files`

**参数**：
- `pattern: String` — glob 模式（如 `src/**/*.rs`、`**/*.toml`）
- `base_path: Option<String>` — 基础目录，默认工作区根
- `limit: usize` — 最大结果数，默认 200

**与 grep_files 的区别**：
- `grep_files` 搜索**文件内容**（需要 pattern 在文件内容中匹配）
- `glob_files` 搜索**文件路径**（按路径通配符匹配，不读取内容）

**依赖**：`glob` crate（检查工作区是否已有依赖，若无需在 `openjax-core/Cargo.toml` 添加）

### 涉及文件

- `openjax-core/src/tools/handlers/glob_files.rs`（新建）
- `openjax-core/src/tools/handlers/mod.rs`
- `openjax-core/src/tools/spec.rs`
- `openjax-core/src/tools/tool_builder.rs`
- `openjax-core/Cargo.toml`（可能需要添加 glob 依赖）

---

## P7：主动先读后写约束（低优先）

在 `prompt.rs` Tool selection policy 部分追加：

```
- ALWAYS call read_file before edit_file_range (Update File) unless creating a brand-new file.
```

### 涉及文件

- `openjax-core/src/agent/prompt.rs`

---

## 执行顺序

```
P1 → P2 → P3 → P5 → P4 → P7 → P6
```

**理由**：
- P1/P2/P3/P5 仅修改现有文件，无新增代码，风险最低，优先消化
- P4 新增一个完整工具（handler + spec + registration），独立一步
- P7 一行提示词修改，最轻量
- P6 可能涉及新增 crate 依赖，影响范围最广，放最后

---

## 验证方案

### P1 验证
```bash
# 全量工具沙箱测试（覆盖参数类型修复）
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"

# 新增测试：用字符串数字参数调用 read_file/list_dir
# 在 openjax-core/tests/tools_sandbox/ 下新增 m_string_args_compat.rs
```

### P2 验证
```bash
# 构建验证（无硬编码工具名遗留）
zsh -lc "cargo build -p openjax-core"

# 运行时验证：新增/禁用一个工具后，打印提示词检查工具枚举是否自动更新
```

### P3 验证
```bash
# 执行 shell 工具，检查 model_output 中不含 backend/degrade_reason 等字段
zsh -lc "cargo test -p openjax-core --test tools_sandbox_suite"
```

### P4 验证
```bash
zsh -lc "cargo test -p openjax-core"

# 新增集成测试（openjax-core/tests/tools_sandbox/m_write_file.rs）：
# - 新建文件
# - 覆盖已有文件
# - 路径逃逸被拒绝（../../../etc/passwd）
# - 自动创建父目录
```

### 全量回归
```bash
zsh -lc "cargo test --workspace"
zsh -lc "cargo clippy --workspace --all-targets -- -D warnings"
```
