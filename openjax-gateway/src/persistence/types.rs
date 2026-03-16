#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRecord {
    pub message_id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub role: String,
    pub content: String,
    pub sequence: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub id: i64,
    pub session_id: String,
    pub event_seq: u64,
    pub turn_seq: u64,
    pub turn_id: Option<String>,
    pub event_type: String,
    pub payload_json: String,
    pub timestamp: String,
    pub stream_source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRecord {
    pub provider_id: String,
    pub provider_name: String,
    pub base_url: String,
    pub model_name: String,
    pub api_key: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveProviderRecord {
    pub provider_id: String,
    pub model_name: String,
    pub updated_at: String,
}
