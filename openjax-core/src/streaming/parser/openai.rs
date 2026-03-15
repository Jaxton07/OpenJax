use anyhow::Result;

use super::{SseParser, parse_sse_data_line, take_complete_lines};

#[derive(Debug, Default)]
pub struct OpenAiSseParser {
    pending: Vec<u8>,
    saw_done: bool,
}

impl SseParser for OpenAiSseParser {
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
    use super::{OpenAiSseParser, SseParser};

    #[test]
    fn parser_handles_chunk_split_and_done_marker() {
        let mut parser = OpenAiSseParser::default();
        let first = parser
            .push_chunk(b"data: {\"x\":1}\n\ndata: [DO")
            .expect("chunk parse");
        assert_eq!(first, vec!["{\"x\":1}".to_string()]);

        let second = parser.push_chunk(b"NE]\n").expect("second chunk parse");
        assert!(second.is_empty());
        assert!(parser.finish().expect("finish").is_empty());
        assert!(parser.saw_done_marker());
    }

    #[test]
    fn parser_reports_missing_done_marker() {
        let mut parser = OpenAiSseParser::default();
        let frames = parser.push_chunk(b"data: {\"x\":1}\n").expect("parse");
        assert_eq!(frames, vec!["{\"x\":1}".to_string()]);
        assert!(parser.finish().expect("finish").is_empty());
        assert!(!parser.saw_done_marker());
    }
}
