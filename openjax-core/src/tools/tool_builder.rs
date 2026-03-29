use crate::approval::ApprovalHandler;
use crate::tools::context::SandboxPolicy;
use crate::tools::handlers::{
    EditHandler, GlobFilesHandler, GrepFilesHandler, ListDirHandler,
    ReadHandler, ShellCommandHandler, WriteFileHandler,
};
use crate::tools::registry::{ToolHandler, ToolRegistry};
use crate::tools::shell::ShellType;
use crate::tools::spec::{ShellToolType, ToolSpec, ToolsConfig, build_all_specs};
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
    build_tool_registry_with_config(&ToolsConfig::default())
}

/// 按配置构建工具注册表
pub fn build_tool_registry_with_config(config: &ToolsConfig) -> (ToolRegistry, Vec<ToolSpec>) {
    let mut builder = ToolRegistryBuilder::new();

    for spec in build_all_specs(config) {
        builder.push_spec(spec, true);
    }

    let grep_handler = Arc::new(GrepFilesHandler);
    builder.register_handler("grep_files", grep_handler);

    let glob_handler = Arc::new(GlobFilesHandler);
    builder.register_handler("glob_files", glob_handler);

    let read_handler = Arc::new(ReadHandler);
    builder.register_handler("Read", read_handler);

    let list_handler = Arc::new(ListDirHandler);
    builder.register_handler("list_dir", list_handler);

    if !matches!(config.shell_type, ShellToolType::Disabled) {
        let shell_handler = Arc::new(ShellCommandHandler);
        builder.register_handler("shell", shell_handler.clone());
        // Backward-compatible alias; primary tool name is `shell`.
        builder.register_handler("exec_command", shell_handler);
    }

    let edit_handler = Arc::new(EditHandler);
    builder.register_handler("Edit", edit_handler);

    let write_file_handler = Arc::new(WriteFileHandler);
    builder.register_handler("write_file", write_file_handler);

    let process_snapshot_handler = Arc::new(ProcessSnapshotHandler::default());
    builder.register_handler("process_snapshot", process_snapshot_handler);

    let system_load_handler = Arc::new(SystemLoadHandler::default());
    builder.register_handler("system_load", system_load_handler);

    let disk_usage_handler = Arc::new(DiskUsageHandler::default());
    builder.register_handler("disk_usage", disk_usage_handler);

    builder.build()
}

pub struct CreateToolInvocationParams {
    pub turn_id: u64,
    pub session_id: Option<String>,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub cwd: std::path::PathBuf,
    pub sandbox_policy: SandboxPolicy,
    pub shell_type: ShellType,
    pub prevent_shell_skill_trigger: bool,
    pub approval_handler: Arc<dyn ApprovalHandler>,
    pub event_sink: Option<tokio::sync::mpsc::UnboundedSender<openjax_protocol::Event>>,
    pub policy_runtime: Option<openjax_policy::runtime::PolicyRuntime>,
}

/// 创建工具调用上下文
pub fn create_tool_invocation(
    params: CreateToolInvocationParams,
) -> crate::tools::context::ToolInvocation {
    crate::tools::context::ToolInvocation {
        tool_name: params.tool_name,
        call_id: params.call_id,
        payload: crate::tools::context::ToolPayload::Function {
            arguments: params.arguments,
        },
        turn: crate::tools::context::ToolTurnContext {
            turn_id: params.turn_id,
            session_id: params.session_id,
            cwd: params.cwd,
            sandbox_policy: params.sandbox_policy,
            shell_type: params.shell_type,
            prevent_shell_skill_trigger: params.prevent_shell_skill_trigger,
            approval_handler: params.approval_handler,
            event_sink: params.event_sink,
            policy_runtime: params.policy_runtime,
            windows_sandbox_level: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateToolInvocationParams, build_default_tool_registry, build_tool_registry_with_config,
        create_tool_invocation,
    };
    use crate::approval::StdinApprovalHandler;
    use crate::tools::context::{SandboxPolicy, ToolPayload};
    use crate::tools::shell::ShellType;
    use crate::tools::spec::{ShellToolType, ToolsConfig};
    use std::sync::Arc;

    #[test]
    fn default_registry_includes_system_tools() {
        let (registry, specs) = build_default_tool_registry();
        let legacy_read = format!("{}_{}", "read", "file");
        let legacy_edit = format!("{}_{}_{}", "edit", "file", "range");
        assert!(registry.handler("Read").is_some());
        assert!(registry.handler("Edit").is_some());
        assert!(registry.handler(&legacy_read).is_none());
        assert!(registry.handler(&legacy_edit).is_none());
        assert!(registry.handler("process_snapshot").is_some());
        assert!(registry.handler("system_load").is_some());
        assert!(registry.handler("disk_usage").is_some());

        let names: Vec<String> = specs.into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"Read".to_string()));
        assert!(names.contains(&"Edit".to_string()));
        assert!(!names.contains(&legacy_read));
        assert!(!names.contains(&legacy_edit));
        assert!(names.contains(&"process_snapshot".to_string()));
        assert!(names.contains(&"system_load".to_string()));
        assert!(names.contains(&"disk_usage".to_string()));
    }

    #[test]
    fn create_tool_invocation_keeps_shell_type_from_runtime_config() {
        let invocation = create_tool_invocation(CreateToolInvocationParams {
            turn_id: 42,
            session_id: Some("sess_42".to_string()),
            call_id: "call-42".to_string(),
            tool_name: "shell".to_string(),
            arguments: r#"{"cmd":"echo hello"}"#.to_string(),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::Sh,
            prevent_shell_skill_trigger: true,
            approval_handler: Arc::new(StdinApprovalHandler::new()),
            event_sink: None,
            policy_runtime: None,
        });
        assert!(matches!(invocation.payload, ToolPayload::Function { .. }));
        assert_eq!(invocation.turn.shell_type, ShellType::Sh);
    }

    #[test]
    fn registry_build_disables_shell_tools_when_configured() {
        let config = ToolsConfig {
            shell_type: ShellToolType::Disabled,
        };
        let (registry, specs) = build_tool_registry_with_config(&config);
        assert!(registry.handler("shell").is_none());
        assert!(registry.handler("exec_command").is_none());
        assert!(!specs.iter().any(|spec| spec.name == "shell"));
    }
}
