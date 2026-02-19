use openjax_tui::render::markdown::render_markdown_as_plain_text;

#[test]
fn markdown_render_converts_headings_lists_and_code_blocks() {
    let input = "# Title\n- item\n```rust\nlet x = 1;\n```";
    let rendered = render_markdown_as_plain_text(input);

    assert!(rendered.contains("H1: Title"));
    assert!(rendered.contains("• item"));
    assert!(rendered.contains("[code]"));
    assert!(rendered.contains("    let x = 1;"));
    assert!(rendered.contains("[/code]"));
}
