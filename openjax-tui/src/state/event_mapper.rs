use crate::render::markdown::render_markdown_as_plain_text;
use crate::state::{AppState, RenderKind, TurnPhase};
use openjax_protocol::Event;

pub fn apply_core_event(state: &mut AppState, event: &Event) {
    match event {
        Event::TurnStarted { turn_id } => {
            state.turn.start_turn(*turn_id);
        }
        Event::AssistantDelta {
            turn_id,
            content_delta,
        } => {
            let merged = state.turn.append_delta(*turn_id, content_delta);
            if let Some(last) = state.transcript.messages.last_mut() {
                if last.role == "assistant" && state.turn.active_turn_id == Some(*turn_id) {
                    last.content = merged;
                    return;
                }
            }
            state.push_assistant_message(merged, RenderKind::Plain);
        }
        Event::AssistantMessage { turn_id, content } => {
            state
                .turn
                .set_stream_content(*turn_id, content.clone(), RenderKind::Markdown);
            let rendered = render_markdown_as_plain_text(content);
            if let Some(last) = state.transcript.messages.last_mut() {
                if last.role == "assistant" {
                    last.content = rendered;
                    last.render_kind = RenderKind::Markdown;
                    return;
                }
            }
            state.push_assistant_message(rendered, RenderKind::Markdown);
        }
        Event::ToolCallStarted {
            turn_id,
            tool_name,
            target,
        } => {
            if let Some(t) = target.as_deref() {
                if !t.trim().is_empty() {
                    state
                        .turn
                        .add_tool_target_hint(*turn_id, tool_name, t.trim());
                }
            }
            state.push_system_message(format!("tool started: {tool_name}"));
        }
        Event::ToolCallCompleted {
            turn_id,
            tool_name,
            ok,
            output,
            ..
        } => {
            let label = tool_label(tool_name);
            let target = state
                .turn
                .pop_tool_target_hint(*turn_id, tool_name)
                .or_else(|| extract_target(tool_name, output));
            state.push_tool_message(label, *ok, target);
            state.push_system_message(format!(
                "tool completed: {tool_name} (ok={ok}) {}",
                truncate(output, 160)
            ));
        }
        Event::TurnCompleted { turn_id } => {
            let _ = state.turn.finalize_turn(*turn_id);
            state.update_phase(TurnPhase::Idle);
            state.push_system_message(format!("turn completed: {turn_id}"));
        }
        Event::ApprovalRequested {
            turn_id,
            request_id,
            target,
            reason,
        } => {
            state.enqueue_approval_request(
                request_id.clone(),
                *turn_id,
                target.clone(),
                reason.clone(),
            );
        }
        Event::ApprovalResolved {
            request_id,
            approved,
            ..
        } => {
            state.close_approval_overlay(request_id);
            state.push_system_message(format!(
                "approval resolved: id={request_id} approved={approved}"
            ));
        }
        Event::ShutdownComplete => {
            state.push_system_message("shutdown complete".to_string());
        }
        Event::AgentSpawned { .. } | Event::AgentStatusChanged { .. } => {}
    }
}

fn tool_label(name: &str) -> String {
    match name.trim().to_ascii_lowercase().as_str() {
        "read_file" => "Read 1 file".to_string(),
        "apply_patch" | "edit_file_range" | "write_file" => "Update 1 file".to_string(),
        "list_dir" => "Read directory".to_string(),
        "grep_files" => "Search files".to_string(),
        "shell" => "Run shell command".to_string(),
        other if !other.is_empty() => other.replace('_', " "),
        _ => "Tool call".to_string(),
    }
}

fn extract_target(tool_name: &str, output: &str) -> Option<String> {
    let normalized = output.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    match tool_name {
        "read_file" => normalized
            .strip_prefix("READ ")
            .map(|v| v.split_whitespace().next().unwrap_or_default().to_string()),
        "apply_patch" | "edit_file_range" | "write_file" => normalized
            .strip_prefix("UPDATE ")
            .map(|v| v.split_whitespace().next().unwrap_or_default().to_string()),
        _ => None,
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars().take(max_chars) {
        out.push(ch);
    }
    out.push_str("...");
    out
}
