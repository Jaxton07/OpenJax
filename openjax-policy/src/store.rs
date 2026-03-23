use crate::schema::{DecisionKind, PolicyRule};

#[derive(Debug, Clone)]
pub struct PolicyStore {
    pub default_decision: DecisionKind,
    pub rules: Vec<PolicyRule>,
}

impl PolicyStore {
    pub fn new(default_decision: DecisionKind, rules: Vec<PolicyRule>) -> Self {
        Self {
            default_decision,
            rules,
        }
    }
}
