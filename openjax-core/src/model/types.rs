use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::spec::ToolSpec;

// ---- Model Stage ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStage {
    Planner,
    FinalWriter,
    ToolReasoning,
}

impl ModelStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planner => "planner",
            Self::FinalWriter => "final_writer",
            Self::ToolReasoning => "tool_reasoning",
        }
    }
}

// ---- Capability Flags ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct CapabilityFlags {
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub json_mode: bool,
}

// ---- Model Request Options ----

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct ModelRequestOptions {
    pub max_output_tokens: Option<u32>,
    pub thinking_budget_tokens: Option<u32>,
    pub require_reasoning: Option<bool>,
}

// ---- Conversation Message Types ----

/// A single turn in the conversation, either from the user or the assistant.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "role", content = "content", rename_all = "snake_case")]
pub enum ConversationMessage {
    User(Vec<UserContentBlock>),
    Assistant(Vec<AssistantContentBlock>),
}

/// A content block inside a user message.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserContentBlock {
    Text { text: String },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

/// A content block inside an assistant message.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Other(String),
}

impl StopReason {
    pub fn from_api_str(s: &str) -> Self {
        match s {
            "end_turn" | "stop" => Self::EndTurn,
            "tool_use" | "tool_calls" => Self::ToolUse,
            "max_tokens" | "length" => Self::MaxTokens,
            other => Self::Other(other.to_string()),
        }
    }
}

// ---- Model Request ----

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ModelRequest {
    pub stage: ModelStage,
    /// Ordered conversation history plus the current user turn.
    pub messages: Vec<ConversationMessage>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Tools available to the model for this request.
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    #[serde(default)]
    pub options: ModelRequestOptions,
}

impl ModelRequest {
    /// Creates a request with a single user text message (convenience constructor).
    pub fn for_stage(stage: ModelStage, user_input: impl Into<String>) -> Self {
        Self {
            stage,
            messages: vec![ConversationMessage::User(vec![UserContentBlock::Text {
                text: user_input.into(),
            }])],
            system_prompt: None,
            tools: Vec::new(),
            options: ModelRequestOptions::default(),
        }
    }

    /// Returns the text from the last user message's first text block.
    ///
    /// Bridge helper used by Phase-1 adapters before Phase-2 rewrites them to
    /// consume `messages` directly.
    pub fn user_text(&self) -> &str {
        for msg in self.messages.iter().rev() {
            if let ConversationMessage::User(blocks) = msg {
                for block in blocks {
                    if let UserContentBlock::Text { text } = block {
                        return text.as_str();
                    }
                }
            }
        }
        ""
    }
}

// ---- Model Usage ----

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct ModelUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

// ---- Model Response ----

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelResponse {
    /// Ordered assistant content blocks (text and/or tool-use).
    pub content: Vec<AssistantContentBlock>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub usage: Option<ModelUsage>,
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
    #[serde(default)]
    pub raw: Option<Value>,
}

impl ModelResponse {
    /// Concatenates all `Text` content blocks into a single string.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                AssistantContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Returns references to all `ToolUse` content blocks.
    pub fn tool_uses(&self) -> Vec<&AssistantContentBlock> {
        self.content
            .iter()
            .filter(|b| matches!(b, AssistantContentBlock::ToolUse { .. }))
            .collect()
    }

    pub fn has_tool_use(&self) -> bool {
        self.content
            .iter()
            .any(|b| matches!(b, AssistantContentBlock::ToolUse { .. }))
    }

    pub fn stop_is_end_turn(&self) -> bool {
        matches!(self.stop_reason, Some(StopReason::EndTurn))
    }
}

// ---- Stream Delta ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamDelta {
    /// Streamed text from the assistant.
    Text(String),
    /// Streamed reasoning/thinking content.
    Reasoning(String),
    /// A tool-use block started streaming (emitted before args arrive).
    ToolUseStart { id: String, name: String },
    /// Incremental JSON fragment for a tool's `input` field.
    ToolArgsDelta { id: String, delta: String },
    /// A tool-use block finished streaming.
    ToolUseEnd { id: String },
}

// ---- Unit Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_response_text_extracts_only_text_blocks() {
        let resp = ModelResponse {
            content: vec![
                AssistantContentBlock::Text {
                    text: "hello ".to_string(),
                },
                AssistantContentBlock::ToolUse {
                    id: "id1".to_string(),
                    name: "shell".to_string(),
                    input: serde_json::json!({"cmd": "ls"}),
                },
                AssistantContentBlock::Text {
                    text: "world".to_string(),
                },
            ],
            ..ModelResponse::default()
        };
        assert_eq!(resp.text(), "hello world");
    }

    #[test]
    fn model_response_tool_uses_filters_correctly() {
        let resp = ModelResponse {
            content: vec![
                AssistantContentBlock::Text {
                    text: "text".to_string(),
                },
                AssistantContentBlock::ToolUse {
                    id: "id1".to_string(),
                    name: "shell".to_string(),
                    input: serde_json::json!({}),
                },
            ],
            ..ModelResponse::default()
        };
        assert_eq!(resp.tool_uses().len(), 1);
        assert!(resp.has_tool_use());
        assert!(!resp.stop_is_end_turn());
    }

    #[test]
    fn model_request_for_stage_wraps_as_user_text() {
        let req = ModelRequest::for_stage(ModelStage::Planner, "hello world");
        assert_eq!(req.user_text(), "hello world");
        assert_eq!(req.messages.len(), 1);
        assert!(
            matches!(&req.messages[0], ConversationMessage::User(blocks) if blocks.len() == 1)
        );
    }

    #[test]
    fn conversation_message_serde_roundtrip() {
        let msg = ConversationMessage::User(vec![
            UserContentBlock::Text {
                text: "hello".to_string(),
            },
            UserContentBlock::ToolResult {
                tool_use_id: "id1".to_string(),
                content: "result".to_string(),
                is_error: false,
            },
        ]);
        let json = serde_json::to_string(&msg).unwrap();
        let back: ConversationMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn stop_reason_from_api_str() {
        assert_eq!(StopReason::from_api_str("end_turn"), StopReason::EndTurn);
        assert_eq!(StopReason::from_api_str("tool_use"), StopReason::ToolUse);
        assert_eq!(StopReason::from_api_str("max_tokens"), StopReason::MaxTokens);
        assert_eq!(StopReason::from_api_str("length"), StopReason::MaxTokens);
        // OpenAI "stop" maps to EndTurn
        assert_eq!(StopReason::from_api_str("stop"), StopReason::EndTurn);
        // OpenAI "tool_calls" maps to ToolUse
        assert_eq!(StopReason::from_api_str("tool_calls"), StopReason::ToolUse);
    }
}
