use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 工具规范
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub display_name: String,
}

/// 工具配置
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct ToolsConfig {
    pub shell_type: ShellToolType,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum ShellToolType {
    Default,
    Local,
    UnifiedExec,
    Disabled,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum ApplyPatchToolType {
    Default,
    Freeform,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            shell_type: ShellToolType::Default,
            apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
        }
    }
}

/// Freeform 工具格式
#[derive(Debug, Clone)]
pub struct FreeformFormat {
    pub r#type: String,
    pub syntax: String,
    pub definition: String,
}

/// 创建 apply_patch Freeform 工具规范
pub fn create_apply_patch_freeform_spec() -> ToolSpec {
    ToolSpec {
        name: "apply_patch".to_string(),
        description: r#"Use the `apply_patch` tool to edit files. This is a FREEFORM tool, so do not wrap the patch in JSON."#.to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "description": "Format type (e.g., 'grammar')"
                        },
                        "syntax": {
                            "type": "string",
                            "description": "Syntax parser (e.g., 'lark')"
                        },
                        "definition": {
                            "type": "string",
                            "description": "Grammar definition"
                        }
                    }
                }
            },
            "required": []
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "Summary of applied patch operations (ADD, UPDATE, DELETE, MOVE)"
        })),
        display_name: "Apply Patch".to_string(),
    }
}

/// 创建 grep_files 工具规范
pub fn create_grep_files_spec() -> ToolSpec {
    ToolSpec {
        name: "grep_files".to_string(),
        description: "Search files using ripgrep with regex pattern support. Returns a list of matching file paths sorted by modification time (newest first).".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., *.rs, src/**/*.ts)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results (default: 100, max: 2000)",
                    "default": 100,
                    "minimum": 1,
                    "maximum": 2000
                }
            },
            "required": ["pattern"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "List of matching file paths, one per line"
        })),
        display_name: "Search Files".to_string(),
    }
}

/// 创建 read_file 工具规范
pub fn create_read_file_spec() -> ToolSpec {
    ToolSpec {
        name: "read_file".to_string(),
        description: "Read file contents with support for pagination and indentation-aware reading. Returns file lines with line numbers in the format 'L<line_number>: <content>'.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "1-indexed line number to start reading from (default: 1)",
                    "default": 1,
                    "minimum": 1
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of lines to read (default: 2000)",
                    "default": 2000,
                    "minimum": 1
                },
                "mode": {
                    "type": "string",
                    "enum": ["slice", "indentation"],
                    "description": "Reading mode: 'slice' for simple pagination, 'indentation' for context-aware reading",
                    "default": "slice"
                },
                "indentation": {
                    "type": "object",
                    "description": "Options for indentation-aware reading (only used when mode='indentation')",
                    "properties": {
                        "anchor_line": {
                            "type": "number",
                            "description": "1-indexed line number to anchor around (default: offset)",
                            "minimum": 1
                        },
                        "max_levels": {
                            "type": "number",
                            "description": "Maximum indentation levels to include (0 = unlimited)",
                            "default": 0,
                            "minimum": 0
                        },
                        "include_siblings": {
                            "type": "boolean",
                            "description": "Include sibling lines at the same indentation level",
                            "default": false
                        },
                        "include_header": {
                            "type": "boolean",
                            "description": "Include comment headers at the same indentation level",
                            "default": true
                        },
                        "max_lines": {
                            "type": "number",
                            "description": "Maximum number of lines to return (default: limit)",
                            "minimum": 1
                        }
                    }
                }
            },
            "required": ["file_path"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "File contents with line numbers, one per line"
        })),
        display_name: "Read File".to_string(),
    }
}

/// 创建 list_dir 工具规范
pub fn create_list_dir_spec() -> ToolSpec {
    ToolSpec {
        name: "list_dir".to_string(),
        description: "List directory contents with support for recursive listing and pagination. Returns directory entries with indentation to show hierarchy. Entries are marked with '/' for directories, '@' for symlinks, and '?' for other types.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "dir_path": {
                    "type": "string",
                    "description": "Path to the directory to list"
                },
                "offset": {
                    "type": "number",
                    "description": "1-indexed entry number to start listing from (default: 1)",
                    "default": 1,
                    "minimum": 1
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of entries to list (default: 25)",
                    "default": 25,
                    "minimum": 1
                },
                "depth": {
                    "type": "number",
                    "description": "Maximum recursion depth (default: 2)",
                    "default": 2,
                    "minimum": 1
                }
            },
            "required": ["dir_path"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "Directory entries with indentation and type markers"
        })),
        display_name: "List Directory".to_string(),
    }
}

/// 创建 shell 工具规范
pub fn create_shell_spec() -> ToolSpec {
    ToolSpec {
        name: "shell".to_string(),
        description: "Execute a shell command with optional approval and sandbox restrictions. Returns exit code, stdout, and stderr. The command runs in the detected user shell with the current working directory set to the workspace root.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "cmd": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "require_escalated": {
                    "type": "boolean",
                    "description": "Whether the command requires elevated privileges (triggers approval)",
                    "default": false
                },
                "timeout_ms": {
                    "type": "number",
                    "description": "Command timeout in milliseconds (default: 30000)",
                    "default": 30000,
                    "minimum": 1000
                }
            },
            "required": ["cmd"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "Command execution result with exit_code, stdout, and stderr sections"
        })),
        display_name: "Run Shell".to_string(),
    }
}

