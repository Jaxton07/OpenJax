use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

use crate::model::client::{ModelClient, ProviderAdapter};
use crate::model::registry::{ModelRegistry, RoutingPlan};
use crate::model::types::{ModelRequest, ModelResponse, ModelStage, StreamDelta};

pub struct ModelRouter {
    adapters: HashMap<String, Arc<dyn ProviderAdapter>>,
    routing: RoutingPlan,
}

impl ModelRouter {
    pub fn new(registry: ModelRegistry, adapters: Vec<Arc<dyn ProviderAdapter>>) -> Self {
        let adapters = adapters
            .into_iter()
            .map(|adapter| (adapter.model_id().to_string(), adapter))
            .collect::<HashMap<_, _>>();
        Self {
            adapters,
            routing: registry.routing,
        }
    }

    fn route_for_stage(&self, stage: ModelStage) -> &str {
        match stage {
            ModelStage::Planner => &self.routing.planner,
            ModelStage::FinalWriter => &self.routing.final_writer,
            ModelStage::ToolReasoning => &self.routing.tool_reasoning,
        }
    }

    async fn call_complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let primary = self.route_for_stage(request.stage);
        let Some(adapter) = self.adapters.get(primary) else {
            return Err(anyhow!(
                "no adapter found for routed model stage={} model_id={primary}",
                request.stage.as_str(),
            ));
        };

        if request.options.require_reasoning == Some(true) && !adapter.capabilities().reasoning {
            return Err(anyhow!(
                "routed model does not support required reasoning capability: stage={} model_id={primary}",
                request.stage.as_str(),
            ));
        }

        let started = Instant::now();
        match adapter.complete(request).await {
            Ok(response) => {
                info!(
                    stage = request.stage.as_str(),
                    model_id = primary,
                    backend = adapter.backend_name(),
                    provider = adapter.provider(),
                    protocol = adapter.protocol(),
                    attempt_index = 1,
                    fallback_from = "",
                    latency_ms = started.elapsed().as_millis(),
                    "model_router attempt succeeded"
                );
                Ok(response)
            }
            Err(err) => {
                warn!(
                    stage = request.stage.as_str(),
                    model_id = primary,
                    backend = adapter.backend_name(),
                    provider = adapter.provider(),
                    protocol = adapter.protocol(),
                    attempt_index = 1,
                    fallback_from = "",
                    latency_ms = started.elapsed().as_millis(),
                    error = %err,
                    "model_router attempt failed"
                );
                Err(err)
            }
        }
    }

    async fn call_complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        let primary = self.route_for_stage(request.stage);
        let Some(adapter) = self.adapters.get(primary) else {
            return Err(anyhow!(
                "no adapter found for routed model stage={} model_id={primary}",
                request.stage.as_str(),
            ));
        };

        if !adapter.capabilities().stream {
            return Err(anyhow!(
                "routed model does not support streaming: stage={} model_id={primary}",
                request.stage.as_str(),
            ));
        }

        let started = Instant::now();
        match adapter.complete_stream(request, delta_sender).await {
            Ok(response) => {
                info!(
                    stage = request.stage.as_str(),
                    model_id = primary,
                    backend = adapter.backend_name(),
                    provider = adapter.provider(),
                    protocol = adapter.protocol(),
                    attempt_index = 1,
                    fallback_from = "",
                    latency_ms = started.elapsed().as_millis(),
                    "model_router stream attempt succeeded"
                );
                Ok(response)
            }
            Err(err) => {
                warn!(
                    stage = request.stage.as_str(),
                    model_id = primary,
                    backend = adapter.backend_name(),
                    provider = adapter.provider(),
                    protocol = adapter.protocol(),
                    attempt_index = 1,
                    fallback_from = "",
                    latency_ms = started.elapsed().as_millis(),
                    error = %err,
                    "model_router stream attempt failed"
                );
                Err(err)
            }
        }
    }
}

