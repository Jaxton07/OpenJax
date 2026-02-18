# grep_files 重构实施计划

## 目标

将 `grep_files` 从基于 walkdir 的简单字符串匹配重构为基于 ripgrep 的高性能搜索工具，达到 Codex 的功能水平。

## 当前实现

**位置：** [openjax-core/src/tools.rs:288-324](../../openjax-core/src/tools.rs#L288-L324)

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `pattern` | string | - | 搜索模式 |
| `path` | string | `.` | 搜索路径 |

**特性：**
- ✅ 递归搜索所有文件
- ✅ 返回行号和内容
- ❌ 不支持正则表达式（仅简单字符串匹配）
- ❌ 性能较差（逐文件读取）
- ❌ 无 glob 模式过滤
- ❌ 无分页支持
- ❌ 无超时控制

**输出格式：**
```
file_a.rs:1:fn main() {
file_b.rs:2:    println!("Hello");
```

## 目标实现

**参考：** [codex-rs/core/src/tools/handlers/grep_files.rs](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/grep_files.rs)

**参数：**
| 参数名 | 类型 | 默认值 | 说明 |
|-------|------|--------|------|
| `pattern` | string | - | 搜索模式（支持正则表达式） |
| `include` | string | - | glob 模式过滤（可选） |
| `path` | string | - | 搜索路径（可选） |
| `limit` | number | 100 | 最大结果数（最大 2000） |

**特性：**
- ✅ 使用 ripgrep (`rg`) 命令
- ✅ 支持正则表达式
- ✅ 按修改时间倒序排序
- ✅ 支持 glob 模式过滤
- ✅ 分页支持（limit）
- ✅ 30 秒超时
- ✅ 仅返回文件路径（不返回行号和内容）
- ✅ 无匹配时返回 `No matches found.`

**输出格式：**
```
/path/to/file_a.rs
/path/to/file_b.rs
```

## 实施步骤

### 步骤 1：添加参数验证

**任务：**
- 添加 `limit` 参数（默认 100，最大 2000）
- 添加 `include` 参数（可选）
- 验证 `pattern` 不为空
- 验证 `limit` 大于 0

**代码：**
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

fn default_limit() -> usize {
    100
}

// 验证
if pattern.trim().is_empty() {
    return Err(anyhow!("pattern must not be empty"));
}

if limit == 0 {
    return Err(anyhow!("limit must be greater than zero"));
}

let limit = limit.min(2000);
```

### 步骤 2：实现 ripgrep 调用

**任务：**
- 使用 `tokio::process::Command` 调用 `rg`
- 添加 `--files-with-matches` 参数（仅返回文件路径）
- 添加 `--sortr=modified` 参数（按修改时间倒序）
- 添加 `--regexp` 参数（支持正则表达式）
- 添加 `--no-messages` 参数（抑制错误消息）
- 支持 `--glob` 参数（glob 模式过滤）
- 添加超时控制（30 秒）

**代码：**
```rust
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

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
        Some(1) => Ok(Vec::new()), // No matches
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("rg failed: {stderr}"))
        }
    }
}
```

### 步骤 3：实现结果解析

**任务：**
- 解析 ripgrep 输出（每行一个文件路径）
- 应用 limit 限制
- 返回文件路径列表

**代码：**
```rust
fn parse_results(stdout: &[u8], limit: usize) -> Vec<String> {
    let mut results = Vec::new();
    for line in stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if let Ok(text) = std::str::from_utf8(line) {
            if text.is_empty() {
                continue;
            }
            results.push(text.to_string());
            if results.len() == limit {
                break;
            }
        }
    }
    results
}
```

### 步骤 4：路径验证

**任务：**
- 复用 OpenJax 现有的路径验证逻辑
- 验证路径在工作区内
- 验证路径存在

**代码：**
```rust
async fn verify_path_exists(path: &Path) -> Result<()> {
    tokio::fs::metadata(path).await.map_err(|err| {
        anyhow!("unable to access `{}`: {err}", path.display())
    })?;
    Ok(())
}

