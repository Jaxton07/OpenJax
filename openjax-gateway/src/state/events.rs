//! AppState and session persistence orchestration.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use openjax_core::Config;
use serde_json::{Value, json};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::auth::load_api_keys_from_env;
use crate::auth::{AuthConfig, AuthService};
use crate::error::ApiError;
use openjax_store::SqliteStore;
use openjax_store::repository::{CreateProviderParams, UpdateProviderParams};
use openjax_store::{ProviderRepository, SessionRepository};

use super::config::{
    build_runtime_config, gateway_db_path, gateway_policy_state, map_store_error,
    migrate_providers_from_config_if_needed,
};
use super::core_projection::apply_turn_runtime_event;
use super::runtime::{SessionRuntime, StreamEventEnvelope, TurnRuntime};

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
        {
            let policy_state = gateway_policy_state(&self.store);
            let guard = match policy_state.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.runtime.clear_session_overlay(session_id);
        }
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
        params: CreateProviderParams<'_>,
    ) -> Result<openjax_store::ProviderRecord, ApiError> {
        let store = self.store.as_ref();
        <SqliteStore as ProviderRepository>::create_provider(store, params).map_err(map_store_error)
    }

    pub fn update_provider(
        &self,
        params: UpdateProviderParams<'_>,
    ) -> Result<Option<openjax_store::ProviderRecord>, ApiError> {
        let store = self.store.as_ref();
        <SqliteStore as ProviderRepository>::update_provider(store, params).map_err(map_store_error)
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
