use serde::{Deserialize, Serialize};

pub const SESSION_INDEX_SCHEMA_VERSION: u32 = 1;
pub const INDEX_SNAPSHOT_FILE: &str = "index.snapshot.json";
pub const INDEX_LOG_FILE: &str = "index.log.ndjson";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexSessionEntry {
    pub session_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_event_seq: u64,
    pub last_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexSnapshot {
    pub schema_version: u32,
    pub updated_at: String,
    #[serde(default)]
    pub sessions: Vec<IndexSessionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IndexLogOpKind {
    UpsertSession,
    DeleteSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexLogRecord {
    pub op: IndexLogOpKind,
    pub session_id: String,
    pub ts: String,
    #[serde(default)]
    pub payload: Option<IndexSessionEntry>,
}
