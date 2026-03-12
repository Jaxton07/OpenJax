use std::cmp::Ordering;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

use super::errors::SystemToolError;
use super::provider::{DefaultSystemMetricsProvider, SystemMetricsProvider};
use super::types::{ErrorBody, ErrorEnvelope, ProcessRecord, ProcessSnapshotArgs, ProcessSortBy};

pub struct ProcessSnapshotHandler {
    provider: Arc<dyn SystemMetricsProvider>,
}

impl ProcessSnapshotHandler {
    pub fn new(provider: Arc<dyn SystemMetricsProvider>) -> Self {
        Self { provider }
    }
}

impl Default for ProcessSnapshotHandler {
    fn default() -> Self {
        Self::new(Arc::new(DefaultSystemMetricsProvider))
    }
}

#[derive(Serialize)]
struct ProcessSnapshotResponse {
    timestamp: String,
    host: Option<String>,
    items: Vec<ProcessSnapshotItem>,
    meta: ProcessSnapshotMeta,
}

#[derive(Serialize)]
struct ProcessSnapshotItem {
    pid: u32,
    name: String,
    cpu_pct: f32,
    memory_bytes: u64,
    memory_pct: f64,
    user: Option<String>,
    status: String,
}

#[derive(Serialize)]
struct ProcessSnapshotMeta {
    sort_by: ProcessSortBy,
    limit: usize,
    sampled_at_ms: u128,
}

#[async_trait]
impl ToolHandler for ProcessSnapshotHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let arguments = match invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "process_snapshot handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: ProcessSnapshotArgs = match serde_json::from_str(&arguments) {
            Ok(parsed) => parsed,
            Err(err) => {
                return Ok(system_error_output(SystemToolError::InvalidArgument(
                    format!("failed to parse arguments: {err}"),
                )));
            }
        };

        if !(1..=100).contains(&args.limit) {
            return Ok(system_error_output(SystemToolError::InvalidArgument(
                "limit must be within 1..=100".to_string(),
            )));
        }

        let sampled_at = std::time::Instant::now();
        let mut items = match self.provider.collect_processes() {
            Ok(data) => data,
            Err(err) => return Ok(system_error_output(err)),
        };

        if let Some(user) = &args.user {
            items.retain(|item| item.user.as_deref() == Some(user.as_str()));
        }

        items.sort_by(|left, right| compare_records(left, right, args.sort_by));
        items.truncate(args.limit);

        let response = ProcessSnapshotResponse {
            timestamp: now_rfc3339(),
            host: self.provider.hostname(),
            items: items
                .into_iter()
                .map(|item| ProcessSnapshotItem {
                    pid: item.pid,
                    name: item.name,
                    cpu_pct: item.cpu_pct,
                    memory_bytes: item.memory_bytes,
                    memory_pct: item.memory_pct,
                    user: item.user,
                    status: item.status,
                })
                .collect(),
            meta: ProcessSnapshotMeta {
                sort_by: args.sort_by,
                limit: args.limit,
                sampled_at_ms: sampled_at.elapsed().as_millis(),
            },
        };

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Json(serde_json::to_value(response).map_err(|err| {
                FunctionCallError::Internal(format!("failed to serialize process snapshot: {err}"))
            })?),
            success: Some(true),
        })
    }
}

