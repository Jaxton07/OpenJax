# list_dir 重构实施计划

## 目标

将 `list_dir` 从简单的目录列表重构为支持递归、分页、文件类型标记的强大工具，达到 Codex 的功能水平。

## 当前实现

**位置：** [openjax-core/src/tools.rs:267-286](../../openjax-core/src/tools.rs#L267-L286)

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `path` | string | `.` | 相对路径 |

**特性：**
- ✅ 列出当前目录（不递归）
- ✅ 按字母排序
- ❌ 无递归支持
- ❌ 无分页支持
- ❌ 无文件类型标记
- ❌ 无缩进显示
- ❌ 无超长条目名截断

**输出格式：**
```
child.txt
deeper
grandchild.txt
nested
root.txt
```

## 目标实现

**参考：** [codex-rs/core/src/tools/handlers/list_dir.rs](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/list_dir.rs)

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `dir_path` | string | - | 相对路径 |
| `offset` | number | 1 | 1-indexed 起始条目 |
| `limit` | number | 25 | 最大条目数 |
| `depth` | number | 2 | 递归深度 |

**特性：**
- ✅ 递归列出目录（depth 控制）
- ✅ 分页支持（offset + limit）
- ✅ 按名称排序
- ✅ 文件类型标记（`/` 目录、`@` 符号链接）
- ✅ 缩进显示层级
- ✅ 超长条目名截断（500 字符）
- ✅ 显示绝对路径
- ✅ 超过限制时提示

**输出格式：**
```
Absolute path: /path/to/dir
nested/
  child.txt
  deeper/
    grandchild.txt
root.txt
```

**文件类型标记：**
- `/` - 目录
- `@` - 符号链接
- `?` - 其他类型
- 无后缀 - 普通文件

## 实施步骤

### 步骤 1：添加参数结构

**任务：**
- 定义 `ListDirArgs` 结构
- 添加默认值

**代码：**
```rust
const MAX_ENTRY_LENGTH: usize = 500;
const INDENTATION_SPACES: usize = 2;

fn default_offset() -> usize { 1 }
fn default_limit() -> usize { 25 }
fn default_depth() -> usize { 2 }

#[derive(Deserialize)]
struct ListDirArgs {
    dir_path: String,
    #[serde(default = "default_offset")]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_depth")]
    depth: usize,
}
```

### 步骤 2：实现参数验证

**任务：**
- 验证 `offset` 大于 0
- 验证 `limit` 大于 0
- 验证 `depth` 大于 0
- 验证路径在工作区内

**代码：**
```rust
if offset == 0 {
    return Err(anyhow!("offset must be a 1-indexed entry number"));
}

if limit == 0 {
    return Err(anyhow!("limit must be greater than zero"));
}

if depth == 0 {
    return Err(anyhow!("depth must be greater than zero"));
}

let path = resolve_workspace_path(cwd, &dir_path)?;
```

### 步骤 3：实现目录条目收集

**任务：**
- 使用广度优先搜索（BFS）遍历目录
- 根据 depth 控制递归深度
- 收集所有条目
- 按名称排序

**代码：**
```rust
#[derive(Clone)]
struct DirEntry {
    name: String,
    display_name: String,
    depth: usize,
    kind: DirEntryKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<&FileType> for DirEntryKind {
    fn from(file_type: &FileType) -> Self {
        if file_type.is_symlink() {
            DirEntryKind::Symlink
        } else if file_type.is_dir() {
            DirEntryKind::Directory
        } else if file_type.is_file() {
            DirEntryKind::File
        } else {
            DirEntryKind::Other
        }
    }
}

async fn collect_entries(
    dir_path: &Path,
    relative_prefix: &Path,
    depth: usize,
    entries: &mut Vec<DirEntry>,
) -> Result<()> {
    let mut queue = VecDeque::new();
    queue.push_back((dir_path.to_path_buf(), relative_prefix.to_path_buf(), depth));

    while let Some((current_dir, prefix, remaining_depth)) = queue.pop_front() {
        let mut read_dir = tokio::fs::read_dir(&current_dir).await
            .map_err(|err| anyhow!("failed to read directory: {err}"))?;

        let mut dir_entries = Vec::new();

        while let Some(entry) = read_dir.next_entry().await
            .map_err(|err| anyhow!("failed to read directory: {err}"))?
        {
            let file_type = entry.file_type().await
                .map_err(|err| anyhow!("failed to inspect entry: {err}"))?;

            let file_name = entry.file_name();
            let relative_path = if prefix.as_os_str().is_empty() {
                PathBuf::from(&file_name)
            } else {
                prefix.join(&file_name)
            };

            let display_name = format_entry_component(&file_name);
            let display_depth = prefix.components().count();
            let sort_key = format_entry_name(&relative_path);
            let kind = DirEntryKind::from(&file_type);
            dir_entries.push((
                entry.path(),
                relative_path,
                kind,
                DirEntry {
                    name: sort_key,
                    display_name,
                    depth: display_depth,
                    kind,
                },
            ));
        }

        dir_entries.sort_unstable_by(|a, b| a.3.name.cmp(&b.3.name));

        for (entry_path, relative_path, kind, dir_entry) in dir_entries {
            if kind == DirEntryKind::Directory && remaining_depth > 1 {
                queue.push_back((entry_path, relative_path, remaining_depth - 1));
            }
            entries.push(dir_entry);
        }
    }

    Ok(())
}
```

### 步骤 4：实现分页和格式化

**任务：**
- 应用 offset 和 limit
- 格式化条目行（缩进 + 文件类型标记）
- 截断超长条目名
- 添加超过限制提示

**代码：**
```rust
async fn list_dir_slice(
    path: &Path,
    offset: usize,
    limit: usize,
    depth: usize,
) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    collect_entries(path, Path::new(""), depth, &mut entries).await?;

    if entries.is_empty() {
        return Ok(Vec::new());
    }

    entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    let start_index = offset - 1;
    if start_index >= entries.len() {
        return Err(anyhow!("offset exceeds directory entry count"));
    }

    let remaining_entries = entries.len() - start_index;
    let capped_limit = limit.min(remaining_entries);
    let end_index = start_index + capped_limit;
    let selected_entries = &entries[start_index..end_index];
    let mut formatted = Vec::with_capacity(selected_entries.len());

    for entry in selected_entries {
        formatted.push(format_entry_line(entry));
    }

    if end_index < entries.len() {
        formatted.push(format!("More than {capped_limit} entries found"));
    }

    Ok(formatted)
}

fn format_entry_name(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace("\\", "/");
    if normalized.len() > MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized
    }
}

fn format_entry_component(name: &OsStr) -> String {
    let normalized = name.to_string_lossy();
    if normalized.len() > MAX_ENTRY_LENGTH {
        take_bytes_at_char_boundary(&normalized, MAX_ENTRY_LENGTH).to_string()
    } else {
        normalized.to_string()
    }
}

fn format_entry_line(entry: &DirEntry) -> String {
    let indent = " ".repeat(entry.depth * INDENTATION_SPACES);
    let mut name = entry.display_name.clone();
    match entry.kind {
        DirEntryKind::Directory => name.push('/'),
        DirEntryKind::Symlink => name.push('@'),
        DirEntryKind::Other => name.push('?'),
        DirEntryKind::File => {}
    }
    format!("{indent}{name}")
}
```

### 步骤 5：集成到 ToolRouter

**任务：**
- 更新 `list_dir` 函数签名
- 解析 JSON 参数
- 调用新的实现
- 返回格式化的结果

**代码：**
```rust
async fn list_dir(call: &ToolCall, cwd: &Path) -> Result<String> {
    let arguments = call.args.get("arguments")
        .ok_or_else(|| anyhow!("list_dir requires arguments"))?;

    let args: ListDirArgs = serde_json::from_str(arguments)
        .map_err(|err| anyhow!("failed to parse arguments: {err}"))?;

    let ListDirArgs {
        dir_path,
        offset,
        limit,
        depth,
    } = args;

    // 验证
    if offset == 0 {
        return Err(anyhow!("offset must be a 1-indexed entry number"));
    }

    if limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    if depth == 0 {
        return Err(anyhow!("depth must be greater than zero"));
    }

    let path = resolve_workspace_path(cwd, &dir_path)?;

    let entries = list_dir_slice(&path, offset, limit, depth).await?;

    let mut output = Vec::with_capacity(entries.len() + 1);
    output.push(format!("Absolute path: {}", path.display()));
    output.extend(entries);

    Ok(output.join("\n"))
}
```

### 步骤 6：编写单元测试

**任务：**
- 测试基本列出
- 测试递归列出
- 测试 depth 参数
- 测试 offset 和 limit 参数
- 测试文件类型标记
- 测试超长条目名截断
- 测试超过限制提示
- 测试 offset 超出条目数

**代码：**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn lists_directory_entries() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();

        let sub_dir = dir_path.join("nested");
        tokio::fs::create_dir(&sub_dir).await?;

        let deeper_dir = sub_dir.join("deeper");
        tokio::fs::create_dir(&deeper_dir).await?;

        tokio::fs::write(dir_path.join("entry.txt"), b"content").await?;
        tokio::fs::write(sub_dir.join("child.txt"), b"child").await?;
        tokio::fs::write(deeper_dir.join("grandchild.txt"), b"grandchild").await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link_path = dir_path.join("link");
            symlink(dir_path.join("entry.txt"), &link_path)?;
        }

        let entries = list_dir_slice(dir_path, 1, 20, 3).await?;

        #[cfg(unix)]
        let expected = vec![
            "entry.txt".to_string(),
            "link@".to_string(),
            "nested/".to_string(),
            "  child.txt".to_string(),
            "  deeper/".to_string(),
            "    grandchild.txt".to_string(),
        ];

        #[cfg(not(unix))]
        let expected = vec![
            "entry.txt".to_string(),
            "nested/".to_string(),
            "  child.txt".to_string(),
            "  deeper/".to_string(),
            "    grandchild.txt".to_string(),
        ];

        assert_eq!(entries, expected);
        Ok(())
    }

    #[tokio::test]
    async fn errors_when_offset_exceeds_entries() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();
        tokio::fs::create_dir(dir_path.join("nested")).await?;

        let err = list_dir_slice(dir_path, 10, 1, 2)
            .await
            .expect_err("offset exceeds entries");
        assert_eq!(err.to_string(), "offset exceeds directory entry count");
        Ok(())
    }

    #[tokio::test]
    async fn respects_depth_parameter() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();
        let nested = dir_path.join("nested");
        let deeper = nested.join("deeper");
        tokio::fs::create_dir(&nested).await?;
        tokio::fs::create_dir(&deeper).await?;
        tokio::fs::write(dir_path.join("root.txt"), b"root").await?;
        tokio::fs::write(nested.join("child.txt"), b"child").await?;
        tokio::fs::write(deeper.join("grandchild.txt"), b"deep").await?;

        let entries_depth_one = list_dir_slice(dir_path, 1, 10, 1).await?;
        assert_eq!(
            entries_depth_one,
            vec!["nested/".to_string(), "root.txt".to_string()]
        );

        let entries_depth_two = list_dir_slice(dir_path, 1, 20, 2).await?;
        assert_eq!(
            entries_depth_two,
            vec![
                "nested/".to_string(),
                "  child.txt".to_string(),
                "  deeper/".to_string(),
                "root.txt".to_string()
            ]
        );

        let entries_depth_three = list_dir_slice(dir_path, 1, 30, 3).await?;
        assert_eq!(
            entries_depth_three,
            vec![
                "nested/".to_string(),
                "  child.txt".to_string(),
                "  deeper/".to_string(),
                "    grandchild.txt".to_string(),
                "root.txt".to_string()
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn paginates_in_sorted_order() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();

        let dir_a = dir_path.join("a");
        let dir_b = dir_path.join("b");
        tokio::fs::create_dir(&dir_a).await?;
        tokio::fs::create_dir(&dir_b).await?;

        tokio::fs::write(dir_a.join("a_child.txt"), b"a").await?;
        tokio::fs::write(dir_b.join("b_child.txt"), b"b").await?;

        let first_page = list_dir_slice(dir_path, 1, 2, 2).await?;
        assert_eq!(
            first_page,
            vec![
                "a/".to_string(),
                "  a_child.txt".to_string(),
                "More than 2 entries found".to_string()
            ]
        );

        let second_page = list_dir_slice(dir_path, 3, 2, 2).await?;
        assert_eq!(
            second_page,
            vec!["b/".to_string(), "  b_child.txt".to_string()]
        );

        Ok(())
    }

    #[tokio::test]
    async fn handles_large_limit_without_overflow() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();
        tokio::fs::write(dir_path.join("alpha.txt"), b"alpha").await?;
        tokio::fs::write(dir_path.join("beta.txt"), b"beta").await?;
        tokio::fs::write(dir_path.join("gamma.txt"), b"gamma").await?;

        let entries = list_dir_slice(dir_path, 2, usize::MAX, 1).await?;
        assert_eq!(
            entries,
            vec!["beta.txt".to_string(), "gamma.txt".to_string()]
        );

        Ok(())
    }

    #[tokio::test]
    async fn indicates_truncated_results() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();

        for idx in 0..40 {
            let file = dir_path.join(format!("file_{idx:02}.txt"));
            tokio::fs::write(file, b"content").await?;
        }

        let entries = list_dir_slice(dir_path, 1, 25, 1).await?;
        assert_eq!(entries.len(), 26);
        assert_eq!(
            entries.last(),
            Some(&"More than 25 entries found".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn truncation_respects_sorted_order() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir_path = temp.path();
        let nested = dir_path.join("nested");
        let deeper = nested.join("deeper");
        tokio::fs::create_dir(&nested).await?;
        tokio::fs::create_dir(&deeper).await?;
        tokio::fs::write(dir_path.join("root.txt"), b"root").await?;
        tokio::fs::write(nested.join("child.txt"), b"child").await?;
        tokio::fs::write(deeper.join("grandchild.txt"), b"deep").await?;

        let entries_depth_three = list_dir_slice(dir_path, 1, 3, 3).await?;
        assert_eq!(
            entries_depth_three,
            vec![
                "nested/".to_string(),
                "  child.txt".to_string(),
                "  deeper/".to_string(),
                "More than 3 entries found".to_string()
            ]
        );

        Ok(())
    }
}
```

### 步骤 7：更新文档

**任务：**
- 更新 [tools.md](../../tools.md) 中的 list_dir 文档
- 添加新参数说明
- 更新使用示例
- 添加文件类型标记说明

## 测试计划

### 单元测试

- ✅ 基本列出功能
- ✅ 递归列出（depth 参数）
- ✅ 分页支持（offset + limit）
- ✅ 文件类型标记
- ✅ 缩进显示
- ✅ 超长条目名截断
- ✅ 超过限制提示
- ✅ offset 超出条目数

### 集成测试

- ✅ 端到端列出流程
- ✅ 与其他工具的集成
- ✅ 错误处理

### 性能测试

- ✅ 大目录列出性能
- ✅ 深度递归性能
- ✅ 分页性能

## 风险与缓解

### 风险 1：递归深度过大

**影响：** 性能问题或栈溢出

**缓解：**
- 限制最大 depth（建议最大 10）
- 使用 BFS 而非 DFS
- 添加超时控制

**代码：**
```rust
const MAX_DEPTH: usize = 10;

