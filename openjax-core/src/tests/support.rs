use anyhow::Result;
use async_trait::async_trait;
use openjax_policy::runtime::PolicyRuntime;
use openjax_policy::schema::DecisionKind;
use openjax_policy::store::PolicyStore;
use openjax_protocol::Op;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{Duration, sleep};

use crate::approval::{ApprovalHandler, ApprovalRequest};
use crate::model::{
    AssistantContentBlock, ModelClient, ModelRequest, ModelResponse, StopReason, StreamDelta,
};
use crate::tools::context::{FunctionCallOutputBody, ToolOutput};
use crate::tools::error::FunctionCallError;
use crate::tools::registry::{ToolHandler, ToolKind};

pub(super) fn text_response(text: impl Into<String>) -> ModelResponse {
    ModelResponse {
        content: vec![AssistantContentBlock::Text { text: text.into() }],
        ..ModelResponse::default()
    }
}

#[derive(Clone)]
pub(super) struct ScriptedStreamingModel {
    complete_calls: Arc<Mutex<usize>>,
    stream_calls: Arc<Mutex<usize>>,
}

impl ScriptedStreamingModel {
    pub(super) fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(super) fn complete_call_count(&self) -> usize {
        *self.complete_calls.lock().expect("complete_calls lock")
    }

    pub(super) fn stream_call_count(&self) -> usize {
        *self.stream_calls.lock().expect("stream_calls lock")
    }
}

#[async_trait]
impl ModelClient for ScriptedStreamingModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::Text {
                text: "seed".to_string(),
            }],
            stop_reason: Some(StopReason::EndTurn),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut stream_calls = self.stream_calls.lock().expect("stream_calls lock");
        *stream_calls += 1;
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text("se".to_string()));
            let _ = sender.send(StreamDelta::Text("ed".to_string()));
        }
        let _ = request;
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::Text {
                text: "seed".to_string(),
            }],
            stop_reason: Some(StopReason::EndTurn),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "scripted-stream"
    }
}

#[derive(Clone)]
pub(super) struct ScriptedToolBatchModel {
    complete_calls: Arc<Mutex<usize>>,
}

impl ScriptedToolBatchModel {
    pub(super) fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for ScriptedToolBatchModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let response = if *calls == 1 {
            ModelResponse {
                content: vec![
                    AssistantContentBlock::ToolUse {
                        id: "call_1".to_string(),
                        name: "list_dir".to_string(),
                        input: serde_json::json!({"path": "."}),
                    },
                    AssistantContentBlock::ToolUse {
                        id: "call_2".to_string(),
                        name: "system_load".to_string(),
                        input: serde_json::json!({}),
                    },
                ],
                stop_reason: Some(StopReason::ToolUse),
                ..ModelResponse::default()
            }
        } else {
            ModelResponse {
                content: vec![AssistantContentBlock::Text {
                    text: "batch done".to_string(),
                }],
                stop_reason: Some(StopReason::EndTurn),
                ..ModelResponse::default()
            }
        };
        Ok(response)
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let response = if *calls == 1 {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::ToolUseStart {
                    id: "call_1".to_string(),
                    name: "list_dir".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolArgsDelta {
                    id: "call_1".to_string(),
                    delta: "{\"path\":\".\"}".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseEnd {
                    id: "call_1".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseStart {
                    id: "call_2".to_string(),
                    name: "system_load".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseEnd {
                    id: "call_2".to_string(),
                });
            }
            ModelResponse {
                content: vec![
                    AssistantContentBlock::ToolUse {
                        id: "call_1".to_string(),
                        name: "list_dir".to_string(),
                        input: serde_json::json!({"path": "."}),
                    },
                    AssistantContentBlock::ToolUse {
                        id: "call_2".to_string(),
                        name: "system_load".to_string(),
                        input: serde_json::json!({}),
                    },
                ],
                stop_reason: Some(StopReason::ToolUse),
                ..ModelResponse::default()
            }
        } else {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::Text("batch done".to_string()));
            }
            ModelResponse {
                content: vec![AssistantContentBlock::Text {
                    text: "batch done".to_string(),
                }],
                stop_reason: Some(StopReason::EndTurn),
                ..ModelResponse::default()
            }
        };
        let _ = request;
        Ok(response)
    }

    fn name(&self) -> &'static str {
        "scripted-tool-batch"
    }
}

#[derive(Clone)]
pub(super) struct ScriptedToolBatchDependencyModel {
    complete_calls: Arc<Mutex<usize>>,
}

