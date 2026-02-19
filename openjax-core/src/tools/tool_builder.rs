use std::sync::Arc;
use crate::tools::context::{SandboxPolicy, ApprovalPolicy};
use crate::tools::registry::{ToolHandler, ToolRegistry};
use crate::tools::spec::{ToolSpec, ToolsConfig, build_all_specs};
use crate::tools::handlers::{ApplyPatchHandler, EditFileRangeHandler, GrepFilesHandler, ListDirHandler, ReadFileHandler, ShellCommandHandler};

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
    let config = ToolsConfig::default();
    
    for spec in build_all_specs(&config) {
        let parallel = !spec.name.eq("apply_patch");
        builder.push_spec(spec, parallel);
    }
    
    let grep_handler = Arc::new(GrepFilesHandler);
    builder.register_handler("grep_files", grep_handler);
    
    let read_handler = Arc::new(ReadFileHandler);
    builder.register_handler("read_file", read_handler);
    
    let list_handler = Arc::new(ListDirHandler);
    builder.register_handler("list_dir", list_handler);
    
    let shell_handler = Arc::new(ShellCommandHandler);
    builder.register_handler("shell", shell_handler.clone());
    // Backward-compatible alias; primary tool name is `shell`.
    builder.register_handler("exec_command", shell_handler);
    
    let patch_handler = Arc::new(ApplyPatchHandler);
    builder.register_handler("apply_patch", patch_handler);

    let edit_range_handler = Arc::new(EditFileRangeHandler);
    builder.register_handler("edit_file_range", edit_range_handler);
    
    builder.build()
}

/// 创建工具调用上下文
pub fn create_tool_invocation(
    tool_name: String,
    arguments: String,
    cwd: std::path::PathBuf,
    sandbox_policy: SandboxPolicy,
    approval_policy: ApprovalPolicy,
) -> crate::tools::context::ToolInvocation {
    crate::tools::context::ToolInvocation {
        tool_name,
        call_id: uuid::Uuid::new_v4().to_string(),
        payload: crate::tools::context::ToolPayload::Function { arguments },
        turn: crate::tools::context::ToolTurnContext {
            cwd,
            sandbox_policy,
            approval_policy,
            windows_sandbox_level: None,
        },
    }
}
