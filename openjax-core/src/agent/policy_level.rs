use openjax_policy::schema::DecisionKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PolicyLevel {
    Permissive,
    Standard,
    Strict,
}

impl PolicyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyLevel::Permissive => "allow",
            PolicyLevel::Standard => "ask",
            PolicyLevel::Strict => "deny",
        }
    }

    pub fn to_decision_kind(self) -> DecisionKind {
        match self {
            PolicyLevel::Permissive => DecisionKind::Allow,
            PolicyLevel::Standard => DecisionKind::Ask,
            PolicyLevel::Strict => DecisionKind::Deny,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "allow" => Some(PolicyLevel::Permissive),
            "ask" => Some(PolicyLevel::Standard),
            "deny" => Some(PolicyLevel::Strict),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_round_trips() {
        for level in [PolicyLevel::Permissive, PolicyLevel::Standard, PolicyLevel::Strict] {
            assert_eq!(PolicyLevel::from_str(level.as_str()), Some(level));
        }
    }

    #[test]
    fn from_str_returns_none_for_invalid() {
        assert!(PolicyLevel::from_str("unknown").is_none());
        assert!(PolicyLevel::from_str("").is_none());
        assert!(PolicyLevel::from_str("invalid").is_none());
    }

    #[test]
    fn to_decision_kind_maps_correctly() {
        assert_eq!(PolicyLevel::Permissive.to_decision_kind(), DecisionKind::Allow);
        assert_eq!(PolicyLevel::Standard.to_decision_kind(), DecisionKind::Ask);
        assert_eq!(PolicyLevel::Strict.to_decision_kind(), DecisionKind::Deny);
    }
}