// 使用
let search_path = args.path.as_deref().unwrap_or(".");
let search_path = resolve_workspace_path(cwd, search_path)?;
verify_path_exists(&search_path).await?;
```

### 步骤 5：集成到 ToolRouter

**任务：**
- 更新 `grep_files` 函数签名
- 调用新的实现
- 返回格式化的结果

**代码：**
```rust
async fn grep_files(call: &ToolCall, cwd: &Path) -> Result<String> {
    let pattern = call.args.get("pattern")
        .ok_or_else(|| anyhow!("grep_files requires pattern=<text>"))?;

    let include = call.args.get("include").map(|s| s.as_str());
    let path_arg = call.args.get("path").map(|s| s.as_str()).unwrap_or(".");
    let limit = call.args.get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100)
        .min(2000);

    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err(anyhow!("pattern must not be empty"));
    }

    let search_path = resolve_workspace_path(cwd, path_arg)?;
    verify_path_exists(&search_path).await?;

    let search_results = run_rg_search(pattern, include, &search_path, limit, cwd).await?;

    if search_results.is_empty() {
        Ok("No matches found.".to_string())
    } else {
        Ok(search_results.join("\n"))
    }
}
```

### 步骤 6：编写单元测试

**任务：**
- 测试基本搜索
- 测试 glob 过滤
- 测试 limit 限制
- 测试无匹配情况
- 测试超时处理
- 测试 ripgrep 不可用情况

**代码：**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_basic_results() {
        let stdout = b"/tmp/file_a.rs\n/tmp/file_b.rs\n";
        let parsed = parse_results(stdout, 10);
        assert_eq!(
            parsed,
            vec!["/tmp/file_a.rs".to_string(), "/tmp/file_b.rs".to_string()]
        );
    }

    #[test]
    fn parse_truncates_after_limit() {
        let stdout = b"/tmp/file_a.rs\n/tmp/file_b.rs\n/tmp/file_c.rs\n";
        let parsed = parse_results(stdout, 2);
        assert_eq!(
            parsed,
            vec!["/tmp/file_a.rs".to_string(), "/tmp/file_b.rs".to_string()]
        );
    }

    #[tokio::test]
    async fn run_search_returns_results() -> anyhow::Result<()> {
        if !rg_available() {
            return Ok(());
        }
        let temp = tempdir()?;
        let dir = temp.path();
        std::fs::write(dir.join("match_one.txt"), "alpha beta gamma")?;
        std::fs::write(dir.join("match_two.txt"), "alpha delta")?;
        std::fs::write(dir.join("other.txt"), "omega")?;

        let results = run_rg_search("alpha", None, dir, 10, dir).await?;
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|path| path.ends_with("match_one.txt")));
        assert!(results.iter().any(|path| path.ends_with("match_two.txt")));
        Ok(())
    }

    #[tokio::test]
    async fn run_search_with_glob_filter() -> anyhow::Result<()> {
        if !rg_available() {
            return Ok(());
        }
        let temp = tempdir()?;
        let dir = temp.path();
        std::fs::write(dir.join("match_one.rs"), "alpha beta gamma")?;
        std::fs::write(dir.join("match_two.txt"), "alpha delta")?;

        let results = run_rg_search("alpha", Some("*.rs"), dir, 10, dir).await?;
        assert_eq!(results.len(), 1);
        assert!(results.iter().all(|path| path.ends_with("match_one.rs")));
        Ok(())
    }

    #[tokio::test]
    async fn run_search_respects_limit() -> anyhow::Result<()> {
        if !rg_available() {
            return Ok(());
        }
        let temp = tempdir()?;
        let dir = temp.path();
        std::fs::write(dir.join("one.txt"), "alpha one")?;
        std::fs::write(dir.join("two.txt"), "alpha two")?;
        std::fs::write(dir.join("three.txt"), "alpha three")?;

        let results = run_rg_search("alpha", None, dir, 2, dir).await?;
        assert_eq!(results.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn run_search_handles_no_matches() -> anyhow::Result<()> {
        if !rg_available() {
            return Ok(());
        }
        let temp = tempdir()?;
        let dir = temp.path();
        std::fs::write(dir.join("one.txt"), "omega")?;

        let results = run_rg_search("alpha", None, dir, 5, dir).await?;
        assert!(results.is_empty());
        Ok(())
    }

    fn rg_available() -> bool {
        std::process::Command::new("rg")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}
```