if depth > MAX_DEPTH {
    return Err(anyhow!("depth cannot exceed {MAX_DEPTH}"));
}
```

### 风险 2：路径验证不一致

**影响：** 安全漏洞或路径逃逸

**缓解：**
- 复用 OpenJax 现有的路径验证逻辑
- 充分的单元测试
- 安全审查

### 风险 3：符号链接循环

**影响：** 无限循环

**缓解：**
- 使用 `FileType::is_symlink()` 检测符号链接
- 不递归进入符号链接目录
- 添加深度限制

## 交付物

- ✅ 支持 depth 参数（递归深度）
- ✅ 支持 offset 和 limit 参数（分页）
- ✅ 文件类型标记（`/` 目录、`@` 符号链接）
- ✅ 缩进显示层级
- ✅ 超长条目名截断（500 字符）
- ✅ 显示绝对路径
- ✅ 超过限制时提示
- ✅ 完整的单元测试
- ✅ 更新的文档

## 工作量估算

- 步骤 1：添加参数结构 - 0.5 天
- 步骤 2：实现参数验证 - 0.5 天
- 步骤 3：实现目录条目收集 - 1 天
- 步骤 4：实现分页和格式化 - 1 天
- 步骤 5：集成到 ToolRouter - 0.5 天
- 步骤 6：编写单元测试 - 1 天
- 步骤 7：更新文档 - 0.5 天

**总计：** 5-6 天

## 参考资料

- [Codex list_dir 实现](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/list_dir.rs)
- [OpenJax 工具对比](../../tools-comparison.md)
