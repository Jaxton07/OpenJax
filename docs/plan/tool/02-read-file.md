# read_file 重构实施计划

## 目标

将 `read_file` 从简单的文件读取重构为支持分页、行号显示的强大工具，达到 Codex 的功能水平。

## 当前实现

**位置：** [openjax-core/src/tools.rs:253-265](../../openjax-core/src/tools.rs#L253-L265)

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `path` | string | - | 相对路径 |

**特性：**
- ✅ 简单读取整个文件
- ✅ 路径验证（相对路径、禁止 `../`）
- ❌ 无分页支持
- ❌ 无缩进感知
- ❌ 无行号显示
- ❌ 无超长行截断

**输出格式：**
```
fn main() {
    println!("Hello");
}
```

## 目标实现

**参考：** [codex-rs/core/src/tools/handlers/read_file.rs](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/read_file.rs)

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `file_path` | string | - | 相对路径 |
| `offset` | number | 1 | 1-indexed 起始行号 |
| `limit` | number | 2000 | 最大行数 |
| `mode` | enum | `Slice` | `Slice` 或 `Indentation` |
| `indentation` | object | - | 缩进配置（Indentation 模式） |
| `indentation.anchor_line` | number | offset | 锚点行号 |
| `indentation.max_levels` | number | 0 | 最大缩进深度（0=无限制） |
| `indentation.include_siblings` | boolean | false | 是否包含同级块 |
| `indentation.include_header` | boolean | true | 是否包含头部注释 |
| `indentation.max_lines` | number | limit | 硬限制 |

**特性：**
- ✅ 分页读取（offset + limit）
- ✅ 缩进感知读取（Indentation 模式）
- ✅ 自动识别代码块边界
- ✅ 跳过空行
- ✅ 识别注释（`#`, `//`, `--`）
- ✅ 行号显示
- ✅ 超长行截断（500 字符）
- ✅ CRLF 处理
- ✅ 非 UTF8 字符处理

**输出格式：**
```
L1: fn main() {
L2:     println!("Hello");
L3: }
```

## 实施步骤

### 步骤 1：添加参数结构

**任务：**
- 定义 `ReadFileArgs` 结构
- 定义 `ReadMode` 枚举
- 定义 `IndentationArgs` 结构
- 添加默认值

**代码：**
```rust
const MAX_LINE_LENGTH: usize = 500;
const TAB_WIDTH: usize = 4;

const COMMENT_PREFIXES: &[&str] = &["#", "//", "--"];

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReadMode {
    #[default]
    Slice,
    Indentation,
}

#[derive(Deserialize, Clone)]
struct IndentationArgs {
    #[serde(default)]
    anchor_line: Option<usize>,
    #[serde(default = "defaults::max_levels")]
    max_levels: usize,
    #[serde(default = "defaults::include_siblings")]
    include_siblings: bool,
    #[serde(default = "defaults::include_header")]
    include_header: bool,
    #[serde(default)]
    max_lines: Option<usize>,
}

#[derive(Deserialize)]
struct ReadFileArgs {
    file_path: String,
    #[serde(default = "defaults::offset")]
    offset: usize,
    #[serde(default = "defaults::limit")]
    limit: usize,
    #[serde(default)]
    mode: ReadMode,
    #[serde(default)]
    indentation: Option<IndentationArgs>,
}

mod defaults {
    pub fn offset() -> usize { 1 }
    pub fn limit() -> usize { 2000 }
    pub fn max_levels() -> usize { 0 }
    pub fn include_siblings() -> bool { false }
    pub fn include_header() -> bool { true }
}
```

### 步骤 2：实现参数验证

**任务：**
- 验证 `offset` 大于 0
- 验证 `limit` 大于 0
- 验证路径在工作区内
- 验证 `anchor_line` 大于 0（如果提供）

**代码：**
```rust
if offset == 0 {
    return Err(anyhow!("offset must be a 1-indexed line number"));
}

if limit == 0 {
    return Err(anyhow!("limit must be greater than zero"));
}

let path = resolve_workspace_path(cwd, &file_path)?;

if let Some(anchor_line) = indentation.as_ref().and_then(|i| i.anchor_line) {
    if anchor_line == 0 {
        return Err(anyhow!("anchor_line must be a 1-indexed line number"));
    }
}
```

### 步骤 3：实现 Slice 模式

**任务：**
- 使用 `tokio::io::AsyncBufReadExt` 逐行读取
- 跳过 offset 之前的行
- 收集 limit 行
- 格式化行号
- 截断超长行

**代码：**
```rust
async fn read_slice(
    path: &Path,
    offset: usize,
    limit: usize,
) -> Result<Vec<String>> {
    let file = File::open(path).await
        .map_err(|err| anyhow!("failed to read file: {err}"))?;

    let mut reader = BufReader::new(file);
    let mut collected = Vec::new();
    let mut seen = 0usize;
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut buffer).await
            .map_err(|err| anyhow!("failed to read file: {err}"))?;

        if bytes_read == 0 {
            break;
        }

        // Remove newline characters
        if buffer.last() == Some(&b'\n') {
            buffer.pop();
            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }
        }

        seen += 1;

        if seen < offset {
            continue;
        }

        if collected.len() == limit {
            break;
        }

        let formatted = format_line(&buffer);
        collected.push(format!("L{seen}: {formatted}"));

        if collected.len() == limit {
            break;
        }
    }

    if seen < offset {
        return Err(anyhow!("offset exceeds file length"));
    }

    Ok(collected)
}

fn format_line(bytes: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(bytes);
    if decoded.len() > MAX_LINE_LENGTH {
        take_bytes_at_char_boundary(&decoded, MAX_LINE_LENGTH).to_string()
    } else {
        decoded.into_owned()
    }
}
```

### 步骤 4：实现 Indentation 模式（可选，后续优化）

**任务：**
- 读取整个文件
- 计算每行的有效缩进
- 从锚点行开始，向上和向下扩展
- 根据 `max_levels` 控制扩展深度
- 根据 `include_siblings` 控制是否包含同级块
- 根据 `include_header` 控制是否包含头部注释
- 跳过空行

**代码：**
```rust
#[derive(Clone, Debug)]
struct LineRecord {
    number: usize,
    raw: String,
    display: String,
    indent: usize,
}

impl LineRecord {
    fn trimmed(&self) -> &str {
        self.raw.trim_start()
    }

    fn is_blank(&self) -> bool {
        self.trimmed().is_empty()
    }

    fn is_comment(&self) -> bool {
        COMMENT_PREFIXES.iter().any(|prefix| self.raw.trim().starts_with(prefix))
    }
}

async fn read_indentation_block(
    path: &Path,
    offset: usize,
    limit: usize,
    options: IndentationArgs,
) -> Result<Vec<String>> {
    let anchor_line = options.anchor_line.unwrap_or(offset);
    let guard_limit = options.max_lines.unwrap_or(limit);

    let collected = collect_file_lines(path).await?;
    if collected.is_empty() || anchor_line > collected.len() {
        return Err(anyhow!("anchor_line exceeds file length"));
    }

    let anchor_index = anchor_line - 1;
    let effective_indents = compute_effective_indents(&collected);
    let anchor_indent = effective_indents[anchor_index];

    // Compute the min indent
    let min_indent = if options.max_levels == 0 {
        0
    } else {
        anchor_indent.saturating_sub(options.max_levels * TAB_WIDTH)
    };

    // Cap requested lines by guard_limit and file length
    let final_limit = limit.min(guard_limit).min(collected.len());

    if final_limit == 1 {
        return Ok(vec![format!(
            "L{}: {}",
            collected[anchor_index].number, collected[anchor_index].display
        )]);
    }

    // Cursors
    let mut i: isize = anchor_index as isize - 1; // up (inclusive)
    let mut j: usize = anchor_index + 1; // down (inclusive)
    let mut i_counter_min_indent = 0;
    let mut j_counter_min_indent = 0;

    let mut out = VecDeque::with_capacity(limit);
    out.push_back(&collected[anchor_index]);

    while out.len() < final_limit {
        let mut progressed = 0;

        // Up.
        if i >= 0 {
            let iu = i as usize;
            if effective_indents[iu] >= min_indent {
                out.push_front(&collected[iu]);
                progressed += 1;
                i -= 1;

                if effective_indents[iu] == min_indent && !options.include_siblings {
                    let allow_header_comment =
                        options.include_header && collected[iu].is_comment();
                    let can_take_line = allow_header_comment || i_counter_min_indent == 0;

                    if can_take_line {
                        i_counter_min_indent += 1;
                    } else {
                        out.pop_front();
                        progressed -= 1;
                        i = -1;
                    }
                }

                if out.len() >= final_limit {
                    break;
                }
            } else {
                i = -1;
            }
        }

        // Down.
        if j < collected.len() {
            let ju = j;
            if effective_indents[ju] >= min_indent {
                out.push_back(&collected[ju]);
                progressed += 1;
                j += 1;

                if effective_indents[ju] == min_indent && !options.include_siblings {
                    if j_counter_min_indent > 0 {
                        out.pop_back();
                        progressed -= 1;
                        j = collected.len();
                    }
                    j_counter_min_indent += 1;
                }
            } else {
                j = collected.len();
            }
        }

        if progressed == 0 {
            break;
        }
    }

    // Trim empty lines
    trim_empty_lines(&mut out);

    Ok(out
        .into_iter()
        .map(|record| format!("L{}: {}", record.number, record.display))
        .collect())
}

async fn collect_file_lines(path: &Path) -> Result<Vec<LineRecord>> {
    let file = File::open(path).await
        .map_err(|err| anyhow!("failed to read file: {err}"))?;

    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    let mut lines = Vec::new();
    let mut number = 0usize;

    loop {
        buffer.clear();
        let bytes_read = reader.read_until(b'\n', &mut buffer).await
            .map_err(|err| anyhow!("failed to read file: {err}"))?;

        if bytes_read == 0 {
            break;
        }

        if buffer.last() == Some(&b'\n') {
            buffer.pop();
            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }
        }

        number += 1;
        let raw = String::from_utf8_lossy(&buffer).into_owned();
        let indent = measure_indent(&raw);
        let display = format_line(&buffer);
        lines.push(LineRecord {
            number,
            raw,
            display,
            indent,
        });
    }

    Ok(lines)
}

fn compute_effective_indents(records: &[LineRecord]) -> Vec<usize> {
    let mut effective = Vec::with_capacity(records.len());
    let mut previous_indent = 0usize;
    for record in records {
        if record.is_blank() {
            effective.push(previous_indent);
        } else {
            previous_indent = record.indent;
            effective.push(previous_indent);
        }
    }
    effective
}

fn measure_indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .map(|c| if c == '\t' { TAB_WIDTH } else { 1 })
        .sum()
}

fn trim_empty_lines(out: &mut VecDeque<&LineRecord>) {
    while matches!(out.front(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_front();
    }
    while matches!(out.back(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_back();
    }
}
```

### 步骤 5：集成到 ToolRouter

**任务：**
- 更新 `read_file` 函数签名
- 解析 JSON 参数
- 根据 mode 选择读取方式
- 返回格式化的结果

**代码：**
```rust
async fn read_file(call: &ToolCall, cwd: &Path) -> Result<String> {
    let arguments = call.args.get("arguments")
        .ok_or_else(|| anyhow!("read_file requires arguments"))?;

    let args: ReadFileArgs = serde_json::from_str(arguments)
        .map_err(|err| anyhow!("failed to parse arguments: {err}"))?;

    let ReadFileArgs {
        file_path,
        offset,
        limit,
        mode,
        indentation,
    } = args;

    // 验证
    if offset == 0 {
        return Err(anyhow!("offset must be a 1-indexed line number"));
    }

    if limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    let path = resolve_workspace_path(cwd, &file_path)?;

    let collected = match mode {
        ReadMode::Slice => read_slice(&path, offset, limit).await?,
        ReadMode::Indentation => {
            let indentation = indentation.unwrap_or_default();
            read_indentation_block(&path, offset, limit, indentation).await?
        }
    };

    Ok(collected.join("\n"))
}
```

### 步骤 6：编写单元测试

**任务：**
- 测试基本读取
- 测试 offset 和 limit
- 测试超长行截断
- 测试 CRLF 处理
- 测试非 UTF8 字符
- 测试 offset 超出文件长度
- 测试 limit 限制
- 测试 Indentation 模式（如果实现）

**代码：**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn reads_requested_range() -> anyhow::Result<()> {
        let mut temp = NamedTempFile::new()?;
        write!(temp, "alpha\nbeta\ngamma\n")?;

        let lines = read_slice(temp.path(), 2, 2).await?;
        assert_eq!(lines, vec!["L2: beta".to_string(), "L3: gamma".to_string()]);
        Ok(())
    }

    #[tokio::test]
    async fn errors_when_offset_exceeds_length() -> anyhow::Result<()> {
        let mut temp = NamedTempFile::new()?;
        writeln!(temp, "only")?;

        let err = read_slice(temp.path(), 3, 1)
            .await
            .expect_err("offset exceeds length");
        assert_eq!(err.to_string(), "offset exceeds file length");
        Ok(())
    }

    #[tokio::test]
    async fn reads_non_utf8_lines() -> anyhow::Result<()> {
        let mut temp = NamedTempFile::new()?;
        temp.as_file_mut().write_all(b"\xff\xfe\nplain\n")?;

        let lines = read_slice(temp.path(), 1, 2).await?;
        let expected_first = format!("L1: {}{}", '\u{FFFD}', '\u{FFFD}');
        assert_eq!(lines, vec![expected_first, "L2: plain".to_string()]);
        Ok(())
    }

    #[tokio::test]
    async fn trims_crlf_endings() -> anyhow::Result<()> {
        let mut temp = NamedTempFile::new()?;
        write!(temp, "one\r\ntwo\r\n")?;

        let lines = read_slice(temp.path(), 1, 2).await?;
        assert_eq!(lines, vec!["L1: one".to_string(), "L2: two".to_string()]);
        Ok(())
    }

    #[tokio::test]
    async fn respects_limit_even_with_more_lines() -> anyhow::Result<()> {
        let mut temp = NamedTempFile::new()?;
        write!(temp, "first\nsecond\nthird\n")?;

        let lines = read_slice(temp.path(), 1, 2).await?;
        assert_eq!(
            lines,
            vec!["L1: first".to_string(), "L2: second".to_string()]
        );
        Ok(())
    }

    #[tokio::test]
    async fn truncates_lines_longer_than_max_length() -> anyhow::Result<()> {
        let mut temp = NamedTempFile::new()?;
        let long_line = "x".repeat(MAX_LINE_LENGTH + 50);
        writeln!(temp, "{long_line}")?;

        let lines = read_slice(temp.path(), 1, 1).await?;
        let expected = "x".repeat(MAX_LINE_LENGTH);
        assert_eq!(lines, vec![format!("L1: {expected}")]);
        Ok(())
    }
}
```

### 步骤 7：更新文档

**任务：**
- 更新 [tools.md](../../tools.md) 中的 read_file 文档
- 添加新参数说明
- 更新使用示例
- 添加 Indentation 模式说明（如果实现）

## 测试计划

### 单元测试

- ✅ 基本读取功能
- ✅ offset 和 limit 参数
- ✅ 超长行截断
- ✅ CRLF 处理
- ✅ 非 UTF8 字符处理
- ✅ offset 超出文件长度
- ✅ limit 限制
- ✅ Indentation 模式（如果实现）

### 集成测试

- ✅ 端到端读取流程
- ✅ 与其他工具的集成
- ✅ 错误处理

### 性能测试

- ✅ 大文件读取性能
- ✅ 分页读取性能
- ✅ Indentation 模式性能（如果实现）

## 风险与缓解

### 风险 1：Indentation 模式复杂度高

**影响：** 开发时间长，容易出错

**缓解：**
- 分阶段实现，先实现 Slice 模式
- 充分的单元测试
- 参考 Codex 的测试用例

### 风险 2：路径验证不一致

**影响：** 安全漏洞或路径逃逸

**缓解：**
- 复用 OpenJax 现有的路径验证逻辑
- 充分的单元测试
- 安全审查

## 交付物

- ✅ 支持 offset 和 limit 参数
- ✅ 显示行号
- ✅ 超长行截断（500 字符）
- ✅ CRLF 处理
- ✅ 非 UTF8 字符处理
- ✅ 完整的单元测试
- ✅ 更新的文档
- ⏸️ 缩进感知模式（可选，后续优化）
- ⏸️ 注释识别（可选，后续优化）

## 工作量估算

- 步骤 1：添加参数结构 - 0.5 天
- 步骤 2：实现参数验证 - 0.5 天
- 步骤 3：实现 Slice 模式 - 1 天
- 步骤 4：实现 Indentation 模式（可选） - 2 天
- 步骤 5：集成到 ToolRouter - 0.5 天
- 步骤 6：编写单元测试 - 1 天
- 步骤 7：更新文档 - 0.5 天

**总计（不含 Indentation 模式）：** 4-5 天
**总计（含 Indentation 模式）：** 6-7 天

## 参考资料

- [Codex read_file 实现](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/read_file.rs)
- [OpenJax 工具对比](../../tools-comparison.md)
