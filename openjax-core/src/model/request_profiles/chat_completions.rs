use anyhow::{Result, anyhow};

use crate::model::types::ModelRequest;

const KIMI_DEFAULT_MAX_TOKENS: u32 = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatCompletionsRequestProfile {
    Default,
    KimiCodingV1,
}

impl ChatCompletionsRequestProfile {
    pub(crate) fn parse(raw: Option<&str>) -> Result<Self> {
        match raw.unwrap_or("default").trim() {
            "" | "default" => Ok(Self::Default),
            "kimi_coding_v1" => Ok(Self::KimiCodingV1),
            other => Err(anyhow!(
                "unknown chat_completions request_profile '{other}'; supported profiles: default, kimi_coding_v1"
            )),
        }
    }

    pub(crate) fn resolve_max_tokens(self, request: &ModelRequest) -> Option<u32> {
        match self {
            Self::Default => request.options.max_output_tokens,
            Self::KimiCodingV1 => request
                .options
                .max_output_tokens
                .or(Some(KIMI_DEFAULT_MAX_TOKENS)),
        }
    }

    pub(crate) fn include_stream_options(self) -> bool {
        !matches!(self, Self::KimiCodingV1)
    }

    pub(crate) fn user_agent(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::KimiCodingV1 => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::types::{ModelRequest, ModelStage};

    use super::ChatCompletionsRequestProfile;

    #[test]
    fn kimi_profile_supplies_default_max_tokens() {
        let profile = ChatCompletionsRequestProfile::KimiCodingV1;
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        assert_eq!(profile.resolve_max_tokens(&request), Some(200_000));
    }

    #[test]
    fn kimi_profile_preserves_explicit_max_tokens() {
        let profile = ChatCompletionsRequestProfile::KimiCodingV1;
        let mut request = ModelRequest::for_stage(ModelStage::Planner, "hello");
        request.options.max_output_tokens = Some(1024);

        assert_eq!(profile.resolve_max_tokens(&request), Some(1024));
    }

    #[test]
    fn kimi_profile_disables_stream_options() {
        let profile = ChatCompletionsRequestProfile::KimiCodingV1;
        assert!(!profile.include_stream_options());
    }

    #[test]
    fn kimi_profile_user_agent_is_none() {
        let profile = ChatCompletionsRequestProfile::KimiCodingV1;
        assert_eq!(profile.user_agent(), None);
    }

    #[test]
    fn default_profile_keeps_request_optional_max_tokens() {
        let profile = ChatCompletionsRequestProfile::Default;
        let request = ModelRequest::for_stage(ModelStage::Planner, "hello");

        assert_eq!(profile.resolve_max_tokens(&request), None);
        assert!(profile.include_stream_options());
        assert_eq!(profile.user_agent(), None);
    }
}
