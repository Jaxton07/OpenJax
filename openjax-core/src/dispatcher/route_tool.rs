use openjax_protocol::Event;

pub(crate) fn emit_tool_call_ready(
    events: &mut Vec<Event>,
    turn_id: u64,
    tool_call_id: &str,
    tool_name: &str,
) {
    events.push(Event::ToolCallReady {
        turn_id,
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
    });
}
