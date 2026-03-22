pub(crate) mod anthropic_messages;
pub(crate) mod chat_completions;

#[cfg(test)]
mod tests {
    use super::anthropic_messages::AnthropicMessagesRequestProfile;
    use super::chat_completions::ChatCompletionsRequestProfile;

    #[test]
    fn chat_profile_defaults_when_missing() {
        let profile = ChatCompletionsRequestProfile::parse(None).expect("default profile");
        assert_eq!(profile, ChatCompletionsRequestProfile::Default);
    }

    #[test]
    fn chat_profile_supports_kimi_coding_v1() {
        let profile =
            ChatCompletionsRequestProfile::parse(Some("kimi_coding_v1")).expect("kimi profile");
        assert_eq!(profile, ChatCompletionsRequestProfile::KimiCodingV1);
    }

    #[test]
    fn chat_profile_rejects_unknown_profile() {
        let err = ChatCompletionsRequestProfile::parse(Some("unknown_profile"))
            .expect_err("unknown profile should fail");
        assert!(
            err.to_string()
                .contains("unknown chat_completions request_profile")
        );
        assert!(err.to_string().contains("unknown_profile"));
    }

    #[test]
    fn anthropic_profile_defaults_when_missing() {
        let profile = AnthropicMessagesRequestProfile::parse(None).expect("default profile");
        assert_eq!(profile, AnthropicMessagesRequestProfile::Default);
    }

    #[test]
    fn anthropic_profile_rejects_unknown_profile() {
        let err = AnthropicMessagesRequestProfile::parse(Some("kimi_coding_v1"))
            .expect_err("unsupported anthropic profile should fail");
        assert!(
            err.to_string()
                .contains("unknown anthropic_messages request_profile")
        );
        assert!(err.to_string().contains("kimi_coding_v1"));
    }
}
