# 工具重构实施方案

## 概述

本文档详细说明了如何复用 Codex 的工具实现来重构 OpenJax 的 `grep_files`、`read_file` 和 `list_dir` 三个工具。

## 核心策略

**直接复用 Codex 核心逻辑 + 适配 OpenJax 架构**

- ✅ **复用**: Codex 的核心算法和实现逻辑
- ✅ **保留**: OpenJax 的现有架构和接口
- ✅ **适配**: 路径系统和参数解析

## 架构对比

### OpenJax 当前架构

```rust
pub struct ToolCall {
    pub name: String,
    pub args: HashMap<String, String>,
}

async fn tool_name(call: &ToolCall, cwd: &Path) -> Result<String>
```

**特点**:
- 使用相对路径系统（`resolve_workspace_path`）
- 简单的字符串参数（`HashMap<String, String>`）
- 返回 `Result<String>`

### Codex 架构

```rust
pub struct ToolInvocation {
    pub payload: ToolPayload,
    pub turn: Turn,
}

#[async_trait]
impl ToolHandler for Handler {
    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>
}
```

**特点**:
- 使用绝对路径系统（`turn.resolve_path`）
- JSON 参数解析（`serde::Deserialize`）
- 返回 `Result<ToolOutput, FunctionCallError>`

## 重构方案

### 1. grep_files 工具

#### 当前实现问题

- ❌ 使用 `walkdir` 遍历文件，性能差
- ❌ 简单字符串匹配，不支持正则表达式
- ❌ 无超时控制
- ❌ 无分页支持
- ❌ 无 glob 过滤

#### Codex 实现优势

- ✅ 使用 `ripgrep` 外部命令，性能提升 10-50 倍
- ✅ 支持正则表达式
- ✅ 30 秒超时控制
- ✅ 支持分页（`limit` 参数）
- ✅ 支持 glob 过滤（`include` 参数）
- ✅ 按修改时间排序（`--sortr=modified`）

#### 实施方案

**步骤 1**: 添加参数解析

```rust
#[derive(Deserialize)]
struct GrepFilesArgs {
    pattern: String,
    #[serde(default)]
    include: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 2000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
```

**步骤 2**: 复用 Codex 的 `run_rg_search` 实现

```rust
async fn run_rg_search(
    pattern: &str,
    include: Option<&str>,
    search_path: &Path,
    limit: usize,
    cwd: &Path,
) -> Result<Vec<String>> {
    let mut command = Command::new("rg");
    command
        .current_dir(cwd)
        .arg("--files-with-matches")
        .arg("--sortr=modified")
        .arg("--regexp")
        .arg(pattern)
        .arg("--no-messages");

    if let Some(glob) = include {
        command.arg("--glob").arg(glob);
    }

    command.arg("--").arg(search_path);

    let output = timeout(COMMAND_TIMEOUT, command.output())
        .await
        .map_err(|_| anyhow!("rg timed out after 30 seconds"))?
        .map_err(|err| anyhow!("failed to launch rg: {err}. Ensure ripgrep is installed and on PATH."))?;

    match output.status.code() {
        Some(0) => Ok(parse_results(&output.stdout, limit)),
        Some(1) => Ok(Vec::new()),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("rg failed: {stderr}"))
        }
    }
}
```

**步骤 3**: 适配 OpenJax 接口

```rust
async fn grep_files(call: &ToolCall, cwd: &Path) -> Result<String> {
    let args: GrepFilesArgs = parse_tool_args(call)?;

    let pattern = args.pattern.trim();
    if pattern.is_empty() {
        return Err(anyhow!("pattern must not be empty"));
    }

    let limit = args.limit.min(MAX_LIMIT);
    let rel_path = args.path.unwrap_or_else(|| ".".to_string());
    let search_path = resolve_workspace_path(cwd, &rel_path)?;

    verify_path_exists(&search_path).await?;

    let include = args.include.as_deref().map(str::trim).and_then(|val| {
        if val.is_empty() { None } else { Some(val.to_string()) }
    });

    let search_results = run_rg_search(pattern, include.as_deref(), &search_path, limit, cwd).await?;

    if search_results.is_empty() {
        Ok("No matches found.".to_string())
    } else {
        Ok(search_results.join("\n"))
    }
}
```

**步骤 4**: 添加辅助函数

```rust
fn parse_tool_args<T: DeserializeOwned>(call: &ToolCall) -> Result<T> {
    let json_str = serde_json::to_string(&call.args)?;
    serde_json::from_str(&json_str).map_err(|e| anyhow!("failed to parse arguments: {e}"))
}

async fn verify_path_exists(path: &Path) -> Result<()> {
    tokio::fs::metadata(path).await
        .with_context(|| format!("unable to access `{}`", path.display()))?;
    Ok(())
}

fn parse_results(stdout: &[u8], limit: usize) -> Vec<String> {
    let mut results = Vec::new();
    for line in stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() { continue; }
        if let Ok(text) = std::str::from_utf8(line) {
            if text.is_empty() { continue; }
            results.push(text.to_string());
            if results.len() == limit { break; }
        }
    }
    results
}
```

