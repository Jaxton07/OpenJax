use crate::schema::{DecisionKind, PolicyDecision};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyAuditRecord {
    pub policy_version: u64,
    pub matched_rule_id: Option<String>,
    pub decision: DecisionKind,
    pub reason: String,
}

impl From<&PolicyDecision> for PolicyAuditRecord {
    fn from(decision: &PolicyDecision) -> Self {
        Self {
            policy_version: decision.policy_version,
            matched_rule_id: decision.matched_rule_id.clone(),
            decision: decision.kind.clone(),
            reason: decision.reason.clone(),
        }
    }
}
