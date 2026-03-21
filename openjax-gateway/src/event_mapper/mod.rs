use openjax_protocol::Event;
use serde_json::{Value, json};

pub mod approval;
pub mod response;
pub mod tool;

#[derive(Debug, Clone)]
pub struct CoreEventMapping {
    pub core_turn_id: Option<u64>,
    pub event_type: &'static str,
    pub payload: Value,
    pub stream_source: Option<&'static str>,
}

pub fn map_core_event_payload(event: &Event) -> Option<CoreEventMapping> {
    response::map(event)
        .or_else(|| tool::map(event))
        .or_else(|| approval::map(event))
        .or_else(|| map_misc(event))
}

fn map_misc(event: &Event) -> Option<CoreEventMapping> {
    match event {
        Event::ShutdownComplete => Some(CoreEventMapping {
            core_turn_id: None,
            event_type: "session_shutdown",
            payload: json!({}),
            stream_source: None,
        }),
        Event::ContextCompacted {
            turn_id,
            compressed_turns,
            retained_turns,
            summary_preview,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "context_compacted",
            payload: json!({
                "compressed_turns": compressed_turns,
                "retained_turns": retained_turns,
                "summary_preview": summary_preview,
            }),
            stream_source: None,
        }),
        Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => None,
        _ => None,
    }
}
