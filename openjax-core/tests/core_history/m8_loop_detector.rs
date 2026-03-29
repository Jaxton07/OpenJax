//! Integration tests for the LoopDetector mechanism.
//! Run with: zsh -lc "cargo test -p openjax-core --test core_history_suite loop_detector"

use openjax_core::{LoopDetector, LoopSignal};

#[test]
fn test_loop_detector_full_flow() {
    let mut detector = LoopDetector::new();

    // Simulate 5 identical calls -> Warned
    for i in 0..5 {
        let result = detector.check_and_advance("Read", "hash_same");
        if i < 4 {
            assert_eq!(result, LoopSignal::None);
        } else {
            assert_eq!(result, LoopSignal::Warned);
        }
    }

    // recovery prompt should be available
    assert!(detector.recovery_prompt().is_some());

    // Different tool call resets
    let reset_result = detector.check_and_advance("write_file", "hash_other");
    assert_eq!(reset_result, LoopSignal::None);
    assert_eq!(detector.current_state(), &LoopSignal::None);
    assert!(detector.recovery_prompt().is_none());

    // Simulate 5 identical calls again -> Warned
    for _ in 0..5 {
        detector.check_and_advance("grep", "hash_grep");
    }

    // Same tool again -> Halt
    assert_eq!(
        detector.check_and_advance("grep", "hash_grep"),
        LoopSignal::Halt
    );
}

#[test]
fn test_diverse_calls_never_trigger() {
    // LoopDetector with window_capacity=16 should not trigger
    // even with many diverse calls (unique hashes each time)
    let mut detector = LoopDetector::new();
    for i in 0..20 {
        detector.check_and_advance("bash", &format!("hash_{}", i));
    }
    assert_eq!(detector.current_state(), &LoopSignal::None);
}
