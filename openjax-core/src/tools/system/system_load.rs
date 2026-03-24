use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;

use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

use super::errors::SystemToolError;
use super::provider::{DefaultSystemMetricsProvider, SystemMetricsProvider};
use super::types::{
    CpuLoadRecord, ErrorBody, ErrorEnvelope, LoadAverageRecord, MemoryLoadRecord, SystemLoadArgs,
};

pub struct SystemLoadHandler {
    provider: Arc<dyn SystemMetricsProvider>,
}

impl SystemLoadHandler {
    pub fn new(provider: Arc<dyn SystemMetricsProvider>) -> Self {
        Self { provider }
    }
}

impl Default for SystemLoadHandler {
    fn default() -> Self {
        Self::new(Arc::new(DefaultSystemMetricsProvider))
    }
}

#[derive(Serialize)]
struct SystemLoadResponse {
    timestamp: String,
    cpu: Option<CpuSection>,
    memory: Option<MemorySection>,
    load_avg: LoadAvgSection,
}

#[derive(Serialize)]
struct CpuSection {
    logical_cores: usize,
    usage_pct: f32,
}

#[derive(Serialize)]
struct MemorySection {
    total_bytes: u64,
    used_bytes: u64,
    used_pct: f64,
    swap_total_bytes: u64,
    swap_used_bytes: u64,
}

#[derive(Serialize)]
struct LoadAvgSection {
    one: f64,
    five: f64,
    fifteen: f64,
}

#[async_trait]
impl ToolHandler for SystemLoadHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let arguments = match invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "system_load handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: SystemLoadArgs = match serde_json::from_str(&arguments) {
            Ok(parsed) => parsed,
            Err(err) => {
                return Ok(system_error_output(SystemToolError::InvalidArgument(
                    format!("failed to parse arguments: {err}"),
                )));
            }
        };

        let cpu = if args.include_cpu {
            let cpu = match self.provider.collect_cpu_load() {
                Ok(data) => data,
                Err(err) => return Ok(system_error_output(err)),
            };
            Some(to_cpu_section(cpu))
        } else {
            None
        };

        let memory = if args.include_memory {
            let memory = match self.provider.collect_memory_load() {
                Ok(data) => data,
                Err(err) => return Ok(system_error_output(err)),
            };
            Some(to_memory_section(memory))
        } else {
            None
        };

        let load = match self.provider.collect_load_average() {
            Ok(data) => data,
            Err(err) => return Ok(system_error_output(err)),
        };

        let response = SystemLoadResponse {
            timestamp: now_rfc3339(),
            cpu,
            memory,
            load_avg: to_load_avg_section(load),
        };

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Json(serde_json::to_value(response).map_err(|err| {
                FunctionCallError::Internal(format!("failed to serialize system load: {err}"))
            })?),
            success: Some(true),
        })
    }
}

fn to_cpu_section(cpu: CpuLoadRecord) -> CpuSection {
    CpuSection {
        logical_cores: cpu.logical_cores,
        usage_pct: cpu.usage_pct,
    }
}

fn to_memory_section(memory: MemoryLoadRecord) -> MemorySection {
    MemorySection {
        total_bytes: memory.total_bytes,
        used_bytes: memory.used_bytes,
        used_pct: memory.used_pct,
        swap_total_bytes: memory.swap_total_bytes,
        swap_used_bytes: memory.swap_used_bytes,
    }
}

fn to_load_avg_section(load: LoadAverageRecord) -> LoadAvgSection {
    LoadAvgSection {
        one: load.one,
        five: load.five,
        fifteen: load.fifteen,
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
    use super::{SystemLoadHandler, to_memory_section};
    use crate::tools::FunctionCallOutputBody;
    use crate::tools::context::{SandboxPolicy, ToolInvocation, ToolPayload, ToolTurnContext};
    use crate::tools::registry::ToolHandler;
    use crate::tools::system::errors::SystemToolError;
    use crate::tools::system::provider::SystemMetricsProvider;
    use crate::tools::system::types::{
        CpuLoadRecord, DiskUsageRecord, LoadAverageRecord, MemoryLoadRecord,
    };
    use std::sync::Arc;

    #[test]
    fn memory_section_preserves_percent() {
        let section = to_memory_section(MemoryLoadRecord {
            total_bytes: 100,
            used_bytes: 25,
            used_pct: 25.0,
            swap_total_bytes: 40,
            swap_used_bytes: 10,
        });
        assert_eq!(section.used_pct, 25.0);
    }

    struct MockProvider;

    impl SystemMetricsProvider for MockProvider {
        fn collect_processes(
            &self,
        ) -> Result<Vec<crate::tools::system::types::ProcessRecord>, SystemToolError> {
            Err(SystemToolError::NotSupported("unused".to_string()))
        }

        fn collect_cpu_load(&self) -> Result<CpuLoadRecord, SystemToolError> {
            Ok(CpuLoadRecord {
                logical_cores: 8,
                usage_pct: 23.5,
            })
        }

        fn collect_memory_load(&self) -> Result<MemoryLoadRecord, SystemToolError> {
            Ok(MemoryLoadRecord {
                total_bytes: 200,
                used_bytes: 50,
                used_pct: 25.0,
                swap_total_bytes: 100,
                swap_used_bytes: 10,
            })
        }

        fn collect_load_average(&self) -> Result<LoadAverageRecord, SystemToolError> {
            Ok(LoadAverageRecord {
                one: 0.1,
                five: 0.2,
                fifteen: 0.3,
            })
        }

        fn collect_disks(&self) -> Result<Vec<DiskUsageRecord>, SystemToolError> {
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
            tool_name: "system_load".to_string(),
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
    async fn include_cpu_false_omits_cpu_section() {
        let handler = SystemLoadHandler::new(Arc::new(MockProvider));
        let output = handler
            .handle(invocation(
                r#"{"include_cpu":"false","include_memory":"true"}"#,
            ))
            .await
            .expect("tool execution should return output");
        match output {
            crate::tools::ToolOutput::Function {
                body: FunctionCallOutputBody::Json(value),
                success: Some(true),
            } => {
                assert!(value["cpu"].is_null());
                assert!(value["memory"].is_object());
            }
            _ => panic!("expected success output"),
        }
    }
}