impl ScriptedToolBatchDependencyModel {
    pub(super) fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for ScriptedToolBatchDependencyModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let response = if *calls == 1 {
            ModelResponse {
                content: vec![
                    AssistantContentBlock::ToolUse {
                        id: "call_1".to_string(),
                        name: "list_dir".to_string(),
                        input: serde_json::json!({"path": "."}),
                    },
                    AssistantContentBlock::ToolUse {
                        id: "call_2".to_string(),
                        name: "system_load".to_string(),
                        input: serde_json::json!({}),
                    },
                ],
                stop_reason: Some(StopReason::ToolUse),
                ..ModelResponse::default()
            }
        } else {
            ModelResponse {
                content: vec![AssistantContentBlock::Text {
                    text: "dependency done".to_string(),
                }],
                stop_reason: Some(StopReason::EndTurn),
                ..ModelResponse::default()
            }
        };
        Ok(response)
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        let response = if *calls == 1 {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::ToolUseStart {
                    id: "call_1".to_string(),
                    name: "list_dir".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseEnd {
                    id: "call_1".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseStart {
                    id: "call_2".to_string(),
                    name: "system_load".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseEnd {
                    id: "call_2".to_string(),
                });
            }
            ModelResponse {
                content: vec![
                    AssistantContentBlock::ToolUse {
                        id: "call_1".to_string(),
                        name: "list_dir".to_string(),
                        input: serde_json::json!({"path": "."}),
                    },
                    AssistantContentBlock::ToolUse {
                        id: "call_2".to_string(),
                        name: "system_load".to_string(),
                        input: serde_json::json!({}),
                    },
                ],
                stop_reason: Some(StopReason::ToolUse),
                ..ModelResponse::default()
            }
        } else {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::Text("dependency done".to_string()));
            }
            ModelResponse {
                content: vec![AssistantContentBlock::Text {
                    text: "dependency done".to_string(),
                }],
                stop_reason: Some(StopReason::EndTurn),
                ..ModelResponse::default()
            }
        };
        let _ = request;
        Ok(response)
    }

    fn name(&self) -> &'static str {
        "scripted-tool-batch-dependency"
    }
}

#[derive(Clone)]
pub(super) struct NativeStreamingFinalModel {
    complete_calls: Arc<Mutex<usize>>,
    stream_calls: Arc<Mutex<usize>>,
}

impl NativeStreamingFinalModel {
    pub(super) fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(super) fn complete_call_count(&self) -> usize {
        *self.complete_calls.lock().expect("complete_calls lock")
    }

    pub(super) fn stream_call_count(&self) -> usize {
        *self.stream_calls.lock().expect("stream_calls lock")
    }
}

#[async_trait]
impl ModelClient for NativeStreamingFinalModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        Ok(text_response("fallback final"))
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.stream_calls.lock().expect("stream_calls lock");
        *calls += 1;
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::Text("se".to_string()));
            let _ = sender.send(StreamDelta::Text("ed".to_string()));
        }
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::Text {
                text: "seed".to_string(),
            }],
            stop_reason: Some(StopReason::EndTurn),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "native-streaming-final"
    }
}

#[derive(Clone)]
pub(super) struct NativeStreamingToolUseModel {
    complete_calls: Arc<Mutex<usize>>,
    stream_calls: Arc<Mutex<usize>>,
}

impl NativeStreamingToolUseModel {
    pub(super) fn new() -> Self {
        Self {
            complete_calls: Arc::new(Mutex::new(0)),
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(super) fn complete_call_count(&self) -> usize {
        *self.complete_calls.lock().expect("complete_calls lock")
    }
}

#[async_trait]
impl ModelClient for NativeStreamingToolUseModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        let mut calls = self.complete_calls.lock().expect("complete_calls lock");
        *calls += 1;
        Ok(text_response("fallback final"))
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.stream_calls.lock().expect("stream_calls lock");
        *calls += 1;
        if *calls == 1 {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::ToolUseStart {
                    id: "call_1".to_string(),
                    name: "list_dir".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolArgsDelta {
                    id: "call_1".to_string(),
                    delta: "{\"path\":\".\"}".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseEnd {
                    id: "call_1".to_string(),
                });
            }
            Ok(ModelResponse {
                content: vec![AssistantContentBlock::ToolUse {
                    id: "call_1".to_string(),
                    name: "list_dir".to_string(),
                    input: serde_json::json!({"path": "."}),
                }],
                stop_reason: Some(StopReason::ToolUse),
                ..ModelResponse::default()
            })
        } else {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::Text("native tool done".to_string()));
            }
            Ok(ModelResponse {
                content: vec![AssistantContentBlock::Text {
                    text: "native tool done".to_string(),
                }],
                stop_reason: Some(StopReason::EndTurn),
                ..ModelResponse::default()
            })
        }
    }

    fn name(&self) -> &'static str {
        "native-streaming-tool-use"
    }
}

#[derive(Clone)]
pub(super) struct MixedTextToolUseModel {
    stream_calls: Arc<Mutex<usize>>,
}

