use std::collections::HashMap;

use tracing::debug;

use crate::{Agent, HistoryItem, MAX_CONVERSATION_HISTORY_TURNS, TurnRecord};

#[derive(Debug, Clone)]
pub(crate) struct RateLimitConfig {
    pub(crate) min_delay_between_requests_ms: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            min_delay_between_requests_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ToolCallKey {
    pub(crate) name: String,
    pub(crate) args: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolCallRecord {
    pub(crate) key: ToolCallKey,
    pub(crate) ok: bool,
    pub(crate) epoch: u64,
    pub(crate) _output: String,
}

impl Agent {
    pub(crate) async fn apply_rate_limit(&mut self) {
        if let Some(last_time) = self.last_api_call_time {
            let elapsed = last_time.elapsed();
            let min_delay = std::time::Duration::from_millis(
                self.rate_limit_config.min_delay_between_requests_ms,
            );

            if elapsed < min_delay {
                let delay = min_delay - elapsed;
                debug!(
                    delay_ms = delay.as_millis(),
                    "rate_limit: delaying API call"
                );
                tokio::time::sleep(delay).await;
            }
        }
        self.last_api_call_time = Some(std::time::Instant::now());
    }

    pub(crate) fn is_duplicate_tool_call(
        &self,
        tool_name: &str,
        args: &HashMap<String, String>,
    ) -> bool {
        if should_skip_duplicate_detection(tool_name) {
            return false;
        }

        let key = ToolCallKey {
            name: tool_name.to_string(),
            args: serde_json::to_string(args).unwrap_or_default(),
        };

        self.recent_tool_calls
            .iter()
            .any(|record| record.key == key && record.ok && record.epoch == self.state_epoch)
    }

    pub(crate) fn record_tool_call(
        &mut self,
        tool_name: &str,
        args: &HashMap<String, String>,
        ok: bool,
        output: &str,
    ) {
        let key = ToolCallKey {
            name: tool_name.to_string(),
            args: serde_json::to_string(args).unwrap_or_default(),
        };

        self.recent_tool_calls.push(ToolCallRecord {
            key,
            ok,
            epoch: self.state_epoch,
            _output: output.to_string(),
        });

        while self.recent_tool_calls.len() > 16 {
            self.recent_tool_calls.remove(0);
        }
    }

    pub(crate) fn commit_turn(
        &mut self,
        user_input: String,
        tool_traces: Vec<String>,
        assistant_output: String,
    ) {
        self.history.push(HistoryItem::Turn(TurnRecord {
            user_input,
            tool_traces,
            assistant_output,
        }));

        // Only count Turn items; Summary items don't count against the quota
        let turn_count = self
            .history
            .iter()
            .filter(|h| matches!(h, HistoryItem::Turn(_)))
            .count();

        if turn_count > MAX_CONVERSATION_HISTORY_TURNS {
            let overflow = turn_count - MAX_CONVERSATION_HISTORY_TURNS;
            let mut removed = 0;
            self.history.retain(|h| {
                if removed < overflow && matches!(h, HistoryItem::Turn(_)) {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }
    }
}

fn should_skip_duplicate_detection(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read_file" | "list_dir" | "grep_files" | "process_snapshot" | "system_load" | "disk_usage"
    )
}
