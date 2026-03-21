use std::collections::{BTreeMap, HashMap};

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
            args: stable_args_json(args),
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
            args: stable_args_json(args),
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

    /// 公开接口：手动触发压缩（gateway /compact 调用）
    pub async fn compact(&mut self, events: &mut Vec<openjax_protocol::Event>) {
        self.do_compact(0, events).await;
    }

    /// 内部：执行一次压缩并推送事件
    pub(crate) async fn do_compact(
        &mut self,
        turn_id: u64,
        events: &mut Vec<openjax_protocol::Event>,
    ) {
        let turns_before = self
            .history
            .iter()
            .filter(|h| matches!(h, crate::HistoryItem::Turn(_)))
            .count();

        match crate::agent::context_compressor::try_compact(&self.history, &*self.model_client)
            .await
        {
            Some(new_history) => {
                let turns_after = new_history
                    .iter()
                    .filter(|h| matches!(h, crate::HistoryItem::Turn(_)))
                    .count();
                let summary_preview = new_history
                    .iter()
                    .find_map(|h| {
                        if let crate::HistoryItem::Summary(s) = h {
                            Some(s.chars().take(120).collect::<String>())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                self.history = new_history;
                self.last_input_tokens = None; // 历史已变，重置估算

                self.push_event(
                    events,
                    openjax_protocol::Event::ContextCompacted {
                        turn_id,
                        compressed_turns: (turns_before - turns_after) as u32,
                        retained_turns: turns_after as u32,
                        summary_preview,
                    },
                );
                tracing::info!(
                    turn_id = turn_id,
                    compressed = turns_before - turns_after,
                    retained = turns_after,
                    "context_compacted"
                );
            }
            None => {
                tracing::info!(turn_id = turn_id, "compact_skipped_insufficient_history");
            }
        }
    }

    /// 内部：检查 token 用量占比，超阈值时自动触发压缩
    pub(crate) async fn check_and_auto_compact(
        &mut self,
        turn_id: u64,
        events: &mut Vec<openjax_protocol::Event>,
    ) {
        if self.context_window_size == 0 {
            return; // 未知上下文窗口，跳过
        }

        let token_estimate = self.last_input_tokens.unwrap_or_else(|| {
            // Fallback：从 history 字符数估算（1 token ≈ 3.5 chars）
            let chars: usize = self
                .history
                .iter()
                .map(|h| match h {
                    crate::HistoryItem::Turn(r) => {
                        r.user_input.len()
                            + r.assistant_output.len()
                            + r.tool_traces.iter().map(|t| t.len()).sum::<usize>()
                    }
                    crate::HistoryItem::Summary(s) => s.len(),
                })
                .sum();
            (chars as f64 / 3.5) as u64
        });

        let ratio = token_estimate as f64 / self.context_window_size as f64;

        self.push_event(
            events,
            openjax_protocol::Event::ContextUsageUpdated {
                turn_id,
                input_tokens: token_estimate,
                context_window_size: self.context_window_size,
                ratio,
            },
        );

        if ratio < 0.75 {
            return;
        }

        tracing::info!(
            turn_id = turn_id,
            token_estimate = token_estimate,
            context_window = self.context_window_size,
            ratio = ratio,
            "auto_compact_triggered"
        );
        self.do_compact(turn_id, events).await;
    }
}

fn should_skip_duplicate_detection(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read_file" | "list_dir" | "grep_files" | "process_snapshot" | "system_load" | "disk_usage"
    )
}

fn stable_args_json(args: &HashMap<String, String>) -> String {
    let ordered: BTreeMap<&str, &str> =
        args.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    serde_json::to_string(&ordered).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::stable_args_json;

    #[test]
    fn stable_args_json_is_order_insensitive() {
        let mut first = HashMap::new();
        first.insert("a".to_string(), "1".to_string());
        first.insert("b".to_string(), "2".to_string());

        let mut second = HashMap::new();
        second.insert("b".to_string(), "2".to_string());
        second.insert("a".to_string(), "1".to_string());

        assert_eq!(stable_args_json(&first), stable_args_json(&second));
    }
}
