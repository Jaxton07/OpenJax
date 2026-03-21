use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(crate) fn word_wrap_lines_borrowed<'a, I, O>(lines: I, width_or_options: O) -> Vec<Line<'a>>
where
    I: IntoIterator<Item = &'a Line<'a>>,
    O: Into<usize>,
{
    let width = width_or_options.into().max(1);
    let mut out = Vec::new();
    for line in lines {
        out.extend(word_wrap_line(line, width));
    }
    out
}

fn word_wrap_line<'a>(line: &'a Line<'a>, width: usize) -> Vec<Line<'a>> {
    let mut result = Vec::new();
    let mut current: Vec<Span<'a>> = Vec::new();
    let mut current_w = 0usize;

    for span in &line.spans {
        let mut remaining = span.content.as_ref();
        while !remaining.is_empty() {
            let (take, take_bytes) = take_fit_prefix(remaining, width.saturating_sub(current_w));
            if take.is_empty() {
                if !current.is_empty() {
                    result.push(Line::from(std::mem::take(&mut current)).style(line.style));
                    current_w = 0;
                    continue;
                }
                if let Some(ch) = remaining.chars().next() {
                    let s = ch.to_string();
                    current.push(Span::styled(s.clone(), span.style));
                    remaining = &remaining[s.len()..];
                    result.push(Line::from(std::mem::take(&mut current)).style(line.style));
                    current_w = 0;
                } else {
                    break;
                }
                continue;
            }
            current_w += UnicodeWidthStr::width(take);
            current.push(Span::styled(take.to_string(), span.style));
            remaining = &remaining[take_bytes..];
            if current_w >= width {
                result.push(Line::from(std::mem::take(&mut current)).style(line.style));
                current_w = 0;
            }
        }
    }

    if !current.is_empty() {
        result.push(Line::from(current).style(line.style));
    }
    if result.is_empty() {
        result.push(line.clone());
    }
    result
}

fn take_fit_prefix(text: &str, remaining_width: usize) -> (&str, usize) {
    if remaining_width == 0 {
        return ("", 0);
    }
    let mut width = 0usize;
    let mut end = 0usize;
    for (idx, ch) in text.char_indices() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if width + w > remaining_width {
            break;
        }
        width += w;
        end = idx + ch.len_utf8();
    }
    (&text[..end], end)
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use unicode_width::UnicodeWidthStr;

    use super::word_wrap_lines_borrowed;

    #[test]
    fn cjk_wrap_respects_width_limit() {
        let line = Line::from("你好世界");
        let wrapped = word_wrap_lines_borrowed([&line], 3usize);
        assert!(
            wrapped.len() >= 2,
            "expected wrapping at narrow width for CJK text",
        );
        for row in wrapped {
            assert!(
                UnicodeWidthStr::width(row.to_string().as_str()) <= 3,
                "wrapped row exceeds target width",
            );
        }
    }

    #[test]
    fn cjk_wrap_preserves_span_style() {
        let style = Style::default().fg(Color::Green);
        let line = Line::from(vec![Span::styled("你好世界", style)]);
        let wrapped = word_wrap_lines_borrowed([&line], 3usize);
        assert!(wrapped.len() >= 2);
        assert!(
            wrapped
                .iter()
                .all(|row| row.spans.iter().all(|span| span.style == style))
        );
    }
}
