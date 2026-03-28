use crate::agent::prompt::{
    build_system_prompt, build_turn_messages, refresh_loop_recovery_in_messages,
    summarize_user_input,
};
use crate::agent::runtime_policy::{
    parse_sandbox_mode, resolve_max_planner_rounds_per_turn_with_lookup,
    resolve_max_tool_calls_per_turn_with_lookup,
};
use crate::model::{AssistantContentBlock, ConversationMessage, UserContentBlock};
use crate::{Config, HistoryItem, SandboxMode, TurnRecord};

#[test]
fn parse_runtime_policies() {
    assert!(matches!(
        parse_sandbox_mode("workspace_write"),
        Some(SandboxMode::WorkspaceWrite)
    ));
    assert!(matches!(
        parse_sandbox_mode("danger_full_access"),
        Some(SandboxMode::DangerFullAccess)
    ));
    assert!(parse_sandbox_mode("invalid").is_none());
}

#[test]
fn resolves_turn_limits_from_config_and_env_with_precedence() {
    let config = Config {
        agent: Some(crate::AgentConfig {
            max_agents: None,
            max_depth: None,
            max_tool_calls_per_turn: Some(15),
            max_planner_rounds_per_turn: Some(30),
        }),
        ..Config::default()
    };

    assert_eq!(
        resolve_max_tool_calls_per_turn_with_lookup(&config, |_| None),
        15
    );
    assert_eq!(
        resolve_max_planner_rounds_per_turn_with_lookup(&config, |_| None),
        30
    );

    let env_lookup = |key: &str| match key {
        "OPENJAX_MAX_TOOL_CALLS_PER_TURN" => Some("12".to_string()),
        "OPENJAX_MAX_PLANNER_ROUNDS_PER_TURN" => Some("25".to_string()),
        _ => None,
    };
    assert_eq!(
        resolve_max_tool_calls_per_turn_with_lookup(&config, env_lookup),
        12
    );
    assert_eq!(
        resolve_max_planner_rounds_per_turn_with_lookup(&config, env_lookup),
        25
    );
}

#[test]
fn build_system_prompt_contains_verification_rule() {
    let prompt = build_system_prompt("(none)");
    assert!(prompt.contains("verification already shows the requested content/changes are present"));
    assert!(prompt.contains("respond immediately"));
}

#[test]
fn build_system_prompt_contains_skills_section() {
    let prompt = build_system_prompt("- name: rust-debug");
    assert!(prompt.contains("Available skills (auto-selected):"));
    assert!(prompt.contains("- name: rust-debug"));
}

#[test]
fn build_turn_messages_includes_prior_conversation_summary() {
    let history = vec![HistoryItem::Turn(TurnRecord {
        user_input: "look at src/main.rs".to_string(),
        tool_traces: vec!["tool=read_file; ok=true; output=fn main() {}".to_string()],
        assistant_output: "入口在这里".to_string(),
    })];

    let messages = build_turn_messages("继续修改", &history, None);

    assert!(matches!(
        &messages[0],
        ConversationMessage::User(blocks)
            if matches!(
                &blocks[0],
                UserContentBlock::Text { text }
                    if text.contains("<prior_conversation>")
                        && text.contains("look at src/main.rs")
                        && text.contains("tool=read_file")
            )
    ));
    assert!(matches!(
        &messages[1],
        ConversationMessage::User(blocks)
            if matches!(
                &blocks[0],
                UserContentBlock::Text { text } if text == "继续修改"
            )
    ));
}

#[test]
fn refresh_loop_recovery_only_updates_last_user_text() {
    let mut messages = vec![
        ConversationMessage::User(vec![UserContentBlock::Text {
            text: "<prior_conversation>\nold\n</prior_conversation>".to_string(),
        }]),
        ConversationMessage::Assistant(vec![AssistantContentBlock::ToolUse {
            id: "call_1".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "src/main.rs"}),
        }]),
        ConversationMessage::User(vec![UserContentBlock::ToolResult {
            tool_use_id: "call_1".to_string(),
            content: "fn main() {}".to_string(),
            is_error: false,
        }]),
        ConversationMessage::User(vec![UserContentBlock::Text {
            text: "继续修改".to_string(),
        }]),
    ];

    refresh_loop_recovery_in_messages(&mut messages, "继续修改", Some("请避免重复调用"));

    assert!(matches!(
        &messages[0],
        ConversationMessage::User(blocks)
            if matches!(
                &blocks[0],
                UserContentBlock::Text { text }
                    if text == "<prior_conversation>\nold\n</prior_conversation>"
            )
    ));
    assert!(matches!(&messages[1], ConversationMessage::Assistant(_)));
    assert!(matches!(
        &messages[2],
        ConversationMessage::User(blocks)
            if matches!(&blocks[0], UserContentBlock::ToolResult { .. })
    ));
    assert!(matches!(
        &messages[3],
        ConversationMessage::User(blocks)
            if matches!(
                &blocks[0],
                UserContentBlock::Text { text }
                    if text == "继续修改\n\n请避免重复调用"
            )
    ));
}

#[test]
fn summarize_user_input_escapes_control_newlines() {
    let (preview, truncated) = summarize_user_input("hello\nworld", 40);
    assert_eq!(preview, "hello\\nworld");
    assert!(!truncated);
}

#[test]
fn summarize_user_input_adds_ellipsis_when_truncated() {
    let (preview, truncated) = summarize_user_input("abcdef", 3);
    assert_eq!(preview, "abc...");
    assert!(truncated);
}
