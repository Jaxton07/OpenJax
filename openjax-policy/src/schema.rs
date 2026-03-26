use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionKind {
    Allow,
    Ask,
    Escalate,
    Deny,
}

impl DecisionKind {
    pub fn conservative_rank(&self) -> u8 {
        match self {
            Self::Allow => 0,
            Self::Ask => 1,
            Self::Escalate => 2,
            Self::Deny => 3,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionKind::Allow => "allow",
            DecisionKind::Ask => "ask",
            DecisionKind::Escalate => "escalate",
            DecisionKind::Deny => "deny",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyInput {
    pub tool_name: String,
    pub action: String,
    pub session_id: Option<String>,
    pub actor: Option<String>,
    pub resource: Option<String>,
    pub capabilities: Vec<String>,
    pub risk_tags: Vec<String>,
    pub policy_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub decision: DecisionKind,
    pub priority: i32,
    pub tool_name: Option<String>,
    pub action: Option<String>,
    pub session_id: Option<String>,
    pub actor: Option<String>,
    pub resource: Option<String>,
    pub capabilities_all: Vec<String>,
    pub risk_tags_all: Vec<String>,
    pub reason: String,
}

impl PolicyRule {
    pub fn matches(&self, input: &PolicyInput) -> bool {
        matches_required_value(&self.tool_name, Some(&input.tool_name))
            && matches_required_value(&self.action, Some(&input.action))
            && matches_required_value(&self.session_id, input.session_id.as_ref())
            && matches_required_value(&self.actor, input.actor.as_ref())
            && matches_required_value(&self.resource, input.resource.as_ref())
            && contains_all(&input.capabilities, &self.capabilities_all)
            && contains_all(&input.risk_tags, &self.risk_tags_all)
    }

    pub fn specificity(&self) -> usize {
        usize::from(self.tool_name.is_some())
            + usize::from(self.action.is_some())
            + usize::from(self.session_id.is_some())
            + usize::from(self.actor.is_some())
            + usize::from(self.resource.is_some())
            + unique_count(&self.capabilities_all)
            + unique_count(&self.risk_tags_all)
    }
}

fn contains_all(source: &[String], required: &[String]) -> bool {
    required
        .iter()
        .all(|expected| source.iter().any(|actual| actual == expected))
}

fn matches_required_value(required: &Option<String>, actual: Option<&String>) -> bool {
    match required {
        None => true,
        Some(expected) => actual == Some(expected),
    }
}

fn unique_count(values: &[String]) -> usize {
    values
        .iter()
        .map(|value| value.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_kind_as_str_returns_correct_strings() {
        assert_eq!(DecisionKind::Allow.as_str(), "allow");
        assert_eq!(DecisionKind::Ask.as_str(), "ask");
        assert_eq!(DecisionKind::Escalate.as_str(), "escalate");
        assert_eq!(DecisionKind::Deny.as_str(), "deny");
    }
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
