use openjax_tui::tui::restore_plan;

#[test]
fn terminal_restore_plan_includes_raw_and_alt_cleanup() {
    let plan = restore_plan(true, true);
    assert_eq!(
        plan,
        vec!["show_cursor", "leave_alt_screen", "disable_raw_mode"]
    );
}

#[test]
fn terminal_restore_plan_without_alt_or_raw_is_minimal() {
    let plan = restore_plan(false, false);
    assert_eq!(plan, vec!["show_cursor"]);
}