#### 新增依赖

无需新增依赖：
- `tokio::process::Command` - 已有
- `tokio::time::timeout` - 已有
- `serde` - 已有

外部依赖：
- `ripgrep (rg)` - 系统工具，需要安装

#### 测试用例

```rust
#[tokio::test]
async fn grep_files_with_regex() {
    let workspace = create_workspace();
    fs::write(workspace.join("test.rs"), "fn main() { println!(\"hello\"); }").await.unwrap();

    let result = grep_files(&ToolCall {
        name: "grep_files".to_string(),
        args: HashMap::from([
            ("pattern".to_string(), "fn \\w+".to_string()),
            ("path".to_string(), ".".to_string()),
        ]),
    }, &workspace).await.unwrap();

    assert!(result.contains("test.rs"));
}
```

---

### 2. read_file 工具

#### 当前实现问题

- ❌ 读取整个文件，无分页
- ❌ 无行号显示
- ❌ 超长行不截断
- ❌ 无缩进感知模式

#### Codex 实现优势

- ✅ 支持分页（`offset` 和 `limit` 参数）
- ✅ 显示行号（`L1: content` 格式）
- ✅ 超长行截断（500 字符）
- ✅ 缩进感知模式（智能扩展代码块）
- ✅ CRLF 处理
- ✅ 非 UTF-8 字符处理

#### 实施方案

**步骤 1**: 添加参数解析

```rust
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

#[derive(Deserialize, Default)]
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

const MAX_LINE_LENGTH: usize = 500;
const TAB_WIDTH: usize = 4;
const COMMENT_PREFIXES: &[&str] = &["#", "//", "--"];
```

**步骤 2**: 复用 Codex 的 `slice::read` 实现

```rust
mod slice {
    pub async fn read(
        path: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<String>> {
        let file = File::open(path).await
            .with_context(|| "failed to read file")?;

        let mut reader = BufReader::new(file);
        let mut collected = Vec::new();
        let mut seen = 0usize;
        let mut buffer = Vec::new();

        loop {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer).await
                .with_context(|| "failed to read file")?;

            if bytes_read == 0 { break; }

            if buffer.last() == Some(&b'\n') {
                buffer.pop();
                if buffer.last() == Some(&b'\r') {
                    buffer.pop();
                }
            }

            seen += 1;

            if seen < offset { continue; }
            if collected.len() == limit { break; }

            let formatted = format_line(&buffer);
            collected.push(format!("L{seen}: {formatted}"));
        }

        if seen < offset {
            return Err(anyhow!("offset exceeds file length"));
        }

        Ok(collected)
    }
}
```

**步骤 3**: 复用 Codex 的 `indentation::read_block` 实现（可选）

```rust
mod indentation {
    pub async fn read_block(
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

        let min_indent = if options.max_levels == 0 {
            0
        } else {
            anchor_indent.saturating_sub(options.max_levels * TAB_WIDTH)
        };

        let final_limit = limit.min(guard_limit).min(collected.len());

        if final_limit == 1 {
            return Ok(vec![format!(
                "L{}: {}",
                collected[anchor_index].number, collected[anchor_index].display
            )]);
        }

        let mut i: isize = anchor_index as isize - 1;
        let mut j: usize = anchor_index + 1;
        let mut i_counter_min_indent = 0;
        let mut j_counter_min_indent = 0;

        let mut out = VecDeque::with_capacity(limit);
        out.push_back(&collected[anchor_index]);

        while out.len() < final_limit {
            let mut progressed = 0;

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

                    if out.len() >= final_limit { break; }
                } else {
                    i = -1;
                }
            }

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

            if progressed == 0 { break; }
        }

        trim_empty_lines(&mut out);

        Ok(out.into_iter()
            .map(|record| format!("L{}: {}", record.number, record.display))
            .collect())
    }

    async fn collect_file_lines(path: &Path) -> Result<Vec<LineRecord>> {
        let file = File::open(path).await
            .with_context(|| "failed to read file")?;

        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        let mut lines = Vec::new();
        let mut number = 0usize;

        loop {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer).await
                .with_context(|| "failed to read file")?;

            if bytes_read == 0 { break; }

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
            lines.push(LineRecord { number, raw, display, indent });
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
}
```

**步骤 4**: 适配 OpenJax 接口

