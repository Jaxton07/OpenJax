use openjax_protocol::{Event, StreamSource};

#[derive(Debug, Clone)]
pub struct ResponseStreamOrchestrator {
    turn_id: u64,
    stream_source: StreamSource,
    started: bool,
    accumulated: String,
}

impl ResponseStreamOrchestrator {
    pub fn new(turn_id: u64, stream_source: StreamSource) -> Self {
        Self {
            turn_id,
            stream_source,
            started: false,
            accumulated: String::new(),
        }
    }

    pub fn on_delta(&mut self, delta: &str) -> Vec<Event> {
        let mut out = Vec::new();
        if delta.is_empty() {
            return out;
        }
        if !self.started {
            self.started = true;
            out.push(Event::ResponseStarted {
                turn_id: self.turn_id,
                stream_source: self.stream_source.clone(),
            });
        }
        self.accumulated.push_str(delta);
        out.push(Event::ResponseTextDelta {
            turn_id: self.turn_id,
            content_delta: delta.to_string(),
            stream_source: self.stream_source.clone(),
        });
        out
    }

    pub fn emit_completed(&mut self, content: String) -> (String, Event) {
        let resolved = if self.accumulated.is_empty() {
            content
        } else if content.is_empty() || content == self.accumulated {
            self.accumulated.clone()
        } else {
            content
        };
        (
            resolved.clone(),
            Event::ResponseCompleted {
                turn_id: self.turn_id,
                content: resolved.clone(),
                stream_source: self.stream_source.clone(),
            },
        )
    }

    pub fn emit_error(&self, code: &str, message: String, retryable: bool) -> Event {
        Event::ResponseError {
            turn_id: self.turn_id,
            code: code.to_string(),
            message,
            retryable,
        }
    }

    pub fn has_started(&self) -> bool {
        self.started
    }

    pub fn accumulated(&self) -> &str {
        &self.accumulated
    }
}

#[cfg(test)]
mod tests {
    use openjax_protocol::{Event, StreamSource};

    use super::ResponseStreamOrchestrator;

    #[test]
    fn emits_started_then_delta_then_completed() {
        let mut orchestrator = ResponseStreamOrchestrator::new(7, StreamSource::ModelLive);
        let events = orchestrator.on_delta("hello");
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            Event::ResponseStarted { turn_id: 7, .. }
        ));
        assert!(matches!(
            events[1],
            Event::ResponseTextDelta {
                turn_id: 7,
                ref content_delta,
                ..
            } if content_delta == "hello"
        ));

        let (resolved, completed) = orchestrator.emit_completed(String::new());
        assert_eq!(resolved, "hello");
        assert!(matches!(
            completed,
            Event::ResponseCompleted {
                turn_id: 7,
                ref content,
                ..
            } if content == "hello"
        ));
    }

    #[test]
    fn emits_response_error_payload() {
        let orchestrator = ResponseStreamOrchestrator::new(3, StreamSource::Synthetic);
        let error = orchestrator.emit_error("upstream", "failed".to_string(), true);
        assert!(matches!(
            error,
            Event::ResponseError {
                turn_id: 3,
                ref code,
                ref message,
                retryable: true,
            } if code == "upstream" && message == "failed"
        ));
    }
}
