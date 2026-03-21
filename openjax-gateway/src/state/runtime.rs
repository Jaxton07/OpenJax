//! Session runtime types and implementations.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use async_trait::async_trait;
use openjax_core::streaming::ReplayBuffer;
use openjax_core::{Agent, ApprovalHandler, ApprovalRequest, Config, approval_timeout_ms_from_env};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{info, warn};

use super::config::{event_channel_capacity, event_replay_limit};

static STREAM_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

pub fn gateway_stream_debug_enabled() -> bool {
    *STREAM_DEBUG_ENABLED.get_or_init(|| {
        std::env::var("OPENJAX_GATEWAY_STREAM_DEBUG")
            .ok()
            .map(|value| {
                let normalized = value.trim().to_ascii_lowercase();
                !(normalized == "0"
                    || normalized == "off"
                    || normalized == "false"
                    || normalized == "disabled")
            })
            .unwrap_or(false)
    })
}

pub fn log_preview(text: &str, max_chars: usize) -> (String, bool) {
    let mut iter = text.chars();
    let preview: String = iter.by_ref().take(max_chars).collect();
    let truncated = iter.next().is_some();
    (preview, truncated)
}

pub fn reasoning_preview(text: &str) -> (String, bool) {
    log_preview(text, 64)
}

// ---------------------------------------------------------------------------
// Session and Turn types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct TurnRuntime {
    pub status: TurnStatus,
    pub assistant_message: Option<String>,
    pub error: Option<ApiTurnError>,
}