```rust
async fn read_file(call: &ToolCall, cwd: &Path) -> Result<String> {
    let args: ReadFileArgs = parse_tool_args(call)?;

    if args.offset == 0 {
        return Err(anyhow!("offset must be a 1-indexed line number"));
    }

    if args.limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    let rel_path = args.file_path;
    let path = resolve_workspace_path(cwd, &rel_path)?;

    let collected = match args.mode {
        ReadMode::Slice => slice::read(&path, args.offset, args.limit).await?,
        ReadMode::Indentation => {
            let indentation = args.indentation.unwrap_or_default();
            indentation::read_block(&path, args.offset, args.limit, indentation).await?
        }
    };

    Ok(collected.join("\n"))
}
```

**步骤 5**: 添加辅助函数

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
        COMMENT_PREFIXES
            .iter()
            .any(|prefix| self.raw.trim().starts_with(prefix))
    }
}

fn format_line(bytes: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(bytes);
    if decoded.len() > MAX_LINE_LENGTH {
        take_bytes_at_char_boundary(&decoded, MAX_LINE_LENGTH).to_string()
    } else {
        decoded.into_owned()
    }
}

fn trim_empty_lines(out: &mut VecDeque<&LineRecord>) {
    while matches!(out.front(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_front();
    }
    while matches!(out.back(), Some(line) if line.raw.trim().is_empty()) {
        out.pop_back();
    }
}

fn take_bytes_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

mod defaults {
    pub fn offset() -> usize { 1 }
    pub fn limit() -> usize { 2000 }
    pub fn max_levels() -> usize { 0 }
    pub fn include_siblings() -> bool { false }
    pub fn include_header() -> bool { true }
}
```

#### 新增依赖

无需新增依赖：
- `tokio::fs::File` - 已有
- `tokio::io::BufReader` - 已有
- `serde` - 已有

#### 测试用例

```rust
#[tokio::test]
async fn read_file_with_offset_and_limit() {
    let workspace = create_workspace();
    fs::write(workspace.join("test.txt"), "line1\nline2\nline3\nline4\nline5").await.unwrap();

    let result = read_file(&ToolCall {
        name: "read_file".to_string(),
        args: HashMap::from([
            ("file_path".to_string(), "test.txt".to_string()),
            ("offset".to_string(), "2".to_string()),
            ("limit".to_string(), "2".to_string()),
        ]),
    }, &workspace).await.unwrap();

    assert_eq!(result, "L2: line2\nL3: line3");
}
```

---

### 3. list_dir 工具

#### 当前实现问题

- ❌ 列出单层目录，无递归
- ❌ 无分页支持
- ❌ 无文件类型标记
- ❌ 无缩进显示

#### Codex 实现优势

- ✅ 支持递归（`depth` 参数）
- ✅ 支持分页（`offset` 和 `limit` 参数）
- ✅ 文件类型标记（`/` 目录、`@` 符号链接、`?` 其他）
- ✅ 缩进显示层级结构
- ✅ 超长条目名截断（500 字符）
- ✅ 显示绝对路径

#### 实施方案

**步骤 1**: 添加参数解析

```rust
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

const MAX_ENTRY_LENGTH: usize = 500;
const INDENTATION_SPACES: usize = 2;

fn default_offset() -> usize { 1 }
fn default_limit() -> usize { 25 }
fn default_depth() -> usize { 2 }
```

**步骤 2**: 复用 Codex 的 `collect_entries` 实现

```rust
async fn collect_entries(
    dir_path: &Path,
    relative_prefix: &Path,
    depth: usize,
    entries: &mut Vec<DirEntry>,
) -> Result<()> {
    let mut queue = VecDeque::new();
    queue.push_back((dir_path.to_path_buf(), relative_prefix.to_path_buf(), depth));

    while let Some((current_dir, prefix, remaining_depth)) = queue.pop_front() {
        let mut read_dir = fs::read_dir(&current_dir).await
            .with_context(|| "failed to read directory")?;

        let mut dir_entries = Vec::new();

        while let Some(entry) = read_dir.next_entry().await
            .with_context(|| "failed to read directory")? {
            let file_type = entry.file_type().await
                .with_context(|| "failed to inspect entry")?;

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

**步骤 3**: 复用 Codex 的 `list_dir_slice` 实现

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
```

**步骤 4**: 适配 OpenJax 接口

```rust
async fn list_dir(call: &ToolCall, cwd: &Path) -> Result<String> {
    let args: ListDirArgs = parse_tool_args(call)?;

    if args.offset == 0 {
        return Err(anyhow!("offset must be a 1-indexed entry number"));
    }

    if args.limit == 0 {
        return Err(anyhow!("limit must be greater than zero"));
    }

    if args.depth == 0 {
        return Err(anyhow!("depth must be greater than zero"));
    }

    let rel_path = args.dir_path;
    let path = resolve_workspace_path(cwd, &rel_path)?;

    let entries = list_dir_slice(&path, args.offset, args.limit, args.depth).await?;
    let mut output = Vec::with_capacity(entries.len() + 1);
    output.push(format!("Absolute path: {}", path.display()));
    output.extend(entries);

    Ok(output.join("\n"))
}
```

**步骤 5**: 添加辅助函数

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

#### 新增依赖

无需新增依赖：
- `tokio::fs` - 已有
- `serde` - 已有

#### 测试用例

```rust
#[tokio::test]
async fn list_dir_with_depth() {
    let workspace = create_workspace();
    fs::create_dir(workspace.join("nested")).await.unwrap();
    fs::create_dir(workspace.join("nested/deeper")).await.unwrap();
    fs::write(workspace.join("root.txt"), b"root").await.unwrap();
    fs::write(workspace.join("nested/child.txt"), b"child").await.unwrap();

    let result = list_dir(&ToolCall {
        name: "list_dir".to_string(),
        args: HashMap::from([
            ("dir_path".to_string(), ".".to_string()),
            ("depth".to_string(), "2".to_string()),
        ]),
    }, &workspace).await.unwrap();

    assert!(result.contains("nested/"));
    assert!(result.contains("  child.txt"));
}
```

---

## 实施顺序

### 阶段 1: grep_files（P0，2-3 天）

1. ✅ 添加参数解析结构
2. ✅ 复用 `run_rg_search` 实现
3. ✅ 适配 OpenJax 接口
4. ✅ 添加辅助函数
5. ✅ 编写测试用例
6. ✅ 运行测试

### 阶段 2: read_file（P0，3-4 天）

1. ✅ 添加参数解析结构
2. ✅ 复用 `slice::read` 实现
3. ✅ （可选）复用 `indentation::read_block` 实现
4. ✅ 适配 OpenJax 接口
5. ✅ 添加辅助函数
6. ✅ 编写测试用例
7. ✅ 运行测试

### 阶段 3: list_dir（P0，2-3 天）

1. ✅ 添加参数解析结构
2. ✅ 复用 `collect_entries` 实现
3. ✅ 复用 `list_dir_slice` 实现
4. ✅ 适配 OpenJax 接口
5. ✅ 添加辅助函数
6. ✅ 编写测试用例
7. ✅ 运行测试

**总工作量：** 7-10 天（不含 read_file 的 Indentation 模式）

## 关键依赖

### 新增依赖

无需新增依赖：
- `grep_files` 使用系统 ripgrep
- `read_file` 和 `list_dir` 使用现有依赖

### 外部依赖

- `ripgrep (rg)` - 系统工具，需要安装

## 成功标准

### 功能标准

- ✅ grep_files 使用 ripgrep，支持正则、glob、分页
- ✅ read_file 支持分页、行号、超长行截断
- ✅ list_dir 支持递归、分页、文件类型标记

### 性能标准

- ✅ grep_files 性能提升 10 倍以上
- ✅ read_file 支持大文件分页读取
- ✅ list_dir 支持大目录分页列出

### 质量标准

- ✅ 所有工具通过单元测试
- ✅ 所有工具通过集成测试
- ✅ 代码审查通过
- ✅ 文档完整更新

## 风险管理

### 高风险

| 风险 | 影响 | 缓解措施 |
|-----|------|---------|
| ripgrep 不可用 | grep_files 无法工作 | 检测 + 友好错误提示 |
| 路径验证不一致 | 安全漏洞 | 复用现有逻辑 + 充分测试 |

### 中风险

| 风险 | 影响 | 缓解措施 |
|-----|------|---------|
| Indentation 模式复杂度高 | 开发时间长 | 分阶段实现 + 充分测试 |
| 递归深度过大 | 性能问题 | 限制最大深度 + BFS |

## 后续优化

### 短期（1-2 周）

1. 添加 read_file 的 Indentation 模式
2. 添加 read_file 的注释识别
3. 添加工具遥测支持
4. 添加钩子支持

### 中期（1-2 月）

1. 重构 shell（参考 Codex）
2. 添加 ToolRegistry
3. 添加 ToolOrchestrator
4. 添加 ToolRuntime 抽象

### 长期（3-6 月）

1. 支持动态工具加载
2. 支持 MCP 工具
3. 支持工具插件系统

## 参考资料

- [Codex Tool System](/Users/ericw/work/code/ai/codex/docs/tool-system.md)
- [Codex vs OpenJax 工具对比](../../tools-comparison.md)
- [OpenJax 工具文档](../../tools.md)
- [Codex grep_files 实现](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/grep_files.rs)
- [Codex read_file 实现](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/read_file.rs)
- [Codex list_dir 实现](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/list_dir.rs)
