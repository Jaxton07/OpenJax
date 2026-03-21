//! Event dispatch and AppState implementation.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use openjax_core::Config;
use openjax_protocol::Event;
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::load_api_keys_from_env;
use crate::auth::{AuthConfig, AuthService};
use crate::error::ApiError;
use crate::event_mapper::map_core_event_payload;
use openjax_store::SqliteStore;
use openjax_store::{ProviderRepository, SessionRepository};

use super::config::{
    build_runtime_config, gateway_db_path, map_store_error, migrate_providers_from_config_if_needed,
};
use super::runtime::{
    ApiTurnError, SessionRuntime, StreamEventEnvelope, TurnRuntime, TurnStatus,
    gateway_stream_debug_enabled, log_preview, reasoning_preview,
};

const AFTER_DISPATCH_LOG_TARGET: &str = "openjax_after_dispatcher";
const AFTER_DISPATCH_PREFIX: &str = "OPENJAX_AFTER_DISPATCH";

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<RwLock<HashMap<String, Arc<Mutex<SessionRuntime>>>>>,
    pub api_keys: Arc<HashSet<String>>,
    pub auth_service: Arc<AuthService>,
    pub store: Arc<SqliteStore>,
}

impl AppState {
    pub fn new() -> Self {
        Self::new_with_api_keys(load_api_keys_from_env())
    }

    pub fn new_with_api_keys(api_keys: HashSet<String>) -> Self {
        Self::try_new_with_api_keys(api_keys).expect("initialize gateway auth service")
    }

