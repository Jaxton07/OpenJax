use anyhow::Result;

use crate::types::{
    ActiveProviderRecord, EventRecord, MessageRecord, ProviderRecord, SessionRecord,
};

pub trait SessionRepository {
    fn create_session(&self, session_id: &str, title: Option<&str>) -> Result<SessionRecord>;
    fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>>;
    fn list_sessions(&self) -> Result<Vec<SessionRecord>>;
    fn delete_session(&self, session_id: &str) -> Result<bool>;
    fn append_message(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        role: &str,
        content: &str,
    ) -> Result<MessageRecord>;
    fn list_messages(&self, session_id: &str) -> Result<Vec<MessageRecord>>;
    #[allow(clippy::too_many_arguments)]
    fn append_event(
        &self,
        session_id: &str,
        event_seq: u64,
        turn_seq: u64,
        turn_id: Option<&str>,
        event_type: &str,
        payload_json: &str,
        timestamp: &str,
        stream_source: &str,
    ) -> Result<EventRecord>;
    fn list_events(&self, session_id: &str, after_event_seq: Option<u64>) -> Result<Vec<EventRecord>>;
    fn last_event_seq(&self, session_id: &str) -> Result<Option<u64>>;
    fn last_turn_seq_by_turn(&self, session_id: &str) -> Result<Vec<(String, u64)>>;
}

pub trait ProviderRepository {
    fn create_provider(
        &self,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: &str,
        provider_type: &str,
        context_window_size: u32,
    ) -> Result<ProviderRecord>;
    fn update_provider(
        &self,
        provider_id: &str,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: Option<&str>,
        context_window_size: u32,
    ) -> Result<Option<ProviderRecord>>;
    fn delete_provider(&self, provider_id: &str) -> Result<bool>;
    fn get_provider(&self, provider_id: &str) -> Result<Option<ProviderRecord>>;
    fn list_providers(&self) -> Result<Vec<ProviderRecord>>;
    fn get_active_provider(&self) -> Result<Option<ActiveProviderRecord>>;
    fn set_active_provider(&self, provider_id: &str) -> Result<Option<ActiveProviderRecord>>;
}
