use std::sync::Arc;
use crate::tools::context::SandboxPolicy;
use crate::tools::registry::{ToolHandler, ToolRegistry};
use crate::tools::spec::{ToolSpec, ToolsConfig, create_grep_files_spec, create_read_file_spec, create_list_dir_spec, create_exec_command_spec, create_apply_patch_spec};
use crate::tools::handlers::{GrepFilesHandler, ReadFileHandler, ListDirHandler, ShellCommandHandler, ApplyPatchHandler};

/// 工具注册构建器
pub struct ToolRegistryBuilder {
    specs: Vec<ToolSpec>,
    registry: ToolRegistry,
}

impl ToolRegistryBuilder {
    pub fn new() -> Self {
        Self {
            specs: Vec::new(),
            registry: ToolRegistry::new(),
        }
    }

    /// 添加工具规范
    pub fn push_spec(&mut self, spec: ToolSpec, _parallel: bool) {
        self.specs.push(spec);
    }

    /// 注册工具处理器
    pub fn register_handler(&mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) {
        self.registry.register(name, handler);
    }

    /// 构建工具注册表和规范
    pub fn build(self) -> (ToolRegistry, Vec<ToolSpec>) {
        (self.registry, self.specs)
    }
}

impl Default for ToolRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 构建默认的工具注册表
pub fn build_default_tool_registry() -> (ToolRegistry, Vec<ToolSpec>) {
    let mut builder = ToolRegistryBuilder::new();
    let _config = ToolsConfig::default();

    // 注册 grep_files
    let grep_handler = Arc::new(GrepFilesHandler);
    builder.push_spec(create_grep_files_spec(), true);
    builder.register_handler("grep_files", grep_handler);

    // 注册 read_file
    let read_handler = Arc::new(ReadFileHandler);
    builder.push_spec(create_read_file_spec(), true);
    builder.register_handler("read_file", read_handler);

    // 注册 list_dir
    let list_handler = Arc::new(ListDirHandler);
    builder.push_spec(create_list_dir_spec(), true);
    builder.register_handler("list_dir", list_handler);

    // 注册 exec_command
    let exec_handler = Arc::new(ShellCommandHandler);
    builder.push_spec(create_exec_command_spec(), true);
    builder.register_handler("exec_command", exec_handler);

    // 注册 apply_patch
    let patch_handler = Arc::new(ApplyPatchHandler);
    builder.push_spec(create_apply_patch_spec(), true);
    builder.register_handler("apply_patch", patch_handler);

    builder.build()
}

/// 创建工具调用上下文
pub fn create_tool_invocation(
    tool_name: String,
    arguments: String,
    cwd: std::path::PathBuf,
    sandbox_policy: SandboxPolicy,
) -> crate::tools::context::ToolInvocation {
    crate::tools::context::ToolInvocation {
        tool_name,
        call_id: uuid::Uuid::new_v4().to_string(),
        payload: crate::tools::context::ToolPayload::Function { arguments },
        turn: crate::tools::context::ToolTurnContext {
            cwd,
            sandbox_policy,
            windows_sandbox_level: None,
        },
    }
}
