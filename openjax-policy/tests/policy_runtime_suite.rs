use openjax_policy::{
    overlay::SessionOverlay,
    runtime::PolicyRuntime,
    schema::{DecisionKind, PolicyInput, PolicyRule},
    store::PolicyStore,
};
use std::{sync::Arc, thread};

#[test]
fn inflight_call_keeps_original_policy_version() {
    let runtime = PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![base_rule("allow-v1", DecisionKind::Allow)],
    ));
    let inflight = runtime.handle();

    let input = base_input(Some("session-a"));
    let before_publish = inflight.decide(&input);
    assert_eq!(before_publish.policy_version, 1);
    assert_eq!(before_publish.kind, DecisionKind::Allow);
    assert_eq!(before_publish.matched_rule_id.as_deref(), Some("allow-v1"));

    runtime.publish(PolicyStore::new(
        DecisionKind::Ask,
        vec![base_rule("deny-v2", DecisionKind::Deny)],
    ));

    let old_handle_after_publish = inflight.decide(&input);
    assert_eq!(old_handle_after_publish.policy_version, 1);
    assert_eq!(old_handle_after_publish.kind, DecisionKind::Allow);
    assert_eq!(
        old_handle_after_publish.matched_rule_id.as_deref(),
        Some("allow-v1")
    );

    let new_handle = runtime.handle();
    let new_handle_after_publish = new_handle.decide(&input);
    assert_eq!(new_handle_after_publish.policy_version, 2);
    assert_eq!(new_handle_after_publish.kind, DecisionKind::Deny);
    assert_eq!(
        new_handle_after_publish.matched_rule_id.as_deref(),
        Some("deny-v2")
    );
}

#[test]
fn session_overlay_only_affects_target_session() {
    let runtime = PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![base_rule("allow-global", DecisionKind::Allow)],
    ));

    runtime.set_session_overlay(
        "session-a",
        SessionOverlay::new(vec![base_rule("deny-overlay", DecisionKind::Deny)]),
    );

    let handle = runtime.handle();

    let session_a = handle.decide(&base_input(Some("session-a")));
    assert_eq!(session_a.kind, DecisionKind::Deny);
    assert_eq!(session_a.matched_rule_id.as_deref(), Some("deny-overlay"));

    let session_b = handle.decide(&base_input(Some("session-b")));
    assert_eq!(session_b.kind, DecisionKind::Allow);
    assert_eq!(session_b.matched_rule_id.as_deref(), Some("allow-global"));
}

#[test]
fn clearing_session_overlay_restores_global_policy() {
    let runtime = PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![base_rule("allow-global", DecisionKind::Allow)],
    ));

    runtime.set_session_overlay(
        "session-a",
        SessionOverlay::new(vec![base_rule("deny-overlay", DecisionKind::Deny)]),
    );

    let with_overlay = runtime.handle().decide(&base_input(Some("session-a")));
    assert_eq!(with_overlay.kind, DecisionKind::Deny);

    runtime.clear_session_overlay("session-a");

    let after_clear = runtime.handle().decide(&base_input(Some("session-a")));
    assert_eq!(after_clear.kind, DecisionKind::Allow);
    assert_eq!(after_clear.matched_rule_id.as_deref(), Some("allow-global"));
}

#[test]
fn concurrent_publish_overlay_and_decide_keeps_runtime_available() {
    let runtime = Arc::new(PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![base_rule("allow-v1", DecisionKind::Allow)],
    )));

    let updater_runtime = Arc::clone(&runtime);
    let updater = thread::spawn(move || {
        for i in 0..300 {
            let global_id = format!("global-{i}");
            let global_decision = if i % 2 == 0 {
                DecisionKind::Allow
            } else {
                DecisionKind::Deny
            };

            updater_runtime.publish(PolicyStore::new(
                DecisionKind::Ask,
                vec![base_rule(&global_id, global_decision)],
            ));

            let overlay_id = format!("overlay-{i}");
            updater_runtime.set_session_overlay(
                "session-a",
                SessionOverlay::new(vec![base_rule(&overlay_id, DecisionKind::Deny)]),
            );
            updater_runtime.clear_session_overlay("session-a");
        }
    });

    let mut readers = Vec::new();
    for _ in 0..4 {
        let reader_runtime = Arc::clone(&runtime);
        readers.push(thread::spawn(move || {
            for _ in 0..1000 {
                let decision = reader_runtime
                    .handle()
                    .decide(&base_input(Some("session-a")));
                assert!(decision.policy_version >= 1);
            }
        }));
    }

    updater.join().expect("updater thread panicked");
    for reader in readers {
        reader.join().expect("reader thread panicked");
    }

    assert!(runtime.current_version() > 1);
}

#[test]
fn inflight_handle_stays_snapshot_consistent_during_concurrent_updates() {
    let runtime = Arc::new(PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![base_rule("allow-v1", DecisionKind::Allow)],
    )));
    let inflight = runtime.handle();

    let updater_runtime = Arc::clone(&runtime);
    let updater = thread::spawn(move || {
        for i in 0..400 {
            let global_id = format!("global-{i}");
            updater_runtime.publish(PolicyStore::new(
                DecisionKind::Ask,
                vec![base_rule(&global_id, DecisionKind::Deny)],
            ));
            updater_runtime.set_session_overlay(
                "session-a",
                SessionOverlay::new(vec![base_rule("overlay-deny", DecisionKind::Deny)]),
            );
            updater_runtime.clear_session_overlay("session-a");
        }
    });

    for _ in 0..1500 {
        let decision = inflight.decide(&base_input(Some("session-a")));
        assert_eq!(decision.policy_version, 1);
        assert_eq!(decision.kind, DecisionKind::Allow);
        assert_eq!(decision.matched_rule_id.as_deref(), Some("allow-v1"));
    }

    updater.join().expect("updater thread panicked");

    let fresh = runtime.handle();
    assert!(fresh.policy_version() > 1);
}

fn base_input(session_id: Option<&str>) -> PolicyInput {
    PolicyInput {
        tool_name: "shell".to_string(),
        action: "exec".to_string(),
        session_id: session_id.map(ToOwned::to_owned),
        actor: None,
        resource: None,
        capabilities: vec!["process_exec".to_string()],
        risk_tags: vec![],
        policy_version: 0,
    }
}

fn base_rule(id: &str, decision: DecisionKind) -> PolicyRule {
    PolicyRule {
        id: id.to_string(),
        decision,
        priority: 100,
        tool_name: Some("shell".to_string()),
        action: Some("exec".to_string()),
        session_id: None,
        actor: None,
        resource: None,
        capabilities_all: vec![],
        risk_tags_all: vec![],
        reason: format!("{id}"),
    }
}
