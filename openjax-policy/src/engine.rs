use std::cmp::Ordering;

use crate::schema::{DecisionKind, PolicyDecision, PolicyInput, PolicyRule};

pub fn decide(input: &PolicyInput, rules: &[PolicyRule], default: DecisionKind) -> PolicyDecision {
    let selected = rules
        .iter()
        .filter(|rule| rule.matches(input))
        .max_by(compare_rule_precedence);

    match selected {
        Some(rule) => PolicyDecision {
            kind: rule.decision.clone(),
            matched_rule_id: Some(rule.id.clone()),
            policy_version: input.policy_version,
            reason: rule.reason.clone(),
        },
        None => PolicyDecision {
            kind: default,
            matched_rule_id: None,
            policy_version: input.policy_version,
            reason: "no matching rule; default decision applied".to_string(),
        },
    }
}

fn compare_rule_precedence(left: &&PolicyRule, right: &&PolicyRule) -> Ordering {
    left.priority
        .cmp(&right.priority)
        .then_with(|| left.specificity().cmp(&right.specificity()))
        .then_with(|| {
            left.decision
                .conservative_rank()
                .cmp(&right.decision.conservative_rank())
        })
        // Stable contract for fully equal precedence keys:
        // choose lexicographically smaller rule id so result is deterministic
        // and independent of input ordering.
        .then_with(|| right.id.cmp(&left.id))
}