impl MixedTextToolUseModel {
    pub(super) fn new() -> Self {
        Self {
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for MixedTextToolUseModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(text_response("unused"))
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.stream_calls.lock().expect("stream_calls lock");
        *calls += 1;
        if *calls == 1 {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::Text("working on it".to_string()));
                let _ = sender.send(StreamDelta::ToolUseStart {
                    id: "call_mix".to_string(),
                    name: "list_dir".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolArgsDelta {
                    id: "call_mix".to_string(),
                    delta: "{\"path\":\".\"}".to_string(),
                });
                let _ = sender.send(StreamDelta::ToolUseEnd {
                    id: "call_mix".to_string(),
                });
            }
            Ok(ModelResponse {
                content: vec![
                    AssistantContentBlock::Text {
                        text: "working on it".to_string(),
                    },
                    AssistantContentBlock::ToolUse {
                        id: "call_mix".to_string(),
                        name: "list_dir".to_string(),
                        input: serde_json::json!({"path": "."}),
                    },
                ],
                stop_reason: Some(StopReason::ToolUse),
                ..ModelResponse::default()
            })
        } else {
            if let Some(sender) = delta_sender {
                let _ = sender.send(StreamDelta::Text("done".to_string()));
            }
            Ok(ModelResponse {
                content: vec![AssistantContentBlock::Text {
                    text: "done".to_string(),
                }],
                stop_reason: Some(StopReason::EndTurn),
                ..ModelResponse::default()
            })
        }
    }

    fn name(&self) -> &'static str {
        "mixed-text-tool-use"
    }
}

#[derive(Clone)]
pub(super) struct OverflowToolUseModel {
    stream_calls: Arc<Mutex<usize>>,
}

impl OverflowToolUseModel {
    pub(super) fn new() -> Self {
        Self {
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ModelClient for OverflowToolUseModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(text_response("unused"))
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.stream_calls.lock().expect("stream_calls lock");
        *calls += 1;
        let tool_uses = (0..11)
            .map(|idx| {
                let id = format!("overflow_{idx}");
                let name = "list_dir".to_string();
                if let Some(sender) = delta_sender.as_ref() {
                    let _ = sender.send(StreamDelta::ToolUseStart {
                        id: id.clone(),
                        name: name.clone(),
                    });
                    let _ = sender.send(StreamDelta::ToolArgsDelta {
                        id: id.clone(),
                        delta: "{\"path\":\".\"}".to_string(),
                    });
                    let _ = sender.send(StreamDelta::ToolUseEnd { id: id.clone() });
                }
                AssistantContentBlock::ToolUse {
                    id,
                    name,
                    input: serde_json::json!({"path": "."}),
                }
            })
            .collect::<Vec<_>>();
        Ok(ModelResponse {
            content: tool_uses,
            stop_reason: Some(StopReason::ToolUse),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "overflow-tool-use"
    }
}

#[derive(Clone)]
pub(super) struct DuplicateToolLoopModel;

#[async_trait]
impl ModelClient for DuplicateToolLoopModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::ToolUse {
                id: "dup_call".to_string(),
                name: "shell".to_string(),
                input: serde_json::json!({"cmd": "echo hi"}),
            }],
            stop_reason: Some(StopReason::ToolUse),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::ToolUseStart {
                id: "dup_call".to_string(),
                name: "shell".to_string(),
            });
            let _ = sender.send(StreamDelta::ToolArgsDelta {
                id: "dup_call".to_string(),
                delta: "{\"cmd\":\"echo hi\"}".to_string(),
            });
            let _ = sender.send(StreamDelta::ToolUseEnd {
                id: "dup_call".to_string(),
            });
        }
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::ToolUse {
                id: "dup_call".to_string(),
                name: "shell".to_string(),
                input: serde_json::json!({"cmd": "echo hi"}),
            }],
            stop_reason: Some(StopReason::ToolUse),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "duplicate-tool-loop"
    }
}

#[derive(Clone)]
pub(super) struct ApprovalBlockedBatchModel {
    stream_calls: Arc<Mutex<usize>>,
}

impl ApprovalBlockedBatchModel {
    pub(super) fn new() -> Self {
        Self {
            stream_calls: Arc::new(Mutex::new(0)),
        }
    }

    pub(super) fn stream_call_count(&self) -> usize {
        *self.stream_calls.lock().expect("stream_calls lock")
    }
}

