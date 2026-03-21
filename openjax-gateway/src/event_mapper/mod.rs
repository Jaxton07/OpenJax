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
        Event::ContextUsageUpdated {
            turn_id,
            input_tokens,
            context_window_size,
            ratio,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "context_usage_updated",
            payload: json!({
                "input_tokens": input_tokens,
                "context_window_size": context_window_size,
                "ratio": ratio,
            }),
            stream_source: None,
        }),
        Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use openjax_protocol::Event;

    use super::map_core_event_payload;

    #[test]
    fn maps_context_usage_updated_event() {
        let mapping = map_core_event_payload(&Event::ContextUsageUpdated {
            turn_id: 7,
            input_tokens: 4096,
            context_window_size: 128000,
            ratio: 0.032,
        })
        .expect("mapping should exist");

        assert_eq!(mapping.core_turn_id, Some(7));
        assert_eq!(mapping.event_type, "context_usage_updated");
        assert_eq!(
            mapping
                .payload
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or_default(),
            4096
        );
        assert_eq!(
            mapping
                .payload
                .get("context_window_size")
                .and_then(|v| v.as_u64())
                .unwrap_or_default(),
            128000
        );
        assert!(
            mapping
                .payload
                .get("ratio")
                .and_then(|v| v.as_f64())
                .unwrap_or_default()
                > 0.03
        );
        assert_eq!(mapping.stream_source, None);
    }
}
