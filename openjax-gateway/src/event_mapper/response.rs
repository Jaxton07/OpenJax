use openjax_protocol::Event;
use serde_json::json;

use super::CoreEventMapping;

pub fn map(event: &Event) -> Option<CoreEventMapping> {
    match event {
        Event::TurnStarted { turn_id } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "turn_started",
            payload: json!({}),
            stream_source: None,
        }),
        Event::ResponseStarted {
            turn_id,
            stream_source,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "response_started",
            payload: json!({ "stream_source": stream_source }),
            stream_source: Some(stream_source_wire(stream_source)),
        }),
        Event::ResponseTextDelta {
            turn_id,
            content_delta,
            stream_source,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "response_text_delta",
            payload: json!({ "content_delta": content_delta, "stream_source": stream_source }),
            stream_source: Some(stream_source_wire(stream_source)),
        }),
        Event::ReasoningDelta {
            turn_id,
            content_delta,
            stream_source,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "reasoning_delta",
            payload: json!({ "content_delta": content_delta, "stream_source": stream_source }),
            stream_source: Some(stream_source_wire(stream_source)),
        }),
        Event::ResponseResumed {
            turn_id,
            stream_source,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "response_resumed",
            payload: json!({ "stream_source": stream_source }),
            stream_source: Some(stream_source_wire(stream_source)),
        }),
        Event::ResponseCompleted {
            turn_id,
            content,
            stream_source,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "response_completed",
            payload: json!({ "content": content, "stream_source": stream_source }),
            stream_source: Some(stream_source_wire(stream_source)),
        }),
        Event::ResponseError {
            turn_id,
            code,
            message,
            retryable,
        } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "response_error",
            payload: json!({ "code": code, "message": message, "retryable": retryable }),
            stream_source: None,
        }),
        Event::TurnCompleted { turn_id } => Some(CoreEventMapping {
            core_turn_id: Some(*turn_id),
            event_type: "turn_completed",
            payload: json!({}),
            stream_source: None,
        }),
        _ => None,
    }
}

fn stream_source_wire(source: &openjax_protocol::StreamSource) -> &'static str {
    match source {
        openjax_protocol::StreamSource::ModelLive => "model_live",
        openjax_protocol::StreamSource::Synthetic => "synthetic",
        openjax_protocol::StreamSource::Replay => "replay",
        openjax_protocol::StreamSource::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use openjax_protocol::{Event, StreamSource};

    use super::map;

    #[test]
    fn maps_reasoning_delta_event() {
        let mapping = map(&Event::ReasoningDelta {
            turn_id: 9,
            content_delta: "step".to_string(),
            stream_source: StreamSource::ModelLive,
        })
        .expect("mapping should exist");

        assert_eq!(mapping.core_turn_id, Some(9));
        assert_eq!(mapping.event_type, "reasoning_delta");
        assert_eq!(
            mapping
                .payload
                .get("content_delta")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            "step"
        );
        assert_eq!(mapping.stream_source, Some("model_live"));
    }
}
