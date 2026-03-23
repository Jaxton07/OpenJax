use openjax_policy::schema::{DecisionKind, PolicyDecision};

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
