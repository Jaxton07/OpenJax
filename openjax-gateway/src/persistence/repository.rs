use anyhow::Result;

use crate::persistence::types::{MessageRecord, ProviderRecord, SessionRecord};

pub trait SessionRepository {
    fn create_session(&self, session_id: &str, title: Option<&str>) -> Result<SessionRecord>;
    fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>>;
    fn list_sessions(&self) -> Result<Vec<SessionRecord>>;
    fn append_message(
        &self,
        session_id: &str,
        turn_id: Option<&str>,
        role: &str,
        content: &str,
    ) -> Result<MessageRecord>;
    fn list_messages(&self, session_id: &str) -> Result<Vec<MessageRecord>>;
}

pub trait ProviderRepository {
    fn create_provider(
        &self,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: &str,
    ) -> Result<ProviderRecord>;
    fn update_provider(
        &self,
        provider_id: &str,
        provider_name: &str,
        base_url: &str,
        model_name: &str,
        api_key: Option<&str>,
    ) -> Result<Option<ProviderRecord>>;
    fn delete_provider(&self, provider_id: &str) -> Result<bool>;
    fn get_provider(&self, provider_id: &str) -> Result<Option<ProviderRecord>>;
    fn list_providers(&self) -> Result<Vec<ProviderRecord>>;
}
