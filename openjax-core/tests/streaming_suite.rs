//! Aggregated integration suite for submit, streaming, and synthetic response event flows.

#[path = "streaming/m6_submit_stream.rs"]
mod submit_stream_m6;
#[path = "streaming/m7_backward_compat_submit.rs"]
mod backward_compat_submit_m7;
#[path = "streaming/m21_tool_streaming_events.rs"]
mod tool_streaming_events_m21;
#[path = "streaming/m23_assistant_message_decommission_guardrails.rs"]
mod assistant_message_decommission_guardrails_m23;
