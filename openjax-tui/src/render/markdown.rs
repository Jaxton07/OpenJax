pub fn render_markdown_as_plain_text(input: &str) -> String {
    let mut out = String::new();
    let mut in_code_block = false;

    for line in input.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            if in_code_block {
                out.push_str("[code]\n");
            } else {
                out.push_str("[/code]\n");
            }
            continue;
        }

        if in_code_block {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("# ") {
            out.push_str("H1: ");
            out.push_str(rest);
            out.push('\n');
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            out.push_str("H2: ");
            out.push_str(rest);
            out.push('\n');
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            out.push_str("• ");
            out.push_str(rest);
            out.push('\n');
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    while out.ends_with('\n') {
        out.pop();
    }
    out
}
