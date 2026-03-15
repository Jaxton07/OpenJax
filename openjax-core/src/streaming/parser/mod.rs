pub mod anthropic;
pub mod openai;

use anyhow::Result;

pub trait SseParser {
    fn push_chunk(&mut self, bytes: &[u8]) -> Result<Vec<String>>;
    fn finish(&mut self) -> Result<Vec<String>>;
    fn saw_done_marker(&self) -> bool;
}

pub(crate) fn take_complete_lines(pending: &mut Vec<u8>) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(pos) = pending.iter().position(|b| *b == 10) {
        let mut line = pending.drain(..=pos).collect::<Vec<u8>>();
        if matches!(line.last(), Some(10)) {
            let _ = line.pop();
        }
        if matches!(line.last(), Some(13)) {
            let _ = line.pop();
        }
        out.push(String::from_utf8_lossy(&line).to_string());
    }
    out
}

pub(crate) fn parse_sse_data_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with("data:") {
        return None;
    }
    Some(trimmed[5..].trim())
}