/// 兼容旧名称：exec_command
pub fn create_exec_command_spec() -> ToolSpec {
    let mut spec = create_shell_spec();
    spec.name = "exec_command".to_string();
    spec.display_name = "Run Shell".to_string();
    spec
}

/// 创建 apply_patch 工具规范
pub fn create_apply_patch_spec() -> ToolSpec {
    ToolSpec {
        name: "apply_patch".to_string(),
        description: "Apply a patch to the workspace. Supports adding, deleting, moving, renaming, and updating files. Returns a summary of applied changes. The patch format uses '*** Begin Patch' and '*** End Patch' delimiters with operations like '*** Add File:', '*** Delete File:', '*** Update File:', '*** Move File:', and '*** Rename File:'.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "Patch text to apply"
                }
            },
            "required": ["patch"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "Summary of applied patch operations (ADD, UPDATE, DELETE, MOVE)"
        })),
        display_name: "Apply Patch".to_string(),
    }
}

/// 创建 edit_file_range 工具规范
pub fn create_edit_file_range_spec() -> ToolSpec {
    ToolSpec {
        name: "edit_file_range".to_string(),
        description: "Edit a file by replacing an inclusive line range [start_line, end_line] with new_text. Line numbers are 1-indexed.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "start_line": {
                    "type": "number",
                    "description": "Start line (1-indexed, inclusive)",
                    "minimum": 1
                },
                "end_line": {
                    "type": "number",
                    "description": "End line (1-indexed, inclusive)",
                    "minimum": 1
                },
                "new_text": {
                    "type": "string",
                    "description": "Replacement text for the specified line range. Use empty string to delete the range."
                }
            },
            "required": ["file_path", "start_line", "end_line", "new_text"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "Summary of applied edit"
        })),
        display_name: "Edit File".to_string(),
    }
}

/// 创建 write_file 工具规范
pub fn create_write_file_spec() -> ToolSpec {
    ToolSpec {
        name: "write_file".to_string(),
        description: "Write file content to a workspace-relative path. Creates missing parent directories and overwrites existing content.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Full content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        }),
        output_schema: Some(serde_json::json!({
            "type": "string",
            "description": "Write summary including path and byte count"
        })),
        display_name: "Write File".to_string(),
    }
}

/// 创建 process_snapshot 工具规范
pub fn create_process_snapshot_spec() -> ToolSpec {
    ToolSpec {
        name: "process_snapshot".to_string(),
        description: "Collect a read-only process snapshot without shell execution. Supports sorting by cpu or memory with optional user filtering.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "sort_by": {
                    "type": "string",
                    "enum": ["cpu", "memory"],
                    "default": "cpu"
                },
                "limit": {
                    "type": "number",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 100
                },
                "user": {
                    "type": "string",
                    "description": "Optional user name filter"
                }
            }
        }),
        output_schema: Some(serde_json::json!({
            "type": "object"
        })),
        display_name: "Process Snapshot".to_string(),
    }
}

/// 创建 system_load 工具规范
pub fn create_system_load_spec() -> ToolSpec {
    ToolSpec {
        name: "system_load".to_string(),
        description: "Collect host CPU, memory, and load-average metrics in a structured response."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "include_cpu": {
                    "type": "boolean",
                    "default": true
                },
                "include_memory": {
                    "type": "boolean",
                    "default": true
                }
            }
        }),
        output_schema: Some(serde_json::json!({
            "type": "object"
        })),
        display_name: "System Load".to_string(),
    }
}

/// 创建 disk_usage 工具规范
pub fn create_disk_usage_spec() -> ToolSpec {
    ToolSpec {
        name: "disk_usage".to_string(),
        description:
            "Collect filesystem usage metrics for a selected path or all mounted filesystems."
                .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path used to resolve the target mount (defaults to cwd)"
                },
                "include_all_mounts": {
                    "type": "boolean",
                    "default": false
                }
            }
        }),
        output_schema: Some(serde_json::json!({
            "type": "object"
        })),
        display_name: "Disk Usage".to_string(),
    }
}

/// 构建所有工具规范
pub fn build_all_specs(config: &ToolsConfig) -> Vec<ToolSpec> {
    let mut specs = vec![
        create_grep_files_spec(),
        create_read_file_spec(),
        create_list_dir_spec(),
        create_process_snapshot_spec(),
        create_system_load_spec(),
        create_disk_usage_spec(),
        create_edit_file_range_spec(),
        create_write_file_spec(),
    ];

    if !matches!(config.shell_type, ShellToolType::Disabled) {
        specs.push(create_shell_spec());
    }

    specs.push(match config.apply_patch_tool_type {
        Some(ApplyPatchToolType::Freeform) => create_apply_patch_freeform_spec(),
        _ => create_apply_patch_spec(),
    });

    specs
}
