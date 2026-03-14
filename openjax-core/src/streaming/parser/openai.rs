use anyhow::{Result, anyhow};

use super::{SseParser, parse_sse_data_line, take_complete_lines};

#[derive(Debug, Default)]
pub struct OpenAiSseParser {
    pending: Vec<u8>,
}

impl SseParser for OpenAiSseParser {
    fn push_chunk(&mut self, bytes: &[u8]) -> Result<Vec<String>> {
        self.pending.extend_from_slice(bytes);
        let mut deltas = Vec::new();
        for line in take_complete_lines(&mut self.pending) {
            let Some(data) = parse_sse_data_line(&line) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let payload: serde_json::Value = serde_json::from_str(data)
                .map_err(|err| anyhow!("openai stream json parse failed: {err}"))?;
            let content = payload
                .get("choices")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.get("delta"))
                .and_then(|v| v.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !content.is_empty() {
                deltas.push(content.to_string());
            }
        }
        Ok(deltas)
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
            return Ok(Vec::new());
        }
        let payload: serde_json::Value = serde_json::from_str(data)
            .map_err(|err| anyhow!("openai trailing stream json parse failed: {err}"))?;
        let content = payload
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("delta"))
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if content.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(vec![content.to_string()])
        }
    }
}
