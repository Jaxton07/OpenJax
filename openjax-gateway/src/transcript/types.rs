use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TRANSCRIPT_SCHEMA_VERSION: u32 = 1;
pub const FIRST_SEGMENT_FILE: &str = "segment-000001.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRecord {
    pub schema_version: u32,
    pub session_id: String,
    pub event_seq: u64,
    pub turn_seq: u64,
    pub turn_id: Option<String>,
    pub event_type: String,
    pub stream_source: String,
    pub timestamp: String,
    pub payload: Value,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptManifest {
    pub schema_version: u32,
    pub session_id: String,
    pub last_event_seq: u64,
    pub last_turn_seq: u64,
    pub active_segment: String,
    pub updated_at: String,
}

impl TranscriptManifest {
    pub fn new_for_first_record(record: &TranscriptRecord) -> Self {
        Self {
            schema_version: TRANSCRIPT_SCHEMA_VERSION,
            session_id: record.session_id.clone(),
            last_event_seq: record.event_seq,
            last_turn_seq: record.turn_seq,
            active_segment: FIRST_SEGMENT_FILE.to_string(),
            updated_at: record.timestamp.clone(),
        }
    }
}
