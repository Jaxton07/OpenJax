use openjax_tui::tui::{AltScreenMode, should_enable_alt_screen};

#[test]
fn alt_screen_mode_always_and_never_are_strict() {
    assert!(should_enable_alt_screen(AltScreenMode::Always));
    assert!(!should_enable_alt_screen(AltScreenMode::Never));
}
