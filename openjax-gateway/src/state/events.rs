//! AppState and session persistence orchestration.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Mutex as StdMutex;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use openjax_core::Config;
use serde_json::json;
use tokio::sync::{Mutex, RwLock};
use tracing::warn;
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
use crate::transcript::{IndexSessionEntry, SessionIndexStore, TranscriptRecord, TranscriptStore};

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<RwLock<HashMap<String, Arc<Mutex<SessionRuntime>>>>>,
    pub api_keys: Arc<HashSet<String>>,
    pub auth_service: Arc<AuthService>,
    pub store: Arc<SqliteStore>,
    pub transcript: Arc<TranscriptStore>,
    pub session_index: Arc<SessionIndexStore>,
    auto_title_dedupe: Arc<StdMutex<AutoTitleDedupeCache>>,
}

pub const AUTO_TITLE_DEDUPE_TTL_MS: u64 = 30_000;
pub const AUTO_TITLE_DEDUPE_MAX_KEYS: usize = 10_000;

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
        let transcript_root = db_path
            .parent()
            .map(|parent| parent.join("transcripts"))
            .unwrap_or_else(|| std::env::temp_dir().join("openjax-transcripts"));
        let transcript = Arc::new(TranscriptStore::new(transcript_root));
        let session_index = Arc::new(SessionIndexStore::new(transcript.root())?);
        migrate_providers_from_config_if_needed(&store);
        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(api_keys),
            auth_service: Arc::new(auth_service),
            store,
            transcript,
            session_index,
            auto_title_dedupe: Arc::new(StdMutex::new(AutoTitleDedupeCache::new(
                AUTO_TITLE_DEDUPE_TTL_MS,
                AUTO_TITLE_DEDUPE_MAX_KEYS,
            ))),
        })
    }

    pub fn new_with_api_keys_for_test(api_keys: HashSet<String>) -> Self {
        let auth_service = AuthService::for_test().expect("initialize test auth service");
        let store = Arc::new(SqliteStore::open_memory().expect("initialize test store"));
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock drift")
            .as_nanos();
        let transcript_root =
            std::env::temp_dir().join(format!("openjax-gateway-transcript-test-{pid}-{nanos}"));
        let transcript = Arc::new(TranscriptStore::new(transcript_root));
        let session_index =
            Arc::new(SessionIndexStore::new(transcript.root()).expect("init test session index"));
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(api_keys),
            auth_service: Arc::new(auth_service),
            store,
            transcript,
            session_index,
            auto_title_dedupe: Arc::new(StdMutex::new(AutoTitleDedupeCache::new(
                AUTO_TITLE_DEDUPE_TTL_MS,
                AUTO_TITLE_DEDUPE_MAX_KEYS,
            ))),
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
        let now = crate::error::now_rfc3339();
        let entry = IndexSessionEntry {
            session_id: session_id.clone(),
            title: None,
            created_at: now.clone(),
            updated_at: now,
            last_event_seq: 0,
            last_preview: String::new(),
        };
        self.session_index
            .create_session_index_entry(entry)
            .map_err(map_session_index_error)?;
        if let Err(store_err) = self.store.create_session(&session_id, None) {
            if let Err(rollback_err) = self.session_index.delete_session_index_entry(&session_id) {
                return Err(ApiError::internal(format!(
                    "create_session store failure rollback failed: store={store_err:#}; rollback={rollback_err:#}"
                )));
            }
            return Err(map_store_error(store_err));
        }
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
            .session_index
            .list_sessions()
            .iter()
            .any(|entry| entry.session_id == session_id);
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
        let index_entry_snapshot = self
            .session_index
            .list_sessions()
            .into_iter()
            .find(|entry| entry.session_id == session_id);
        self.session_index
            .delete_session_index_entry(session_id)
            .map_err(map_session_index_error)?;
        if let Err(store_err) = self.store.delete_session(session_id) {
            if let Some(snapshot) = index_entry_snapshot {
                if let Err(rollback_err) = self.session_index.create_session_index_entry(snapshot) {
                    return Err(ApiError::internal(format!(
                        "delete_session store failure rollback failed: store={store_err:#}; rollback={rollback_err:#}"
                    )));
                }
            }
            return Err(map_store_error(store_err));
        }
        Ok(())
    }

    pub fn list_persisted_sessions(
        &self,
        cursor: Option<(&str, &str)>,
        limit: usize,
    ) -> Result<(Vec<IndexSessionEntry>, Option<(String, String)>), ApiError> {
        if self.session_index.is_repair_required() {
            return Err(ApiError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                code: "INDEX_REPAIR_REQUIRED",
                message: "session index requires repair".to_string(),
                retryable: false,
                details: json!({}),
            });
        }
        let all = self.session_index.list_sessions();
        let mut filtered = Vec::new();
        for item in all {
            let include = match cursor {
                None => true,
                Some((cursor_updated_at, cursor_session_id)) => {
                    item.updated_at.as_str() < cursor_updated_at
                        || (item.updated_at.as_str() == cursor_updated_at
                            && item.session_id.as_str() < cursor_session_id)
                }
            };
            if include {
                filtered.push(item);
            }
        }
        let mut sessions = filtered.into_iter().take(limit).collect::<Vec<_>>();
        let next_cursor = if sessions.len() == limit {
            sessions
                .last()
                .map(|last| (last.updated_at.clone(), last.session_id.clone()))
        } else {
            None
        };
        if sessions.is_empty() {
            sessions.shrink_to_fit();
        }
        Ok((sessions, next_cursor))
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
    ) -> Result<Vec<TranscriptRecord>, ApiError> {
        self.transcript
            .replay(session_id, after_event_seq)
            .map_err(|err| ApiError::internal(err.to_string()))
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

    pub fn update_session_title(
        &self,
        session_id: &str,
        title: Option<String>,
    ) -> Result<(), ApiError> {
        self.session_index
            .update_session_title(session_id, title)
            .map_err(map_session_index_error)?;
        if let Err(err) = self.store.touch_session(session_id) {
            warn!(
                session_id,
                error_message = %err,
                "sqlite touch_session failed after title update; continuing with file index as source of truth"
            );
        }
        Ok(())
    }

    pub fn update_session_tags(&self, session_id: &str, tags: Vec<String>) -> Result<(), ApiError> {
        self.session_index
            .update_session_tags(session_id, tags)
            .map_err(map_session_index_error)?;
        if let Err(err) = self.store.touch_session(session_id) {
            warn!(
                session_id,
                error_message = %err,
                "sqlite touch_session failed after tags update; continuing with file index as source of truth"
            );
        }
        Ok(())
    }

    pub fn append_event(
        &self,
        event: &StreamEventEnvelope,
    ) -> Result<StreamEventEnvelope, ApiError> {
        if cfg!(debug_assertions) {
            let force_all = event.request_id.contains("__force_append_fail_all__");
            let force_payload = event
                .payload
                .get("__force_append_fail")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if force_all || force_payload {
                return Err(ApiError::internal("forced append failure"));
            }
        }
        let record = TranscriptRecord {
            schema_version: crate::transcript::TRANSCRIPT_SCHEMA_VERSION,
            session_id: event.session_id.clone(),
            event_seq: event.event_seq,
            turn_seq: event.turn_seq,
            turn_id: event.turn_id.clone(),
            event_type: event.event_type.clone(),
            stream_source: event.stream_source.clone(),
            timestamp: event.timestamp.clone(),
            payload: event.payload.clone(),
            request_id: event.request_id.clone(),
        };
        let persisted = self
            .transcript
            .append(&record)
            .map_err(|err| ApiError::internal(err.to_string()))?;
        if let Err(err) = self.refresh_session_index_from_event(&persisted) {
            warn!(
                session_id = %persisted.session_id,
                event_seq = persisted.event_seq,
                event_type = %persisted.event_type,
                error_code = err.code,
                error_message = %err.message,
                "session index refresh failed after transcript append"
            );
        }
        Ok(StreamEventEnvelope {
            request_id: persisted.request_id,
            session_id: persisted.session_id,
            turn_id: persisted.turn_id,
            event_seq: persisted.event_seq,
            turn_seq: persisted.turn_seq,
            timestamp: persisted.timestamp,
            event_type: persisted.event_type,
            stream_source: persisted.stream_source,
            payload: persisted.payload,
        })
    }

    fn refresh_session_index_from_event(&self, record: &TranscriptRecord) -> Result<(), ApiError> {
        let current_entry = self
            .session_index
            .list_sessions()
            .into_iter()
            .find(|entry| entry.session_id == record.session_id);
        let current_preview = current_entry
            .as_ref()
            .map(|entry| entry.last_preview.clone())
            .unwrap_or_default();
        if let Some(next_title) = derive_session_title(&record.event_type, &record.payload) {
            let should_skip = build_auto_title_dedupe_key(record)
                .map(|key| {
                    let now_ms = now_unix_ms();
                    let mut cache = self.auto_title_dedupe.lock().unwrap_or_else(|e| e.into_inner());
                    cache.should_skip(key, now_ms)
                })
                .unwrap_or(false);
            if !should_skip {
                self.session_index
                    .update_session_title_if_empty(&record.session_id, next_title)
                    .map_err(map_session_index_error)?;
            }
        }
        let next_preview =
            derive_last_preview(&record.event_type, &record.payload).unwrap_or(current_preview);
        self.session_index
            .touch_session_index_entry(
                &record.session_id,
                record.timestamp.clone(),
                record.event_seq,
                next_preview,
            )
            .map_err(map_session_index_error)?;
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
        let events = self
            .transcript
            .replay(session_id, None)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        if let Some(max_event_seq) = events.last().map(|event| event.event_seq) {
            runtime.next_event_seq = max_event_seq.saturating_add(1);
        }
        for row in events {
            let envelope = StreamEventEnvelope {
                request_id: row.request_id.clone(),
                session_id: row.session_id.clone(),
                turn_id: row.turn_id.clone(),
                event_seq: row.event_seq,
                turn_seq: row.turn_seq,
                timestamp: row.timestamp.clone(),
                event_type: row.event_type.clone(),
                stream_source: row.stream_source.clone(),
                payload: row.payload.clone(),
            };
            let _ = runtime.event_log.push(envelope);
            if let Some(turn_id) = row.turn_id.clone() {
                runtime.observe_stream_node_ids(Some(&turn_id), &row.event_type, &row.payload);
                let turn = runtime
                    .turns
                    .entry(turn_id.clone())
                    .or_insert_with(TurnRuntime::queued);
                apply_turn_runtime_event(turn, &row.event_type, &row.payload);
                runtime.turn_event_seq.insert(turn_id, row.turn_seq);
            }
        }
        Ok(Arc::new(Mutex::new(runtime)))
    }
}