    pub fn try_new_with_api_keys(api_keys: HashSet<String>) -> anyhow::Result<Self> {
        let auth_config = AuthConfig::from_env();
        let auth_service = if auth_config.db_path.as_os_str() == ":memory:" {
            AuthService::for_test()?
        } else {
            AuthService::from_config(auth_config.clone())?
        };
        let db_path = gateway_db_path();
        let store = Arc::new(SqliteStore::open(&db_path)?);
        migrate_providers_from_config_if_needed(&store);
        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(api_keys),
            auth_service: Arc::new(auth_service),
            store,
        })
    }

    pub fn new_with_api_keys_for_test(api_keys: HashSet<String>) -> Self {
        let auth_service = AuthService::for_test().expect("initialize test auth service");
        let store = Arc::new(SqliteStore::open_memory().expect("initialize test store"));
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(api_keys),
            auth_service: Arc::new(auth_service),
            store,
        }
    }

    pub fn is_api_key_allowed(&self, key: &str) -> bool {
        self.api_keys.contains(key)
    }

    pub fn auth_service(&self) -> Arc<AuthService> {
        self.auth_service.clone()
    }

    pub async fn create_session(&self) -> Result<String, ApiError> {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        self.store
            .create_session(&session_id, None)
            .map_err(map_store_error)?;
        let runtime = self
            .build_session_runtime(&session_id)
            .map_err(map_store_error)?;
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), runtime);
        Ok(session_id)
    }

    pub async fn get_session(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<SessionRuntime>>, ApiError> {
        if let Some(runtime) = self.sessions.read().await.get(session_id).cloned() {
            return Ok(runtime);
        }
        let exists = self
            .store
            .get_session(session_id)
            .map_err(map_store_error)?
            .is_some();
        if !exists {
            return Err(ApiError::not_found(
                "session not found",
                json!({ "session_id": session_id }),
            ));
        }
        let runtime = self
            .build_session_runtime(session_id)
            .map_err(map_store_error)?;
        let mut guard = self.sessions.write().await;
        if let Some(existing) = guard.get(session_id).cloned() {
            return Ok(existing);
        }
        guard.insert(session_id.to_string(), runtime.clone());
        Ok(runtime)
    }

    pub async fn remove_session(&self, session_id: &str) -> Option<Arc<Mutex<SessionRuntime>>> {
        self.sessions.write().await.remove(session_id)
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<(), ApiError> {
        self.sessions.write().await.remove(session_id);
        self.store
            .delete_session(session_id)
            .map_err(map_store_error)?;
        Ok(())
    }

    pub fn list_persisted_sessions(&self) -> Result<Vec<openjax_store::SessionRecord>, ApiError> {
        self.store.list_sessions().map_err(map_store_error)
    }

    pub fn list_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<openjax_store::MessageRecord>, ApiError> {
        self.store
            .list_messages(session_id)
            .map_err(map_store_error)
    }

    pub fn list_session_events(
        &self,
        session_id: &str,
        after_event_seq: Option<u64>,
    ) -> Result<Vec<openjax_store::EventRecord>, ApiError> {
        self.store
            .list_events(session_id, after_event_seq)
            .map_err(map_store_error)
    }

    pub fn append_message(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        role: &str,
        content: &str,
    ) -> Result<(), ApiError> {
        self.store
            .append_message(session_id, turn_id, role, content)
            .map_err(map_store_error)?;
        Ok(())
    }

    pub fn append_event(&self, event: &StreamEventEnvelope) -> Result<(), ApiError> {
        let payload_json = serde_json::to_string(&event.payload)
            .map_err(|err| ApiError::internal(err.to_string()))?;
        self.store
            .append_event(
                &event.session_id,
                event.event_seq,
                event.turn_seq,
                event.turn_id.as_deref(),
                &event.event_type,
                &payload_json,
                &event.timestamp,
                &event.stream_source,
            )
            .map_err(map_store_error)?;
        Ok(())
    }

    pub fn list_providers(&self) -> Result<Vec<openjax_store::ProviderRecord>, ApiError> {
        self.store.list_providers().map_err(map_store_error)
    }

    pub fn get_active_provider(
        &self,
    ) -> Result<Option<openjax_store::ActiveProviderRecord>, ApiError> {
        self.store.get_active_provider().map_err(map_store_error)
    }

    pub fn set_active_provider(
        &self,
        provider_id: &str,
    ) -> Result<Option<openjax_store::ActiveProviderRecord>, ApiError> {
        self.store
            .set_active_provider(provider_id)
            .map_err(map_store_error)
    }

    pub fn create_provider(
        &self,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: &str,
        provider_type: &str,
        context_window_size: u32,
    ) -> Result<openjax_store::ProviderRecord, ApiError> {
        self.store
            .create_provider(
                provider_name,
                base_url,
                model_name,
                api_key,
                provider_type,
                context_window_size,
            )
            .map_err(map_store_error)
    }

    pub fn update_provider(
        &self,
        provider_id: &str,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: Option<&str>,
        context_window_size: u32,
    ) -> Result<Option<openjax_store::ProviderRecord>, ApiError> {
        self.store
            .update_provider(
                provider_id,
                provider_name,
                base_url,
                model_name,
                api_key,
                context_window_size,
            )
            .map_err(map_store_error)
    }

    pub fn delete_provider(&self, provider_id: &str) -> Result<bool, ApiError> {
        self.store
            .delete_provider(provider_id)
            .map_err(map_store_error)
    }

    pub fn runtime_config(&self) -> Config {
        let providers = self.store.list_providers().unwrap_or_default();
        let active_provider_id = self
            .store
            .get_active_provider()
            .ok()
            .flatten()
            .map(|item| item.provider_id);
        build_runtime_config(providers, active_provider_id.as_deref())
    }

    fn build_session_runtime(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Arc<Mutex<SessionRuntime>>> {
        let providers = self.store.list_providers().unwrap_or_default();
        let active_provider_id = self
            .store
            .get_active_provider()?
            .map(|item| item.provider_id);
        let config = build_runtime_config(providers, active_provider_id.as_deref());
        let mut runtime = SessionRuntime::new_with_config(config);
        runtime.active_provider_id = active_provider_id;
        if let Some(max_event_seq) = self.store.last_event_seq(session_id)? {
            runtime.next_event_seq = max_event_seq.saturating_add(1);
        }
        for (turn_id, seq) in self.store.last_turn_seq_by_turn(session_id)? {
            runtime.turn_event_seq.insert(turn_id, seq);
        }
        let events = self.store.list_events(session_id, None)?;
        for row in events {
            let payload =
                serde_json::from_str::<Value>(&row.payload_json).unwrap_or_else(|_| json!({}));
            let envelope = StreamEventEnvelope {
                request_id: format!("req_replay_{}", row.id),
                session_id: row.session_id.clone(),
                turn_id: row.turn_id.clone(),
                event_seq: row.event_seq,
                turn_seq: row.turn_seq,
                timestamp: row.timestamp.clone(),
                event_type: row.event_type.clone(),
                stream_source: row.stream_source.clone(),
                payload: payload.clone(),
            };
            let _ = runtime.event_log.push(envelope);
            if let Some(turn_id) = row.turn_id {
                let turn = runtime
                    .turns
                    .entry(turn_id)
                    .or_insert_with(TurnRuntime::queued);
                apply_turn_runtime_event(turn, &row.event_type, &payload);
            }
        }
        Ok(Arc::new(Mutex::new(runtime)))
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Turn task
// ---------------------------------------------------------------------------

pub async fn run_turn_task(
    app_state: AppState,
    session_runtime: Arc<Mutex<SessionRuntime>>,
    session_id: String,
    request_id: String,
    input: String,
    turn_id_tx: oneshot::Sender<Result<String, ApiError>>,
) {
    let (event_sink_tx, mut event_sink_rx) = mpsc::unbounded_channel();
    let agent = {
        let guard = session_runtime.lock().await;
        guard.agent.clone()
    };

    let submit_task = tokio::spawn(async move {
        let mut guard = agent.lock().await;
        guard
            .submit_with_sink(openjax_protocol::Op::UserTurn { input }, event_sink_tx)
            .await
    });

    let mut sent_turn_id = false;
    let mut pending_turn_id_tx = Some(turn_id_tx);
    while let Some(event) = event_sink_rx.recv().await {
        let mapped = {
            let mut session = session_runtime.lock().await;
            map_core_event(
                &app_state,
                &mut session,
                &session_id,
                &request_id,
                event,
                &mut pending_turn_id_tx,
            )
        };
        if mapped.is_some() {
            sent_turn_id = true;
        }
    }

    match submit_task.await {
        Ok(events) => {
            if !sent_turn_id {
                let mut session = session_runtime.lock().await;
                if let Some(core_turn_id) = first_turn_id(&events) {
                    let public_turn_id = session.get_or_create_public_turn_id(core_turn_id);
                    if let Some(tx) = pending_turn_id_tx.take() {
                        let _ = tx.send(Ok(public_turn_id));
                    }
                } else if let Some(tx) = pending_turn_id_tx.take() {
                    let _ = tx.send(Err(ApiError::internal("failed to infer turn id")));
                }
            }
        }
        Err(_) => {
            if let Some(tx) = pending_turn_id_tx.take() {
                let _ = tx.send(Err(ApiError::upstream_unavailable(
                    "core execution task failed",
                    json!({}),
                )));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Event mapping
// ---------------------------------------------------------------------------

fn apply_turn_runtime_event(turn: &mut TurnRuntime, event_type: &str, payload: &Value) {
    if event_type == "turn_started" {
        turn.status = TurnStatus::Running;
        return;
    }
    if event_type == "turn_completed" {
        if !matches!(turn.status, TurnStatus::Failed) {
            turn.status = TurnStatus::Completed;
        }
        return;
    }
    if event_type == "response_error" || event_type == "error" {
        turn.status = TurnStatus::Failed;
        turn.error = Some(ApiTurnError {
            code: payload
                .get("code")
                .and_then(|value| value.as_str())
                .unwrap_or("UPSTREAM_ERROR")
                .to_string(),
            message: payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("response failed")
                .to_string(),
            retryable: payload
                .get("retryable")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            details: payload.clone(),
        });
        return;
    }
    if (event_type == "response_completed" || event_type == "assistant_message")
        && let Some(content) = payload.get("content").and_then(|value| value.as_str())
    {
        turn.assistant_message = Some(content.to_string());
        turn.status = TurnStatus::Completed;
    }
}

fn publish_and_persist_event(
    app_state: &AppState,
    session: &mut SessionRuntime,
    event: StreamEventEnvelope,
) {
    if let Err(err) = app_state.append_event(&event) {
        warn!(
            session_id = %event.session_id,
            event_seq = event.event_seq,
            event_type = %event.event_type,
            error = %err.message,
            "failed to persist event"
        );
    }
    session.publish_event(event);
}

fn map_core_event(
    app_state: &AppState,
    session: &mut SessionRuntime,
    session_id: &str,
    request_id: &str,
    event: Event,
    turn_id_tx: &mut Option<oneshot::Sender<Result<String, ApiError>>>,
) -> Option<String> {
    let mapping = map_core_event_payload(&event)?;
    let core_turn_id = mapping.core_turn_id;
    let event_type = mapping.event_type;
    let payload = mapping.payload;
    let stream_source = mapping.stream_source;

    let public_turn_id = core_turn_id.map(|tid| session.get_or_create_public_turn_id(tid));
    if let Some(turn_id) = &public_turn_id {
        let turn = session
            .turns
            .entry(turn_id.clone())
            .or_insert_with(TurnRuntime::queued);
        apply_turn_runtime_event(turn, event_type, &payload);
        if event_type == "response_completed"
            && let Some(content) = payload.get("content").and_then(|value| value.as_str())
        {
            turn.assistant_message = Some(content.to_string());
            let _ = app_state.append_message(session_id, Some(turn_id), "assistant", content);
        }
    }

    if let Some(turn_id) = public_turn_id.clone()
        && let Some(tx) = turn_id_tx.take()
    {
        let _ = tx.send(Ok(turn_id));
    }

    let envelope = session.create_gateway_event(
        request_id,
        session_id,
        public_turn_id.clone(),
        event_type,
        payload,
        stream_source,
    );
    if event_type == "reasoning_delta" {
        let delta_raw = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let (delta_preview, delta_preview_truncated) = reasoning_preview(delta_raw);
        info!(
            target: AFTER_DISPATCH_LOG_TARGET,
            session_id = %session_id,
            turn_id = ?public_turn_id,
            flow_prefix = AFTER_DISPATCH_PREFIX,
            flow_node = "gateway.reasoning.publish",
            flow_route = "reasoning_delta",
            flow_next = "frontend.reasoning_delta",
            event_seq = envelope.event_seq,
            turn_seq = envelope.turn_seq,
            delta_len = delta_raw.chars().count(),
            delta_preview = %delta_preview,
            delta_preview_truncated = delta_preview_truncated,
            "after_dispatcher_trace"
        );
    }
    if gateway_stream_debug_enabled()
        && matches!(
            event_type,
            "response_started"
                | "response_text_delta"
                | "response_completed"
                | "turn_completed"
                | "response_error"
        )
    {
        let delta_raw = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str());
        let content_raw = envelope
            .payload
            .get("content")
            .and_then(|value| value.as_str());
        let delta_len = envelope
            .payload
            .get("content_delta")
            .and_then(|value| value.as_str())
            .map(|value| value.len());
        let content_len = envelope
            .payload
            .get("content")
            .and_then(|value| value.as_str())
            .map(|value| value.len());
        let (delta_preview, delta_preview_truncated) = delta_raw
            .map(|value| log_preview(value, 24))
            .map(|(preview, truncated)| (Some(preview), Some(truncated)))
            .unwrap_or((None, None));
        let (content_preview, content_preview_truncated) = content_raw
            .map(|value| log_preview(value, 80))
            .map(|(preview, truncated)| (Some(preview), Some(truncated)))
            .unwrap_or((None, None));
        let assistant_len = public_turn_id
            .as_ref()
            .and_then(|turn_id| session.turns.get(turn_id))
            .and_then(|turn| turn.assistant_message.as_ref())
            .map(|value| value.len());
        let event_gap_ms = session
            .get_last_event_emitted_at()
            .map(|ts: std::time::Instant| ts.elapsed().as_millis() as u64);
        info!(
            session_id = %session_id,
            turn_id = ?public_turn_id,
            event_type = event_type,
            event_seq = envelope.event_seq,
            turn_seq = envelope.turn_seq,
            stream_source = %envelope.stream_source,
            delta_len = ?delta_len,
            delta_preview = ?delta_preview,
            delta_preview_truncated = ?delta_preview_truncated,
            content_len = ?content_len,
            content_preview = ?content_preview,
            content_preview_truncated = ?content_preview_truncated,
            assistant_message_len = ?assistant_len,
            event_gap_ms = ?event_gap_ms,
            "stream_debug.gateway_event_emitted"
        );
    }
    if (event_type == "tool_call_started"
        || event_type == "tool_call_ready"
        || event_type == "tool_call_completed")
        && envelope.payload.get("tool_call_id").is_some()
    {
        info!(
            event_type = event_type,
            turn_id = ?public_turn_id,
            tool_call_id = ?envelope.payload.get("tool_call_id").and_then(|v| v.as_str()),
            "tool event mapped"
        );
    }
    session.set_last_event_emitted_at(Some(std::time::Instant::now()));
    publish_and_persist_event(app_state, session, envelope);

    public_turn_id
}

fn first_turn_id(events: &[Event]) -> Option<u64> {
    for event in events {
        match event {
            Event::TurnStarted { turn_id }
            | Event::ToolCallStarted { turn_id, .. }
            | Event::ToolCallCompleted { turn_id, .. }
            | Event::ToolCallArgsDelta { turn_id, .. }
            | Event::ToolCallReady { turn_id, .. }
            | Event::ToolCallProgress { turn_id, .. }
            | Event::ToolCallFailed { turn_id, .. }
            | Event::AssistantMessage { turn_id, .. }
            | Event::ResponseStarted { turn_id, .. }
            | Event::ResponseTextDelta { turn_id, .. }
            | Event::ReasoningDelta { turn_id, .. }
            | Event::ToolCallsProposed { turn_id, .. }
            | Event::ToolBatchCompleted { turn_id, .. }
            | Event::ResponseResumed { turn_id, .. }
            | Event::ResponseCompleted { turn_id, .. }
            | Event::ResponseError { turn_id, .. }
            | Event::ApprovalRequested { turn_id, .. }
            | Event::ApprovalResolved { turn_id, .. }
            | Event::LoopWarning { turn_id, .. }
            | Event::TurnCompleted { turn_id } => return Some(*turn_id),
            Event::AgentSpawned { .. }
            | Event::AgentStatusChanged { .. }
            | Event::ContextUsageUpdated { .. }
            | Event::ContextCompacted { .. }
            | Event::ShutdownComplete => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use openjax_core::provider_protocol;
    use std::collections::HashSet;

    #[test]
    fn turn_status_remains_failed_after_turn_completed() {
        let app_state = AppState::new_with_api_keys_for_test(HashSet::new());
        let mut session = SessionRuntime::default();
        let mut turn_id_tx = None;
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::ResponseError {
                turn_id: 1,
                code: "ERR".to_string(),
                message: "failed".to_string(),
                retryable: false,
            },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::TurnCompleted { turn_id: 1 },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert_eq!(turn.status, TurnStatus::Failed);
    }

    #[test]
    fn turn_message_only_updates_from_response_completed() {
        let app_state = AppState::new_with_api_keys_for_test(HashSet::new());
        let mut session = SessionRuntime::default();
        let mut turn_id_tx = None;
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::TurnStarted { turn_id: 1 },
            &mut turn_id_tx,
        );
        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::AssistantMessage {
                turn_id: 1,
                content: "legacy".to_string(),
            },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert!(turn.assistant_message.is_none());

        let _ = map_core_event(
            &app_state,
            &mut session,
            "sess_1",
            "req_1",
            Event::ResponseCompleted {
                turn_id: 1,
                content: "final".to_string(),
                stream_source: openjax_protocol::StreamSource::Synthetic,
            },
            &mut turn_id_tx,
        );
        let turn = session.turns.get("turn_1").expect("turn exists");
        assert_eq!(turn.assistant_message.as_deref(), Some("final"));
    }

    #[test]
    fn first_turn_id_supports_reasoning_delta() {
        let turn_id = first_turn_id(&[Event::ReasoningDelta {
            turn_id: 7,
            content_delta: "thinking".to_string(),
            stream_source: openjax_protocol::StreamSource::ModelLive,
        }]);
        assert_eq!(turn_id, Some(7));
    }

    #[test]
    fn provider_protocol_defaults_to_chat_completions_for_glm_style_base_url() {
        let protocol = provider_protocol("https://open.bigmodel.cn/api/paas/v4", "glm-main");
        assert_eq!(protocol, "chat_completions");
    }
}
