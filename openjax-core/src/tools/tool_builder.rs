use crate::approval::ApprovalHandler;
use crate::tools::context::{ApprovalPolicy, SandboxPolicy};
use crate::tools::handlers::{
    ApplyPatchHandler, EditFileRangeHandler, GrepFilesHandler, ListDirHandler, ReadFileHandler,
    ShellCommandHandler,
};
use crate::tools::registry::{ToolHandler, ToolRegistry};
use crate::tools::spec::{ToolSpec, ToolsConfig, build_all_specs};
use crate::tools::system::{DiskUsageHandler, ProcessSnapshotHandler, SystemLoadHandler};
use std::sync::Arc;

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

    let process_snapshot_handler = Arc::new(ProcessSnapshotHandler::default());
    builder.register_handler("process_snapshot", process_snapshot_handler);

    let system_load_handler = Arc::new(SystemLoadHandler::default());
    builder.register_handler("system_load", system_load_handler);

    let disk_usage_handler = Arc::new(DiskUsageHandler::default());
    builder.register_handler("disk_usage", disk_usage_handler);

    builder.build()
}

/// 创建工具调用上下文
pub fn create_tool_invocation(
    turn_id: u64,
    call_id: String,
    tool_name: String,
    arguments: String,
    cwd: std::path::PathBuf,
    sandbox_policy: SandboxPolicy,
    approval_policy: ApprovalPolicy,
    prevent_shell_skill_trigger: bool,
    approval_handler: Arc<dyn ApprovalHandler>,
    event_sink: Option<tokio::sync::mpsc::UnboundedSender<openjax_protocol::Event>>,
) -> crate::tools::context::ToolInvocation {
    crate::tools::context::ToolInvocation {
        tool_name,
        call_id,
        payload: crate::tools::context::ToolPayload::Function { arguments },
        turn: crate::tools::context::ToolTurnContext {
            turn_id,
            cwd,
            sandbox_policy,
            approval_policy,
            prevent_shell_skill_trigger,
            approval_handler,
            event_sink,
            windows_sandbox_level: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::build_default_tool_registry;

    #[test]
    fn default_registry_includes_system_tools() {
        let (registry, specs) = build_default_tool_registry();
        assert!(registry.handler("process_snapshot").is_some());
        assert!(registry.handler("system_load").is_some());
        assert!(registry.handler("disk_usage").is_some());

        let names: Vec<String> = specs.into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"process_snapshot".to_string()));
        assert!(names.contains(&"system_load".to_string()));
        assert!(names.contains(&"disk_usage".to_string()));
    }
}
