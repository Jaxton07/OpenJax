use openjax_policy::{
    decide,
    schema::{DecisionKind, PolicyDecision, PolicyInput, PolicyRule},
};

#[test]
fn ask_construction_keeps_metadata() {
    let decision = PolicyDecision::ask("rule-1", 1, "default ask");

    assert_eq!(decision.kind, DecisionKind::Ask);
    assert_eq!(decision.matched_rule_id, Some("rule-1".to_string()));
    assert_eq!(decision.policy_version, 1);
    assert_eq!(decision.reason, "default ask");
}

#[test]
fn ask_unmatched_keeps_rule_id_empty() {
    let decision = PolicyDecision::ask_unmatched(7, "no matching rule");

    assert_eq!(decision.kind, DecisionKind::Ask);
    assert_eq!(decision.matched_rule_id, None);
    assert_eq!(decision.policy_version, 7);
    assert_eq!(decision.reason, "no matching rule");
}

#[test]
fn same_priority_conflict_prefers_safer_decision() {
    let input = PolicyInput {
        tool_name: "shell".to_string(),
        action: "exec".to_string(),
        session_id: None,
        actor: None,
        resource: None,
        capabilities: vec!["process_exec".to_string()],
        risk_tags: vec![],
        policy_version: 3,
    };

    let rules = vec![
        PolicyRule {
            id: "allow-shell".to_string(),
            decision: DecisionKind::Allow,
            priority: 100,
            tool_name: Some("shell".to_string()),
            action: Some("exec".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec![],
            reason: "allow shell exec".to_string(),
        },
        PolicyRule {
            id: "deny-shell".to_string(),
            decision: DecisionKind::Deny,
            priority: 100,
            tool_name: Some("shell".to_string()),
            action: Some("exec".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec![],
            reason: "deny shell exec".to_string(),
        },
    ];

    let decision = decide(&input, &rules, DecisionKind::Ask);
    assert_eq!(decision.kind, DecisionKind::Deny);
    assert_eq!(decision.matched_rule_id.as_deref(), Some("deny-shell"));
    assert_eq!(decision.policy_version, 3);
}

#[test]
fn higher_priority_rule_wins_before_safety_order() {
    let input = base_input();
    let rules = vec![
        base_rule("deny-low-priority", DecisionKind::Deny, 10),
        base_rule("allow-high-priority", DecisionKind::Allow, 20),
    ];

    let decision = decide(&input, &rules, DecisionKind::Ask);
    assert_eq!(decision.kind, DecisionKind::Allow);
    assert_eq!(
        decision.matched_rule_id.as_deref(),
        Some("allow-high-priority")
    );
}

#[test]
fn more_specific_rule_wins_before_safety_order() {
    let input = base_input();
    let rules = vec![
        PolicyRule {
            id: "deny-generic".to_string(),
            decision: DecisionKind::Deny,
            priority: 20,
            tool_name: Some("shell".to_string()),
            action: None,
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec![],
            reason: "generic deny".to_string(),
        },
        PolicyRule {
            id: "allow-specific".to_string(),
            decision: DecisionKind::Allow,
            priority: 20,
            tool_name: Some("shell".to_string()),
            action: Some("exec".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec!["process_exec".to_string()],
            risk_tags_all: vec![],
            reason: "specific allow".to_string(),
        },
    ];

    let decision = decide(&input, &rules, DecisionKind::Ask);
    assert_eq!(decision.kind, DecisionKind::Allow);
    assert_eq!(decision.matched_rule_id.as_deref(), Some("allow-specific"));
}

#[test]
fn no_rule_match_uses_default_decision() {
    let input = base_input();
    let rules = vec![PolicyRule {
        id: "non-matching".to_string(),
        decision: DecisionKind::Deny,
        priority: 100,
        tool_name: Some("read_file".to_string()),
        action: Some("read".to_string()),
        session_id: None,
        actor: None,
        resource: None,
        capabilities_all: vec!["fs_read".to_string()],
        risk_tags_all: vec![],
        reason: "not for this input".to_string(),
    }];

    let decision = decide(&input, &rules, DecisionKind::Escalate);
    assert_eq!(decision.kind, DecisionKind::Escalate);
    assert_eq!(decision.matched_rule_id, None);
    assert_eq!(decision.policy_version, 3);
}

#[test]
fn duplicate_constraints_do_not_increase_specificity() {
    let input = PolicyInput {
        tool_name: "shell".to_string(),
        action: "exec".to_string(),
        session_id: None,
        actor: None,
        resource: None,
        capabilities: vec!["process_exec".to_string()],
        risk_tags: vec!["high-risk".to_string()],
        policy_version: 3,
    };

    let rules = vec![
        PolicyRule {
            id: "allow-with-duplicates".to_string(),
            decision: DecisionKind::Allow,
            priority: 100,
            tool_name: Some("shell".to_string()),
            action: Some("exec".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec!["process_exec".to_string(), "process_exec".to_string()],
            risk_tags_all: vec!["high-risk".to_string(), "high-risk".to_string()],
            reason: "allow with duplicated constraints".to_string(),
        },
        PolicyRule {
            id: "deny-unique".to_string(),
            decision: DecisionKind::Deny,
            priority: 100,
            tool_name: Some("shell".to_string()),
            action: Some("exec".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec!["process_exec".to_string()],
            risk_tags_all: vec!["high-risk".to_string()],
            reason: "deny with unique constraints".to_string(),
        },
    ];

    let decision = decide(&input, &rules, DecisionKind::Ask);
    assert_eq!(decision.kind, DecisionKind::Deny);
    assert_eq!(decision.matched_rule_id.as_deref(), Some("deny-unique"));
}

#[test]
fn same_sort_key_uses_stable_rule_id_tiebreak() {
    let input = base_input();
    let rules = vec![
        base_rule("rule-a", DecisionKind::Allow, 100),
        base_rule("rule-z", DecisionKind::Allow, 100),
    ];

    let decision = decide(&input, &rules, DecisionKind::Ask);
    assert_eq!(decision.kind, DecisionKind::Allow);
    assert_eq!(decision.matched_rule_id.as_deref(), Some("rule-a"));
}

fn base_input() -> PolicyInput {
    PolicyInput {
        tool_name: "shell".to_string(),
        action: "exec".to_string(),
        session_id: None,
        actor: None,
        resource: None,
        capabilities: vec!["process_exec".to_string()],
        risk_tags: vec![],
        policy_version: 3,
    }
}

fn base_rule(id: &str, decision: DecisionKind, priority: i32) -> PolicyRule {
    PolicyRule {
        id: id.to_string(),
        decision,
        priority,
        tool_name: Some("shell".to_string()),
        action: Some("exec".to_string()),
        session_id: None,
        actor: None,
        resource: None,
        capabilities_all: vec![],
        risk_tags_all: vec![],
        reason: format!("{id} decision"),
    }
}
