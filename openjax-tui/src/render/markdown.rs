use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};

pub fn render_markdown_as_plain_text(input: &str) -> String {
    let mut out = String::new();
    let mut in_code_block = false;
    let mut list_depth = 0usize;

    for ev in Parser::new(input) {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => match level {
                HeadingLevel::H1 => out.push_str("H1: "),
                HeadingLevel::H2 => out.push_str("H2: "),
                _ => out.push_str("H: "),
            },
            Event::End(TagEnd::Heading(_)) => out.push('\n'),
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                out.push_str("[code]");
                if let CodeBlockKind::Fenced(lang) = kind {
                    if !lang.is_empty() {
                        out.push(' ');
                        out.push_str(lang.as_ref());
                    }
                }
                out.push('\n');
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                out.push_str("[/code]\n");
            }
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                out.push('\n');
            }
            Event::Start(Tag::Item) => {
                out.push_str(&"  ".repeat(list_depth.saturating_sub(1)));
                out.push_str("• ");
            }
            Event::End(TagEnd::Item) => out.push('\n'),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Code(text) => {
                out.push('`');
                out.push_str(text.as_ref());
                out.push('`');
            }
            Event::Text(text) => {
                if in_code_block {
                    for line in text.lines() {
                        out.push_str("    ");
                        out.push_str(line);
                        out.push('\n');
                    }
                } else {
                    out.push_str(text.as_ref());
                }
            }
            _ => {}
        }
    }

    let mut normalized = String::new();
    for line in out.lines() {
        if line.starts_with("[code]") || line.starts_with("[/code]") || line.starts_with("    ") {
            normalized.push_str(line);
            normalized.push('\n');
        } else if line.trim().is_empty() {
            normalized.push('\n');
        } else {
            normalized.push_str(line);
            normalized.push('\n');
        }
    }
    while normalized.ends_with('\n') {
        normalized.pop();
    }
    normalized
}