impl TurnRuntime {
    pub fn queued() -> Self {
        Self {
            status: TurnStatus::Queued,
            assistant_message: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiTurnError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamEventEnvelope {
    pub request_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub event_seq: u64,
    #[serde(default)]
    pub turn_seq: u64,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub stream_source: String,
    pub payload: Value,
}

// ---------------------------------------------------------------------------
// SessionRuntime
// ---------------------------------------------------------------------------

pub struct SessionRuntime {
    pub agent: Arc<Mutex<Agent>>,
    pub approval_handler: Arc<GatewayApprovalHandler>,
    pub active_provider_id: Option<String>,
    pub status: SessionStatus,
    pub turns: HashMap<String, TurnRuntime>,
    pub core_turn_to_public: HashMap<u64, String>,
    pub next_event_seq: u64,
    pub turn_event_seq: HashMap<String, u64>,
    pub event_log: ReplayBuffer<StreamEventEnvelope>,
    pub event_tx: broadcast::Sender<StreamEventEnvelope>,
    pub resolved_approvals: HashSet<String>,
    last_event_emitted_at: Option<Instant>,
    replay_capacity: usize,
}

impl SessionRuntime {
    pub fn new_with_config(config: Config) -> Self {
        let mut agent = Agent::with_config(config);
        let approval_handler = Arc::new(GatewayApprovalHandler::default());
        agent.set_approval_handler(approval_handler.clone());
        let replay_capacity = event_replay_limit();
        let channel_capacity = event_channel_capacity();
        let (event_tx, _) = broadcast::channel(channel_capacity);
        Self {
            agent: Arc::new(Mutex::new(agent)),
            approval_handler,
            active_provider_id: None,
            status: SessionStatus::Active,
            turns: HashMap::new(),
            core_turn_to_public: HashMap::new(),
            next_event_seq: 1,
            turn_event_seq: HashMap::new(),
            event_log: ReplayBuffer::with_capacity(replay_capacity),
            event_tx,
            resolved_approvals: HashSet::new(),
            last_event_emitted_at: None,
            replay_capacity,
        }
    }

    pub fn clear_context(&mut self) {
        let mut agent = Agent::with_config(Config::load());
        let approval_handler = Arc::new(GatewayApprovalHandler::default());
        agent.set_approval_handler(approval_handler.clone());
        self.agent = Arc::new(Mutex::new(agent));
        self.approval_handler = approval_handler;
        self.active_provider_id = None;
        self.status = SessionStatus::Active;
        self.turns.clear();
        self.core_turn_to_public.clear();
        self.resolved_approvals.clear();
        self.turn_event_seq.clear();
        self.event_log = ReplayBuffer::with_capacity(self.replay_capacity);
        self.last_event_emitted_at = None;
    }

    pub fn clear_context_with_config(&mut self, config: Config) {
        let mut agent = Agent::with_config(config);
        let approval_handler = Arc::new(GatewayApprovalHandler::default());
        agent.set_approval_handler(approval_handler.clone());
        self.agent = Arc::new(Mutex::new(agent));
        self.approval_handler = approval_handler;
        self.status = SessionStatus::Active;
        self.turns.clear();
        self.core_turn_to_public.clear();
        self.resolved_approvals.clear();
        self.turn_event_seq.clear();
        self.event_log = ReplayBuffer::with_capacity(self.replay_capacity);
        self.last_event_emitted_at = None;
    }

    pub fn create_gateway_event(
        &mut self,
        request_id: &str,
        session_id: &str,
        turn_id: Option<String>,
        event_type: &str,
        payload: Value,
        stream_source: Option<&str>,
    ) -> StreamEventEnvelope {
        let turn_seq = if let Some(turn_id) = &turn_id {
            let seq = self.turn_event_seq.entry(turn_id.clone()).or_insert(0);
            *seq += 1;
            *seq
        } else {
            0
        };
        let event = StreamEventEnvelope {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            turn_id,
            event_seq: self.next_event_seq,
            turn_seq,
            timestamp: crate::error::now_rfc3339(),
            event_type: event_type.to_string(),
            stream_source: stream_source.unwrap_or("synthetic").to_string(),
            payload,
        };
        self.next_event_seq += 1;
        event
    }

    pub fn publish_event(&mut self, event: StreamEventEnvelope) {
        let _ = self.event_log.push(event.clone());
        let _ = self.event_tx.send(event);
    }

    pub fn replay_from(
        &self,
        after_event_seq: Option<u64>,
    ) -> Result<Vec<StreamEventEnvelope>, crate::error::ApiError> {
        let replay = self.event_log.replay_from(after_event_seq).map_err(|err| {
            crate::error::ApiError::invalid_argument(
                "replay point is outside retention window",
                json!({ "after_event_seq": err.requested_after_seq, "min_allowed": err.min_allowed }),
            )
        })?;
        Ok(replay.into_iter().map(|(_, event)| event).collect())
    }

    pub fn get_or_create_public_turn_id(&mut self, core_turn_id: u64) -> String {
        if let Some(id) = self.core_turn_to_public.get(&core_turn_id) {
            return id.clone();
        }
        let public_id = format!("turn_{}", core_turn_id);
        self.core_turn_to_public
            .insert(core_turn_id, public_id.clone());
        public_id
    }

    pub fn set_last_event_emitted_at(&mut self, ts: Option<Instant>) {
        self.last_event_emitted_at = ts;
    }

    pub fn get_last_event_emitted_at(&self) -> Option<Instant> {
        self.last_event_emitted_at
    }
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self::new_with_config(Config::load())
    }
}

// ---------------------------------------------------------------------------
// GatewayApprovalHandler
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct GatewayApprovalHandler {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl GatewayApprovalHandler {
    pub async fn resolve(&self, approval_id: &str, approved: bool) -> bool {
        let tx = self.pending.lock().await.remove(approval_id);
        match tx {
            Some(tx) => tx.send(approved).is_ok(),
            None => false,
        }
    }
}

#[async_trait]
impl ApprovalHandler for GatewayApprovalHandler {
    async fn request_approval(&self, request: ApprovalRequest) -> Result<bool, String> {
        let timeout_ms = approval_timeout_ms_from_env();
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(request.request_id.clone(), tx);
        info!(
            approval_id = %request.request_id,
            target = %request.target,
            "approval requested"
        );
        match timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(approved)) => Ok(approved),
            Ok(Err(_)) => Err("approval channel closed".to_string()),
            Err(_) => {
                warn!(approval_id = %request.request_id, "approval timed out");
                Err("approval timed out".to_string())
            }
        }
    }
}
