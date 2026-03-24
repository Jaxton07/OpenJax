use crate::schema::{DecisionKind, PolicyRule};

#[derive(Debug, Clone)]
pub struct PolicyStore {
    pub default_decision: DecisionKind,
    pub rules: Vec<PolicyRule>,
}

impl PolicyStore {
    pub fn new(default_decision: DecisionKind, mut rules: Vec<PolicyRule>) -> Self {
        // Insert system built-in rules (high priority)
        let system_destructive = PolicyRule {
            id: "system:destructive_escalate".to_string(),
            decision: DecisionKind::Escalate,
            priority: 1000,
            tool_name: None,
            action: None,
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec!["destructive".to_string()],
            reason: "destructive commands always require escalation approval".to_string(),
        };
        rules.push(system_destructive);
        Self {
            default_decision,
            rules,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PolicyInput;

    #[test]
    fn policy_store_has_builtin_destructive_escalate_rule() {
        let store = PolicyStore::new(DecisionKind::Ask, vec![]);
        let rule = store.rules.iter().find(|r| r.id == "system:destructive_escalate");
        assert!(
            rule.is_some(),
            "system:destructive_escalate rule must exist"
        );
        let rule = rule.unwrap();
        assert_eq!(rule.decision, DecisionKind::Escalate);
        assert_eq!(rule.priority, 1000);
        assert!(rule.risk_tags_all.contains(&"destructive".to_string()));
    }

    #[test]
    fn destructive_command_triggers_escalate_via_policy_center() {
        use crate::decide;

        let store = PolicyStore::new(DecisionKind::Ask, vec![]);
        let input = PolicyInput {
            tool_name: "shell".to_string(),
            action: "exec".to_string(),
            session_id: None,
            actor: None,
            resource: None,
            capabilities: vec![],
            risk_tags: vec!["destructive".to_string()],
            policy_version: 0,
        };
        let decision = decide(&input, &store.rules, store.default_decision);
        assert_eq!(decision.kind, DecisionKind::Escalate);
    }
}
