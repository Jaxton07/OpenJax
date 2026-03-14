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