fn compare_records(
    left: &ProcessRecord,
    right: &ProcessRecord,
    sort_by: ProcessSortBy,
) -> Ordering {
    match sort_by {
        ProcessSortBy::Cpu => right
            .cpu_pct
            .partial_cmp(&left.cpu_pct)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.memory_bytes.cmp(&left.memory_bytes))
            .then_with(|| left.pid.cmp(&right.pid)),
        ProcessSortBy::Memory => right
            .memory_bytes
            .cmp(&left.memory_bytes)
            .then_with(|| {
                right
                    .cpu_pct
                    .partial_cmp(&left.cpu_pct)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.pid.cmp(&right.pid)),
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
    use super::ProcessSnapshotHandler;
    use super::compare_records;
    use crate::tools::FunctionCallOutputBody;
    use crate::tools::context::{
        ApprovalPolicy, SandboxPolicy, ToolInvocation, ToolPayload, ToolTurnContext,
    };
    use crate::tools::registry::ToolHandler;
    use crate::tools::system::errors::SystemToolError;
    use crate::tools::system::provider::SystemMetricsProvider;
    use crate::tools::system::types::{ProcessRecord, ProcessSortBy};
    use std::sync::Arc;

    fn rec(pid: u32, cpu_pct: f32, mem: u64) -> ProcessRecord {
        ProcessRecord {
            pid,
            name: "proc".to_string(),
            cpu_pct,
            memory_bytes: mem,
            memory_pct: 1.0,
            user: None,
            status: "running".to_string(),
        }
    }

    #[test]
    fn sorts_by_cpu_then_memory() {
        let mut items = [rec(1, 5.0, 100), rec(2, 10.0, 50), rec(3, 10.0, 200)];
        items.sort_by(|l, r| compare_records(l, r, ProcessSortBy::Cpu));
        assert_eq!(items[0].pid, 3);
        assert_eq!(items[1].pid, 2);
        assert_eq!(items[2].pid, 1);
    }

    #[test]
    fn sorts_by_memory_then_cpu() {
        let mut items = [rec(1, 5.0, 100), rec(2, 10.0, 50), rec(3, 10.0, 200)];
        items.sort_by(|l, r| compare_records(l, r, ProcessSortBy::Memory));
        assert_eq!(items[0].pid, 3);
        assert_eq!(items[1].pid, 1);
        assert_eq!(items[2].pid, 2);
    }

    struct MockProvider {
        processes: Result<Vec<ProcessRecord>, SystemToolError>,
    }

    impl SystemMetricsProvider for MockProvider {
        fn collect_processes(&self) -> Result<Vec<ProcessRecord>, SystemToolError> {
            self.processes.clone()
        }

        fn collect_cpu_load(
            &self,
        ) -> Result<crate::tools::system::types::CpuLoadRecord, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_memory_load(
            &self,
        ) -> Result<crate::tools::system::types::MemoryLoadRecord, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_load_average(
            &self,
        ) -> Result<crate::tools::system::types::LoadAverageRecord, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_disks(
            &self,
        ) -> Result<Vec<crate::tools::system::types::DiskUsageRecord>, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn hostname(&self) -> Option<String> {
            Some("test-host".to_string())
        }

        fn default_path(&self) -> Result<String, SystemToolError> {
            Ok(".".to_string())
        }
    }

    fn invocation(arguments: &str) -> ToolInvocation {
        ToolInvocation {
            tool_name: "process_snapshot".to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
            turn: ToolTurnContext {
                approval_policy: ApprovalPolicy::OnRequest,
                sandbox_policy: SandboxPolicy::Write,
                ..ToolTurnContext::default()
            },
        }
    }

    #[tokio::test]
    async fn limit_out_of_range_returns_invalid_argument() {
        let handler = ProcessSnapshotHandler::new(Arc::new(MockProvider {
            processes: Ok(vec![]),
        }));
        let output = handler
            .handle(invocation(r#"{"limit":"101"}"#))
            .await
            .expect("tool execution should return output");
        match output {
            crate::tools::ToolOutput::Function {
                body: FunctionCallOutputBody::Json(value),
                success: Some(false),
            } => assert_eq!(value["error"]["code"], "invalid_argument"),
            _ => panic!("expected structured invalid_argument response"),
        }
    }

    #[tokio::test]
    async fn user_filter_applies_to_results() {
        let handler = ProcessSnapshotHandler::new(Arc::new(MockProvider {
            processes: Ok(vec![
                ProcessRecord {
                    pid: 1,
                    name: "a".to_string(),
                    cpu_pct: 12.0,
                    memory_bytes: 100,
                    memory_pct: 20.0,
                    user: Some("alice".to_string()),
                    status: "running".to_string(),
                },
                ProcessRecord {
                    pid: 2,
                    name: "b".to_string(),
                    cpu_pct: 10.0,
                    memory_bytes: 99,
                    memory_pct: 19.8,
                    user: Some("bob".to_string()),
                    status: "sleep".to_string(),
                },
            ]),
        }));
        let output = handler
            .handle(invocation(r#"{"user":"alice","limit":"10"}"#))
            .await
            .expect("tool execution should return output");
        match output {
            crate::tools::ToolOutput::Function {
                body: FunctionCallOutputBody::Json(value),
                success: Some(true),
            } => {
                assert_eq!(value["items"].as_array().map(Vec::len), Some(1));
                assert_eq!(value["items"][0]["user"], "alice");
            }
            _ => panic!("expected success output"),
        }
    }

    #[tokio::test]
    async fn provider_permission_error_maps_to_code() {
        let handler = ProcessSnapshotHandler::new(Arc::new(MockProvider {
            processes: Err(SystemToolError::PermissionDenied("blocked".to_string())),
        }));
        let output = handler
            .handle(invocation(r#"{}"#))
            .await
            .expect("tool execution should return output");
        match output {
            crate::tools::ToolOutput::Function {
                body: FunctionCallOutputBody::Json(value),
                success: Some(false),
            } => assert_eq!(value["error"]["code"], "permission_denied"),
            _ => panic!("expected permission_denied output"),
        }
    }
}