#[async_trait]
impl ModelClient for ApprovalBlockedBatchModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::Text {
                text: "should not be reached".to_string(),
            }],
            stop_reason: Some(StopReason::EndTurn),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let mut calls = self.stream_calls.lock().expect("stream_calls lock");
        *calls += 1;
        let response = if *calls == 1 {
            if let Some(sender) = delta_sender {
                for (id, name, delta) in [
                    ("call_1", "list_dir", Some("{\"path\":\".\"}")),
                    ("call_2", "system_load", None),
                    ("call_3", "list_dir", Some("{\"path\":\".\"}")),
                ] {
                    let _ = sender.send(StreamDelta::ToolUseStart {
                        id: id.to_string(),
                        name: name.to_string(),
                    });
                    if let Some(delta) = delta {
                        let _ = sender.send(StreamDelta::ToolArgsDelta {
                            id: id.to_string(),
                            delta: delta.to_string(),
                        });
                    }
                    let _ = sender.send(StreamDelta::ToolUseEnd {
                        id: id.to_string(),
                    });
                }
            }
            ModelResponse {
                content: vec![
                    AssistantContentBlock::ToolUse {
                        id: "call_1".to_string(),
                        name: "list_dir".to_string(),
                        input: serde_json::json!({"path": "."}),
                    },
                    AssistantContentBlock::ToolUse {
                        id: "call_2".to_string(),
                        name: "system_load".to_string(),
                        input: serde_json::json!({}),
                    },
                    AssistantContentBlock::ToolUse {
                        id: "call_3".to_string(),
                        name: "list_dir".to_string(),
                        input: serde_json::json!({"path": "."}),
                    },
                ],
                stop_reason: Some(StopReason::ToolUse),
                ..ModelResponse::default()
            }
        } else {
            ModelResponse {
                content: vec![AssistantContentBlock::Text {
                    text: "should not be reached".to_string(),
                }],
                stop_reason: Some(StopReason::EndTurn),
                ..ModelResponse::default()
            }
        };
        Ok(response)
    }

    fn name(&self) -> &'static str {
        "approval-blocked-batch"
    }
}

#[derive(Clone)]
pub(super) struct ApprovalCancellationBatchModel;

#[async_trait]
impl ModelClient for ApprovalCancellationBatchModel {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
        Ok(ModelResponse {
            content: vec![AssistantContentBlock::Text {
                text: "should not be reached".to_string(),
            }],
            stop_reason: Some(StopReason::EndTurn),
            ..ModelResponse::default()
        })
    }

    async fn complete_stream(
        &self,
        _request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        if let Some(sender) = delta_sender {
            let _ = sender.send(StreamDelta::ToolUseStart {
                id: "call_approve".to_string(),
                name: "edit_file_range".to_string(),
            });
            let _ = sender.send(StreamDelta::ToolArgsDelta {
                id: "call_approve".to_string(),
                delta: "{\"file_path\":\"todo.txt\",\"start_line\":\"1\",\"end_line\":\"1\",\"new_text\":\"x\"}".to_string(),
            });
            let _ = sender.send(StreamDelta::ToolUseEnd {
                id: "call_approve".to_string(),
            });
            let _ = sender.send(StreamDelta::ToolUseStart {
                id: "call_slow".to_string(),
                name: "system_load".to_string(),
            });
            let _ = sender.send(StreamDelta::ToolUseEnd {
                id: "call_slow".to_string(),
            });
        }
        Ok(ModelResponse {
            content: vec![
                AssistantContentBlock::ToolUse {
                    id: "call_approve".to_string(),
                    name: "edit_file_range".to_string(),
                    input: serde_json::json!({
                        "file_path": "todo.txt",
                        "start_line": "1",
                        "end_line": "1",
                        "new_text": "x"
                    }),
                },
                AssistantContentBlock::ToolUse {
                    id: "call_slow".to_string(),
                    name: "system_load".to_string(),
                    input: serde_json::json!({}),
                },
            ],
            stop_reason: Some(StopReason::ToolUse),
            ..ModelResponse::default()
        })
    }

    fn name(&self) -> &'static str {
        "approval-cancellation-batch"
    }
}

pub(super) struct SlowProbeTool {
    pub(super) marker_path: PathBuf,
}

#[async_trait]
impl ToolHandler for SlowProbeTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(
        &self,
        _invocation: crate::tools::context::ToolInvocation,
    ) -> Result<ToolOutput, FunctionCallError> {
        sleep(Duration::from_millis(250)).await;
        fs::write(&self.marker_path, "ran")
            .map_err(|e| FunctionCallError::Internal(format!("marker write failed: {e}")))?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text("slow probe done".to_string()),
            success: Some(true),
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct RejectApprovalHandler;

#[async_trait]
impl ApprovalHandler for RejectApprovalHandler {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(false)
    }
}

pub(super) fn ask_policy_runtime() -> PolicyRuntime {
    PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]))
}

pub(super) fn user_turn(input: impl Into<String>) -> Op {
    Op::UserTurn {
        input: input.into(),
    }
}