#[async_trait]
impl ModelClient for ModelRouter {
    async fn complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        self.call_complete(request).await
    }

    async fn complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<StreamDelta>>,
    ) -> Result<ModelResponse> {
        self.call_complete_stream(request, delta_sender).await
    }

    fn name(&self) -> &'static str {
        "model-router"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use tokio::sync::mpsc::UnboundedSender;

    use crate::model::client::{ModelClient, ProviderAdapter};
    use crate::model::registry::{ModelRegistry, RegisteredModel, RoutingPlan};
    use crate::model::types::{
        CapabilityFlags, ModelRequest, ModelResponse, ModelStage, StreamDelta,
    };

    use super::ModelRouter;

    #[derive(Clone)]
    struct MockAdapter {
        id: String,
        complete_calls: Arc<AtomicUsize>,
        stream_calls: Arc<AtomicUsize>,
        complete_err: Option<String>,
        stream_err: Option<String>,
        capabilities: CapabilityFlags,
    }

    impl MockAdapter {
        fn failing(id: &str, complete_err: &str, stream_err: &str) -> Self {
            Self {
                id: id.to_string(),
                complete_calls: Arc::new(AtomicUsize::new(0)),
                stream_calls: Arc::new(AtomicUsize::new(0)),
                complete_err: Some(complete_err.to_string()),
                stream_err: Some(stream_err.to_string()),
                capabilities: CapabilityFlags {
                    stream: true,
                    reasoning: true,
                    tool_call: false,
                    json_mode: false,
                },
            }
        }

        fn successful(id: &str) -> Self {
            Self {
                id: id.to_string(),
                complete_calls: Arc::new(AtomicUsize::new(0)),
                stream_calls: Arc::new(AtomicUsize::new(0)),
                complete_err: None,
                stream_err: None,
                capabilities: CapabilityFlags {
                    stream: true,
                    reasoning: true,
                    tool_call: false,
                    json_mode: false,
                },
            }
        }
    }

    #[async_trait]
    impl ProviderAdapter for MockAdapter {
        async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse> {
            self.complete_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(err) = &self.complete_err {
                Err(anyhow!(err.clone()))
            } else {
                Ok(ModelResponse {
                    text: "ok".to_string(),
                    ..Default::default()
                })
            }
        }

        async fn complete_stream(
            &self,
            _request: &ModelRequest,
            _delta_sender: Option<UnboundedSender<StreamDelta>>,
        ) -> Result<ModelResponse> {
            self.stream_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(err) = &self.stream_err {
                Err(anyhow!(err.clone()))
            } else {
                Ok(ModelResponse {
                    text: "ok".to_string(),
                    ..Default::default()
                })
            }
        }

        fn backend_name(&self) -> &'static str {
            "mock-backend"
        }

        fn model_id(&self) -> &str {
            &self.id
        }

        fn provider(&self) -> &str {
            "mock"
        }

        fn protocol(&self) -> &str {
            "chat_completions"
        }

        fn capabilities(&self) -> CapabilityFlags {
            self.capabilities
        }
    }

    fn build_registry_with_fallback() -> ModelRegistry {
        let mut models = HashMap::new();
        models.insert(
            "primary".to_string(),
            RegisteredModel {
                id: "primary".to_string(),
                provider: "mock".to_string(),
                protocol: "chat_completions".to_string(),
                model: "m1".to_string(),
                base_url: None,
                api_key: None,
                anthropic_version: None,
                thinking_budget_tokens: None,
                capabilities: CapabilityFlags {
                    stream: true,
                    reasoning: true,
                    tool_call: false,
                    json_mode: false,
                },
            },
        );
        models.insert(
            "backup".to_string(),
            RegisteredModel {
                id: "backup".to_string(),
                provider: "mock".to_string(),
                protocol: "chat_completions".to_string(),
                model: "m2".to_string(),
                base_url: None,
                api_key: None,
                anthropic_version: None,
                thinking_budget_tokens: None,
                capabilities: CapabilityFlags {
                    stream: true,
                    reasoning: true,
                    tool_call: false,
                    json_mode: false,
                },
            },
        );
        let mut fallbacks = HashMap::new();
        fallbacks.insert("primary".to_string(), vec!["backup".to_string()]);
        ModelRegistry {
            models,
            routing: RoutingPlan {
                planner: "primary".to_string(),
                final_writer: "primary".to_string(),
                tool_reasoning: "primary".to_string(),
                fallbacks,
                max_fallback_chain: 2,
            },
            used_legacy_bridge: false,
            has_legacy_fields: false,
        }
    }

    #[tokio::test]
    async fn complete_does_not_fallback_when_primary_fails() {
        let primary = MockAdapter::failing("primary", "primary failed", "stream failed");
        let backup = MockAdapter::successful("backup");
        let primary_calls = primary.complete_calls.clone();
        let backup_calls = backup.complete_calls.clone();
        let router = ModelRouter::new(
            build_registry_with_fallback(),
            vec![Arc::new(primary), Arc::new(backup)],
        );
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        let err = router
            .complete(&request)
            .await
            .expect_err("primary error should be surfaced");
        assert!(
            err.to_string().contains("primary failed"),
            "expected primary error, got: {err}"
        );
        assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backup_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn complete_stream_does_not_fallback_when_primary_fails() {
        let primary = MockAdapter::failing("primary", "complete failed", "stream primary failed");
        let backup = MockAdapter::successful("backup");
        let primary_calls = primary.stream_calls.clone();
        let backup_calls = backup.stream_calls.clone();
        let router = ModelRouter::new(
            build_registry_with_fallback(),
            vec![Arc::new(primary), Arc::new(backup)],
        );
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        let err = router
            .complete_stream(&request, None)
            .await
            .expect_err("primary stream error should be surfaced");
        assert!(
            err.to_string().contains("stream primary failed"),
            "expected primary stream error, got: {err}"
        );
        assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
        assert_eq!(backup_calls.load(Ordering::SeqCst), 0);
    }
}
