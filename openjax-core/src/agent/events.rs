use openjax_protocol::Event;

use crate::Agent;

impl Agent {
    pub(crate) fn push_event(&self, events: &mut Vec<Event>, event: Event) {
        if let Some(sink) = &self.event_sink {
            let _ = sink.send(event.clone());
        }
        events.push(event);
    }
}
