use std::str::FromStr;

use openjax_policy::schema::DecisionKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PolicyLevel {
    Permissive,
    Standard,
    Strict,
}

impl FromStr for PolicyLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allow" => Ok(PolicyLevel::Permissive),
            "ask" => Ok(PolicyLevel::Standard),
            "deny" => Ok(PolicyLevel::Strict),
            other => Err(format!("unknown policy level: {other}")),
        }
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_round_trips() {
        for level in [
            PolicyLevel::Permissive,
            PolicyLevel::Standard,
            PolicyLevel::Strict,
        ] {
            assert_eq!(level.as_str().parse::<PolicyLevel>(), Ok(level));
        }
    }

    #[test]
    fn from_str_returns_err_for_invalid() {
        assert!("unknown".parse::<PolicyLevel>().is_err());
        assert!("".parse::<PolicyLevel>().is_err());
        assert!("invalid".parse::<PolicyLevel>().is_err());
    }

    #[test]
    fn to_decision_kind_maps_correctly() {
        assert_eq!(
            PolicyLevel::Permissive.to_decision_kind(),
            DecisionKind::Allow
        );
        assert_eq!(PolicyLevel::Standard.to_decision_kind(), DecisionKind::Ask);
        assert_eq!(PolicyLevel::Strict.to_decision_kind(), DecisionKind::Deny);
    }
}
