use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

use super::errors::SystemToolError;
use super::provider::{DefaultSystemMetricsProvider, SystemMetricsProvider, choose_disk_by_path};
use super::types::{DiskUsageArgs, DiskUsageRecord, ErrorBody, ErrorEnvelope};

pub struct DiskUsageHandler {
    provider: Arc<dyn SystemMetricsProvider>,
}

impl DiskUsageHandler {
    pub fn new(provider: Arc<dyn SystemMetricsProvider>) -> Self {
        Self { provider }
    }
}

impl Default for DiskUsageHandler {
    fn default() -> Self {
        Self::new(Arc::new(DefaultSystemMetricsProvider))
    }
}

#[derive(Serialize)]
struct DiskUsageResponse {
    timestamp: String,
    selected_path: String,
    items: Vec<DiskUsageItem>,
}

#[derive(Serialize)]
struct DiskUsageItem {
    mount_point: String,
    fs_name: String,
    total_bytes: u64,
    available_bytes: u64,
    used_bytes: u64,
    used_pct: f64,
}

#[async_trait]
impl ToolHandler for DiskUsageHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let arguments = match invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "disk_usage handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: DiskUsageArgs = match serde_json::from_str(&arguments) {
            Ok(parsed) => parsed,
            Err(err) => {
                return Ok(system_error_output(SystemToolError::InvalidArgument(
                    format!("failed to parse arguments: {err}"),
                )));
            }
        };

        let selected_path = match resolve_selected_path(&args, self.provider.as_ref()) {
            Ok(path) => path,
            Err(err) => return Ok(system_error_output(err)),
        };

        let disks = match self.provider.collect_disks() {
            Ok(data) => data,
            Err(err) => return Ok(system_error_output(err)),
        };

        if disks.is_empty() {
            return Ok(system_error_output(SystemToolError::CollectionFailed(
                "no disk metrics available".to_string(),
            )));
        }

        let selected_items = if args.include_all_mounts {
            disks
        } else {
            let Some(disk) = choose_disk_by_path(&disks, &selected_path).cloned() else {
                return Ok(system_error_output(SystemToolError::CollectionFailed(
                    "failed to map selected path to a mounted filesystem".to_string(),
                )));
            };
            vec![disk]
        };

        let response = DiskUsageResponse {
            timestamp: now_rfc3339(),
            selected_path: selected_path.display().to_string(),
            items: selected_items
                .into_iter()
                .map(|item| to_disk_item(&item))
                .collect(),
        };

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Json(serde_json::to_value(response).map_err(|err| {
                FunctionCallError::Internal(format!("failed to serialize disk usage: {err}"))
            })?),
            success: Some(true),
        })
    }
}

fn resolve_selected_path(
    args: &DiskUsageArgs,
    provider: &dyn SystemMetricsProvider,
) -> Result<PathBuf, SystemToolError> {
    let raw_path = match &args.path {
        Some(path) => path.clone(),
        None => provider.default_path()?,
    };

    let path = PathBuf::from(raw_path);
    if !path.exists() {
        return Err(SystemToolError::InvalidArgument(format!(
            "path does not exist: {}",
            path.display()
        )));
    }
    std::fs::canonicalize(path)
        .map_err(|err| SystemToolError::InvalidArgument(format!("invalid path: {err}")))
}

fn to_disk_item(disk: &DiskUsageRecord) -> DiskUsageItem {
    let used_bytes = disk.total_bytes.saturating_sub(disk.available_bytes);
    let used_pct = if disk.total_bytes > 0 {
        (used_bytes as f64 / disk.total_bytes as f64) * 100.0
    } else {
        0.0
    };

    DiskUsageItem {
        mount_point: disk.mount_point.clone(),
        fs_name: disk.fs_name.clone(),
        total_bytes: disk.total_bytes,
        available_bytes: disk.available_bytes,
        used_bytes,
        used_pct,
    }
}

fn system_error_output(err: SystemToolError) -> ToolOutput {
    ToolOutput::Function {
        body: FunctionCallOutputBody::Json(serde_json::json!(ErrorEnvelope {
            error: ErrorBody {
                code: err.code().to_string(),
                message: err.to_string(),
            }
        })),
        success: Some(false),
    }
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::{DiskUsageHandler, to_disk_item};
    use crate::tools::FunctionCallOutputBody;
    use crate::tools::context::{SandboxPolicy, ToolInvocation, ToolPayload, ToolTurnContext};
    use crate::tools::registry::ToolHandler;
    use crate::tools::system::errors::SystemToolError;
    use crate::tools::system::provider::SystemMetricsProvider;
    use crate::tools::system::types::{
        CpuLoadRecord, DiskUsageRecord, LoadAverageRecord, MemoryLoadRecord, ProcessRecord,
    };
    use std::sync::Arc;

    #[test]
    fn used_percentage_is_bounded() {
        let item = to_disk_item(&DiskUsageRecord {
            mount_point: "/".to_string(),
            fs_name: "apfs".to_string(),
            total_bytes: 100,
            available_bytes: 60,
        });

        assert_eq!(item.used_bytes, 40);
        assert!((0.0..=100.0).contains(&item.used_pct));
    }

    struct MockProvider;

    impl SystemMetricsProvider for MockProvider {
        fn collect_processes(&self) -> Result<Vec<ProcessRecord>, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_cpu_load(&self) -> Result<CpuLoadRecord, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_memory_load(&self) -> Result<MemoryLoadRecord, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_load_average(&self) -> Result<LoadAverageRecord, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_disks(&self) -> Result<Vec<DiskUsageRecord>, SystemToolError> {
            Ok(vec![
                DiskUsageRecord {
                    mount_point: "/".to_string(),
                    fs_name: "rootfs".to_string(),
                    total_bytes: 1000,
                    available_bytes: 250,
                },
                DiskUsageRecord {
                    mount_point: "/tmp".to_string(),
                    fs_name: "tmpfs".to_string(),
                    total_bytes: 500,
                    available_bytes: 100,
                },
            ])
        }

        fn hostname(&self) -> Option<String> {
            Some("test-host".to_string())
        }

        fn default_path(&self) -> Result<String, SystemToolError> {
            Ok("/tmp".to_string())
        }
    }

    fn invocation(arguments: &str) -> ToolInvocation {
        ToolInvocation {
            tool_name: "disk_usage".to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
            turn: ToolTurnContext {
                sandbox_policy: SandboxPolicy::Write,
                ..ToolTurnContext::default()
            },
        }
    }

    #[tokio::test]
    async fn invalid_path_returns_invalid_argument() {
        let handler = DiskUsageHandler::new(Arc::new(MockProvider));
        let output = handler
            .handle(invocation(r#"{"path":"/definitely-not-exists-openjax"}"#))
            .await
            .expect("tool execution should return output");
        match output {
            crate::tools::ToolOutput::Function {
                body: FunctionCallOutputBody::Json(value),
                success: Some(false),
            } => assert_eq!(value["error"]["code"], "invalid_argument"),
            _ => panic!("expected invalid_argument output"),
        }
    }

    #[tokio::test]
    async fn include_all_mounts_returns_multiple_items() {
        let handler = DiskUsageHandler::new(Arc::new(MockProvider));
        let output = handler
            .handle(invocation(r#"{"include_all_mounts":"true"}"#))
            .await
            .expect("tool execution should return output");
        match output {
            crate::tools::ToolOutput::Function {
                body: FunctionCallOutputBody::Json(value),
                success: Some(true),
            } => assert!(value["items"].as_array().map(Vec::len).unwrap_or(0) >= 2),
            _ => panic!("expected success output"),
        }
    }
}
