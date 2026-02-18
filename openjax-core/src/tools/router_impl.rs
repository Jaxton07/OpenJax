use anyhow::{Result, anyhow};
use tracing::{debug, info, warn};

use super::router::{ToolCall, ToolRuntimeConfig};
use super::grep_files::grep_files;
use super::read_file::read_file;
use super::list_dir::list_dir;
use super::exec_command::exec_command;
use super::apply_patch::apply_patch_tool;

pub struct ToolRouter;

impl ToolRouter {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        call: &ToolCall,
        cwd: &std::path::Path,
        config: ToolRuntimeConfig,
    ) -> Result<String> {
        debug!(
            tool_name = %call.name,
            args = ?call.args,
            cwd = %cwd.display(),
            sandbox_mode = config.sandbox_mode.as_str(),
            "tool_execute started"
        );
        let result = match call.name.as_str() {
            "read_file" => read_file(call, cwd).await,
            "list_dir" => list_dir(call, cwd).await,
            "grep_files" => grep_files(call, cwd).await,
            "exec_command" => exec_command(call, cwd, config).await,
            "apply_patch" => apply_patch_tool(call, cwd).await,
            _ => {
                warn!(tool_name = %call.name, "tool_execute unknown tool");
                Err(anyhow!("unknown tool: {}", call.name))
            }
        };

        let output = match &result {
            Ok(output) => {
                info!(tool_name = %call.name, output_len = output.len(), "tool_execute completed");
                output.clone()
            }
            Err(err) => {
                warn!(tool_name = %call.name, error = %err, "tool_execute failed");
                format!("error: {err}")
            }
        };

        Ok(output)
    }
}
