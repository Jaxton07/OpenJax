use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

use crate::model::client::{ModelClient, ProviderAdapter};
use crate::model::registry::{ModelRegistry, RoutingPlan};
use crate::model::types::{ModelRequest, ModelResponse, ModelStage};

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

    fn build_attempt_chain(&self, primary: &str) -> Vec<String> {
        let mut out = vec![primary.to_string()];
        let mut seen = HashSet::from([primary.to_string()]);
        let mut idx = 0usize;
        while idx < out.len() {
            let current = out[idx].clone();
            let Some(next) = self.routing.fallbacks.get(&current) else {
                idx += 1;
                continue;
            };
            for candidate in next {
                if seen.insert(candidate.to_string()) {
                    out.push(candidate.to_string());
                    if out.len() > self.routing.max_fallback_chain {
                        return out;
                    }
                }
            }
            idx += 1;
        }
        out
    }

    async fn call_complete(&self, request: &ModelRequest) -> Result<ModelResponse> {
        let primary = self.route_for_stage(request.stage);
        let chain = self.build_attempt_chain(primary);
        let mut last_err = None;
        for (idx, model_id) in chain.iter().enumerate() {
            let Some(adapter) = self.adapters.get(model_id) else {
                warn!(
                    stage = request.stage.as_str(),
                    model_id = %model_id,
                    "model_router missing adapter for routed model"
                );
                continue;
            };
            if request.options.require_reasoning == Some(true) && !adapter.capabilities().reasoning
            {
                warn!(
                    stage = request.stage.as_str(),
                    model_id = %model_id,
                    backend = adapter.backend_name(),
                    provider = adapter.provider(),
                    protocol = adapter.protocol(),
                    attempt_index = idx + 1,
                    fallback_from = if idx == 0 { "" } else { primary },
                    "model_router adapter skipped due to missing reasoning capability"
                );
                continue;
            }
            let started = Instant::now();
            match adapter.complete(request).await {
                Ok(response) => {
                    info!(
                        stage = request.stage.as_str(),
                        model_id = %model_id,
                        backend = adapter.backend_name(),
                        provider = adapter.provider(),
                        protocol = adapter.protocol(),
                        attempt_index = idx + 1,
                        fallback_from = if idx == 0 { "" } else { primary },
                        latency_ms = started.elapsed().as_millis(),
                        "model_router attempt succeeded"
                    );
                    return Ok(response);
                }
                Err(err) => {
                    warn!(
                        stage = request.stage.as_str(),
                        model_id = %model_id,
                        backend = adapter.backend_name(),
                        provider = adapter.provider(),
                        protocol = adapter.protocol(),
                        attempt_index = idx + 1,
                        fallback_from = if idx == 0 { "" } else { primary },
                        latency_ms = started.elapsed().as_millis(),
                        error = %err,
                        "model_router attempt failed"
                    );
                    last_err = Some(err);
                }
            }
        }

        match last_err {
            Some(err) => Err(err),
            None => Err(anyhow!(
                "no available model adapter for stage={}",
                request.stage.as_str()
            )),
        }
    }

    async fn call_complete_stream(
        &self,
        request: &ModelRequest,
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse> {
        let primary = self.route_for_stage(request.stage);
        let chain = self.build_attempt_chain(primary);
        let mut last_err = None;
        for (idx, model_id) in chain.iter().enumerate() {
            let Some(adapter) = self.adapters.get(model_id) else {
                continue;
            };

            if !adapter.capabilities().stream {
                warn!(
                    stage = request.stage.as_str(),
                    model_id = %model_id,
                    backend = adapter.backend_name(),
                    provider = adapter.provider(),
                    protocol = adapter.protocol(),
                    attempt_index = idx + 1,
                    fallback_from = if idx == 0 { "" } else { primary },
                    "model_router adapter skipped due to missing stream capability"
                );
                continue;
            }

            let started = Instant::now();
            match adapter.complete_stream(request, delta_sender.clone()).await {
                Ok(response) => {
                    info!(
                        stage = request.stage.as_str(),
                        model_id = %model_id,
                        backend = adapter.backend_name(),
                        provider = adapter.provider(),
                        protocol = adapter.protocol(),
                        attempt_index = idx + 1,
                        fallback_from = if idx == 0 { "" } else { primary },
                        latency_ms = started.elapsed().as_millis(),
                        "model_router stream attempt succeeded"
                    );
                    return Ok(response);
                }
                Err(err) => {
                    warn!(
                        stage = request.stage.as_str(),
                        model_id = %model_id,
                        backend = adapter.backend_name(),
                        provider = adapter.provider(),
                        protocol = adapter.protocol(),
                        attempt_index = idx + 1,
                        fallback_from = if idx == 0 { "" } else { primary },
                        latency_ms = started.elapsed().as_millis(),
                        error = %err,
                        "model_router stream attempt failed"
                    );
                    last_err = Some(err);
                }
            }
        }

        match last_err {
            Some(err) => Err(err),
            None => Err(anyhow!(
                "no stream-capable model adapter for stage={}",
                request.stage.as_str()
            )),
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
        delta_sender: Option<UnboundedSender<String>>,
    ) -> Result<ModelResponse> {
        self.call_complete_stream(request, delta_sender).await
    }

    fn name(&self) -> &'static str {
        "model-router"
    }
}