### 步骤 7：更新文档

**任务：**
- 更新 [tools.md](../../tools.md) 中的 grep_files 文档
- 添加新参数说明
- 更新使用示例
- 添加 ripgrep 依赖说明

## 测试计划

### 单元测试

- ✅ 基本搜索功能
- ✅ glob 模式过滤
- ✅ limit 限制
- ✅ 无匹配情况
- ✅ 超时处理
- ✅ ripgrep 不可用情况
- ✅ 路径验证

### 集成测试

- ✅ 端到端搜索流程
- ✅ 与其他工具的集成
- ✅ 错误处理

### 性能测试

- ✅ 对比 walkdir 实现的性能
- ✅ 大文件搜索性能
- ✅ 大目录搜索性能
- ✅ 正则表达式性能

## 性能基准

### 预期提升

| 场景 | 当前实现 | 目标实现 | 提升倍数 |
|-----|---------|---------|---------|
| 小文件搜索（100 文件） | ~100ms | ~10ms | 10x |
| 中等文件搜索（1000 文件） | ~1000ms | ~50ms | 20x |
| 大文件搜索（10000 文件） | ~10000ms | ~200ms | 50x |

### 测试方法

```bash
# 创建测试文件
for i in {1..1000}; do echo "content $i" > "file_$i.txt"; done

# 测试当前实现
time tool:grep_files pattern=content path=.

# 测试新实现
time tool:grep_files pattern=content path=. limit=100
```

## 风险与缓解

### 风险 1：ripgrep 不可用

**影响：** grep_files 无法工作

**缓解：**
- 检测 ripgrep 是否安装
- 提供友好的错误提示
- 考虑回退到 walkdir 实现（性能较差）

**代码：**
```rust
fn rg_available() -> bool {
    std::process::Command::new("rg")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

// 使用前检查
if !rg_available() {
    return Err(anyhow!(
        "ripgrep (rg) is not installed. Please install it from https://github.com/BurntSushi/ripgrep"
    ));
}
```

### 风险 2：路径验证不一致

**影响：** 安全漏洞或路径逃逸

**缓解：**
- 复用 OpenJax 现有的路径验证逻辑
- 充分的单元测试
- 安全审查

## 交付物

- ✅ 使用 ripgrep 的 grep_files 实现
- ✅ 支持正则表达式
- ✅ 支持 glob 模式过滤
- ✅ 支持分页（limit 参数）
- ✅ 支持超时控制（30 秒）
- ✅ 完整的单元测试
- ✅ 更新的文档

## 工作量估算

- 步骤 1：添加参数验证 - 0.5 天
- 步骤 2：实现 ripgrep 调用 - 0.5 天
- 步骤 3：实现结果解析 - 0.5 天
- 步骤 4：路径验证 - 0.5 天
- 步骤 5：集成到 ToolRouter - 0.5 天
- 步骤 6：编写单元测试 - 1 天
- 步骤 7：更新文档 - 0.5 天

**总计：** 4-5 天

## 参考资料

- [Codex grep_files 实现](/Users/ericw/work/code/ai/codex/codex-rs/core/src/tools/handlers/grep_files.rs)
- [ripgrep 文档](https://github.com/BurntSushi/ripgrep)
- [OpenJax 工具对比](../../tools-comparison.md)
