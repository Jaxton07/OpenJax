use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct ModelRequestOptions {
    pub max_output_tokens: Option<u32>,
    pub thinking_budget_tokens: Option<u32>,
    pub require_reasoning: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ModelRequest {
    pub stage: ModelStage,
    pub user_input: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub options: ModelRequestOptions,
}

impl ModelRequest {
    pub fn for_stage(stage: ModelStage, user_input: impl Into<String>) -> Self {
        Self {
            stage,
            user_input: user_input.into(),
            system_prompt: None,
            options: ModelRequestOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
pub struct ModelUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelResponse {
    pub text: String,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub usage: Option<ModelUsage>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub raw: Option<Value>,
}
