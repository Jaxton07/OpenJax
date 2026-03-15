use anyhow::Result;

use super::{SseParser, parse_sse_data_line, take_complete_lines};

#[derive(Debug, Default)]
pub struct AnthropicSseParser {
    pending: Vec<u8>,
    saw_done: bool,
}

impl SseParser for AnthropicSseParser {
    fn push_chunk(&mut self, bytes: &[u8]) -> Result<Vec<String>> {
        self.pending.extend_from_slice(bytes);
        let mut frames = Vec::new();
        for line in take_complete_lines(&mut self.pending) {
            let Some(data) = parse_sse_data_line(&line) else {
                continue;
            };
            if data == "[DONE]" {
                self.saw_done = true;
                continue;
            }
            frames.push(data.to_string());
        }
        Ok(frames)
    }

    fn finish(&mut self) -> Result<Vec<String>> {
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }
        let trailing = String::from_utf8_lossy(&self.pending).to_string();
        self.pending.clear();
        let Some(data) = parse_sse_data_line(&trailing) else {
            return Ok(Vec::new());
        };
        if data == "[DONE]" {
            self.saw_done = true;
            return Ok(Vec::new());
        }
        Ok(vec![data.to_string()])
    }

    fn saw_done_marker(&self) -> bool {
        self.saw_done
    }
}

#[cfg(test)]
mod tests {
    use super::{AnthropicSseParser, SseParser};

    #[test]
    fn parser_flushes_trailing_data_on_finish() {
        let mut parser = AnthropicSseParser::default();
        let frames = parser
            .push_chunk(b"event: ping\ndata: {\"delta\":{\"text\":\"he\"}}\n")
            .expect("push chunk");
        assert_eq!(frames, vec!["{\"delta\":{\"text\":\"he\"}}".to_string()]);

        let pending = parser
            .push_chunk(b"data: {\"delta\":{\"text\":\"llo\"}}")
            .expect("pending chunk");
        assert!(pending.is_empty());

        let trailing = parser.finish().expect("finish");
        assert_eq!(trailing, vec!["{\"delta\":{\"text\":\"llo\"}}".to_string()]);
        assert!(!parser.saw_done_marker());
    }

    #[test]
    fn parser_tracks_done_marker() {
        let mut parser = AnthropicSseParser::default();
        let frames = parser.push_chunk(b"data: [DONE]\n").expect("parse");
        assert!(frames.is_empty());
        assert!(parser.finish().expect("finish").is_empty());
        assert!(parser.saw_done_marker());
    }
}