fn derive_last_preview(event_type: &str, payload: &serde_json::Value) -> Option<String> {
    if event_type != "user_message" {
        return None;
    }
    payload
        .get("content")
        .and_then(|value| value.as_str())
        .map(|content| truncate_utf8_chars(content.trim(), 120))
}

fn derive_session_title(
    event_type: &str,
    payload: &serde_json::Value,
) -> Option<String> {
    if event_type != "user_message" {
        return None;
    }
    let raw = payload.get("content").and_then(|value| value.as_str())?;
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    const MAX_TITLE_CHARS: usize = 24;
    let title = if normalized.chars().count() > MAX_TITLE_CHARS {
        format!(
            "{}...",
            normalized.chars().take(MAX_TITLE_CHARS).collect::<String>()
        )
    } else {
        normalized
    };
    Some(title)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn build_auto_title_dedupe_key(record: &TranscriptRecord) -> Option<String> {
    if record.event_type != "user_message" {
        return None;
    }
    let raw = record.payload.get("content").and_then(|value| value.as_str())?;
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    normalized.hash(&mut hasher);
    let content_hash = hasher.finish();
    Some(format!(
        "{}|{}|{}|{}|{}",
        record.session_id,
        record.turn_id.as_deref().unwrap_or_default(),
        record.turn_seq,
        record.event_type,
        content_hash
    ))
}

#[derive(Debug)]
struct AutoTitleDedupeCache {
    ttl_ms: u64,
    cap: usize,
    expires_at: HashMap<String, u64>,
    order: VecDeque<String>,
}

impl AutoTitleDedupeCache {
    fn new(ttl_ms: u64, cap: usize) -> Self {
        Self {
            ttl_ms,
            cap: cap.max(1),
            expires_at: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn should_skip(&mut self, key: String, now_ms: u64) -> bool {
        self.evict_expired(now_ms);
        if let Some(expires_at) = self.expires_at.get(&key).copied() {
            if expires_at > now_ms {
                return true;
            }
            self.expires_at.remove(&key);
        }
        self.order.retain(|queued| queued != &key);
        let next_expiry = now_ms.saturating_add(self.ttl_ms);
        self.expires_at.insert(key.clone(), next_expiry);
        self.order.push_back(key);
        self.evict_overflow();
        false
    }

    fn evict_expired(&mut self, now_ms: u64) {
        let mut next_order = VecDeque::with_capacity(self.order.len());
        let mut seen = HashSet::new();
        while let Some(key) = self.order.pop_front() {
            let Some(expires_at) = self.expires_at.get(&key).copied() else {
                continue;
            };
            if expires_at <= now_ms {
                self.expires_at.remove(&key);
                continue;
            }
            if seen.insert(key.clone()) {
                next_order.push_back(key);
            }
        }
        self.order = next_order;
    }

    fn evict_overflow(&mut self) {
        while self.expires_at.len() > self.cap {
            let Some(front) = self.order.pop_front() else {
                break;
            };
            self.expires_at.remove(&front);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AUTO_TITLE_DEDUPE_MAX_KEYS, AUTO_TITLE_DEDUPE_TTL_MS, AutoTitleDedupeCache,
        derive_session_title,
    };
    use serde_json::json;

    #[test]
    fn auto_title_dedupe_cache_honors_ttl_window() {
        let mut cache = AutoTitleDedupeCache::new(AUTO_TITLE_DEDUPE_TTL_MS, 8);
        let now = 1_000_u64;
        assert!(!cache.should_skip("k1".to_string(), now));
        assert!(cache.should_skip("k1".to_string(), now + AUTO_TITLE_DEDUPE_TTL_MS - 1));
        assert!(!cache.should_skip("k1".to_string(), now + AUTO_TITLE_DEDUPE_TTL_MS));
    }

    #[test]
    fn auto_title_dedupe_cache_evicts_oldest_when_over_capacity() {
        let mut cache = AutoTitleDedupeCache::new(AUTO_TITLE_DEDUPE_TTL_MS, 2);
        let now = 1_000_u64;
        assert!(!cache.should_skip("k1".to_string(), now));
        assert!(!cache.should_skip("k2".to_string(), now));
        assert!(!cache.should_skip("k3".to_string(), now));
        assert_eq!(cache.expires_at.len(), 2);
        assert!(!cache.should_skip("k1".to_string(), now + 1));
    }

    #[test]
    fn auto_title_dedupe_cache_handles_middle_expiry_and_key_reinsert() {
        let mut cache = AutoTitleDedupeCache::new(10, 8);
        assert!(!cache.should_skip("k1".to_string(), 0));
        assert!(!cache.should_skip("k2".to_string(), 1));
        assert!(!cache.should_skip("k3".to_string(), 2));
        assert!(!cache.should_skip("k2".to_string(), 20));
        assert!(cache.should_skip("k2".to_string(), 21));
    }

    #[test]
    fn derive_session_title_normalizes_whitespace_and_truncates_24_codepoints() {
        let normalized = derive_session_title("user_message", &json!({ "content": "  a\t b \n c " }));
        assert_eq!(normalized.as_deref(), Some("a b c"));

        let long = "你".repeat(25);
        let truncated = derive_session_title("user_message", &json!({ "content": long }));
        assert_eq!(truncated.as_deref(), Some(format!("{}...", "你".repeat(24)).as_str()));
    }

    #[test]
    fn dedupe_constants_match_spec_contract() {
        assert_eq!(AUTO_TITLE_DEDUPE_TTL_MS, 30_000);
        assert_eq!(AUTO_TITLE_DEDUPE_MAX_KEYS, 10_000);
    }
}

fn truncate_utf8_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    input.chars().take(max_chars).collect()
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

fn map_session_index_error(err: anyhow::Error) -> ApiError {
    if format!("{err:#}").contains("index_repair_required") {
        return ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "INDEX_REPAIR_REQUIRED",
            message: "session index requires repair".to_string(),
            retryable: false,
            details: json!({}),
        };
    }
    ApiError::internal(err.to_string())
}
