#[test]
fn inline_height_env_is_optional() {
    let parsed = std::env::var("OPENJAX_TUI_INLINE_HEIGHT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(16);
    assert!(parsed >= 1);
}
