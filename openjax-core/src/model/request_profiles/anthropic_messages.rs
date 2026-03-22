use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnthropicMessagesRequestProfile {
    Default,
}

impl AnthropicMessagesRequestProfile {
    pub(crate) fn parse(raw: Option<&str>) -> Result<Self> {
        match raw.unwrap_or("default").trim() {
            "" | "default" | "anthropic_default" => Ok(Self::Default),
            other => Err(anyhow!(
                "unknown anthropic_messages request_profile '{other}'; supported profiles: default, anthropic_default"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AnthropicMessagesRequestProfile;

    #[test]
    fn default_profile_parses_explicit_name() {
        let profile = AnthropicMessagesRequestProfile::parse(Some("default"))
            .expect("default anthropic profile");
        assert_eq!(profile, AnthropicMessagesRequestProfile::Default);
    }

    #[test]
    fn default_profile_parses_anthropic_default_alias() {
        let profile = AnthropicMessagesRequestProfile::parse(Some("anthropic_default"))
            .expect("anthropic default alias");
        assert_eq!(profile, AnthropicMessagesRequestProfile::Default);
    }
}
