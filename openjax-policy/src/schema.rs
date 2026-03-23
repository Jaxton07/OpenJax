use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionKind {
    Allow,
    Ask,
    Escalate,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub kind: DecisionKind,
    pub matched_rule_id: Option<String>,
    pub policy_version: u64,
    pub reason: String,
}

impl PolicyDecision {
    pub fn ask(
        matched_rule_id: impl Into<String>,
        policy_version: u64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            kind: DecisionKind::Ask,
            matched_rule_id: Some(matched_rule_id.into()),
            policy_version,
            reason: reason.into(),
        }
    }

    pub fn ask_unmatched(policy_version: u64, reason: impl Into<String>) -> Self {
        Self {
            kind: DecisionKind::Ask,
            matched_rule_id: None,
            policy_version,
            reason: reason.into(),
        }
    }
}
