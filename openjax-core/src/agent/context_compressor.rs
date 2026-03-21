use tracing::warn;

use crate::HistoryItem;
use crate::model::{ModelClient, ModelRequest, ModelStage};

/// 将 history 分割为「待摘要」和「保留」两部分。
/// 保留最后 3 个 Turn（含其间的 Summary）。
/// 如果 Turn 总数 <= 4，返回 None（不压缩）。
pub(crate) fn split_for_compression(
    history: &[HistoryItem],
) -> Option<(&[HistoryItem], &[HistoryItem])> {
    let turn_indices: Vec<usize> = history
        .iter()
        .enumerate()
        .filter(|(_, h)| matches!(h, HistoryItem::Turn(_)))
        .map(|(i, _)| i)
        .collect();

    if turn_indices.len() <= 4 {
        return None;
    }

    // 第 (len-3) 个 Turn 的位置是分界点
    let cutoff = turn_indices[turn_indices.len() - 3];
    Some((&history[..cutoff], &history[cutoff..]))
}

/// 将待摘要的 history 序列化为适合放入 prompt 的文本。
fn format_for_prompt(items: &[HistoryItem]) -> String {
    let mut out = String::new();
    for item in items {
        match item {
            HistoryItem::Turn(r) => {
                out.push_str(&format!("User: {}\n", r.user_input));
                if !r.tool_traces.is_empty() {
                    out.push_str("Tools:\n");
                    for t in &r.tool_traces {
                        out.push_str(&format!("  - {}\n", t));
                    }
                }
                out.push_str(&format!("Assistant: {}\n\n", r.assistant_output));
            }
            HistoryItem::Summary(s) => {
                out.push_str(&format!("[PREVIOUS SUMMARY]\n{}\n\n", s));
            }
        }
    }
    out
}

fn build_compression_prompt(history_text: &str, turn_count: usize) -> String {
    format!(
        "You are a context compressor for an AI assistant session. \
Given the following conversation history ({turn_count} turns), produce a concise summary \
in this EXACT format (omit Key Decisions section if none exist):\n\n\
[CONTEXT SUMMARY - covers {turn_count} turns]\n\n\
**Objective**: <one sentence describing the user's main goal>\n\n\
**Key Decisions**:  (omit if none)\n\
- <decision>\n\n\
**Execution Steps**:\n\
1. tool_name(args_summary) → result ✓/✗\n\n\
**Current State**: <one sentence on current status and what remains>\n\n\
Keep the total summary under 400 tokens. Be factual. Preserve tool names and outcomes.\n\n\
--- HISTORY ---\n{history_text}"
    )
}

/// 尝试压缩 history。返回 Some(new_history) 表示压缩成功，None 表示跳过。
pub(crate) async fn try_compact(
    history: &[HistoryItem],
    model_client: &dyn ModelClient,
) -> Option<Vec<HistoryItem>> {
    let (to_summarize, recent) = split_for_compression(history)?;

    let summarize_turn_count = to_summarize
        .iter()
        .filter(|h| matches!(h, HistoryItem::Turn(_)))
        .count();

    if summarize_turn_count == 0 {
        return None;
    }

    let history_text = format_for_prompt(to_summarize);
    let prompt = build_compression_prompt(&history_text, summarize_turn_count);

    let request = ModelRequest::for_stage(ModelStage::Planner, prompt);
    let summary_text = match model_client.complete(&request).await {
        Ok(resp) => resp.text,
        Err(err) => {
            warn!(error = %err, "context_compression_model_call_failed_skipping");
            return None;
        }
    };

    let mut new_history = vec![HistoryItem::Summary(summary_text)];
    new_history.extend_from_slice(recent);
    Some(new_history)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TurnRecord;

    fn turn(user: &str, assistant: &str) -> HistoryItem {
        HistoryItem::Turn(TurnRecord {
            user_input: user.to_string(),
            tool_traces: Vec::new(),
            assistant_output: assistant.to_string(),
        })
    }

    fn summary(s: &str) -> HistoryItem {
        HistoryItem::Summary(s.to_string())
    }

    #[test]
    fn test_no_split_with_four_or_fewer_turns() {
        let h: Vec<HistoryItem> = (0..4)
            .map(|i| turn(&format!("u{i}"), &format!("a{i}")))
            .collect();
        assert!(split_for_compression(&h).is_none());
    }

    #[test]
    fn test_split_with_five_turns_keeps_3_recent() {
        let h: Vec<HistoryItem> = (0..5)
            .map(|i| turn(&format!("u{i}"), &format!("a{i}")))
            .collect();
        let (old, recent) = split_for_compression(&h).unwrap();
        let old_turns = old
            .iter()
            .filter(|x| matches!(x, HistoryItem::Turn(_)))
            .count();
        let recent_turns = recent
            .iter()
            .filter(|x| matches!(x, HistoryItem::Turn(_)))
            .count();
        assert_eq!(old_turns, 2);
        assert_eq!(recent_turns, 3);
    }

    #[test]
    fn test_split_with_seven_turns() {
        let h: Vec<HistoryItem> = (0..7)
            .map(|i| turn(&format!("u{i}"), &format!("a{i}")))
            .collect();
        let (old, recent) = split_for_compression(&h).unwrap();
        let old_turns = old
            .iter()
            .filter(|x| matches!(x, HistoryItem::Turn(_)))
            .count();
        let recent_turns = recent
            .iter()
            .filter(|x| matches!(x, HistoryItem::Turn(_)))
            .count();
        assert_eq!(old_turns, 4);
        assert_eq!(recent_turns, 3);
    }

    #[test]
    fn test_existing_summary_stays_in_old_by_position() {
        // Summary 在边界之前 → 进入 old 部分
        let mut h: Vec<HistoryItem> = (0..5)
            .map(|i| turn(&format!("u{i}"), &format!("a{i}")))
            .collect();
        h.insert(0, summary("old summary"));
        let (old, recent) = split_for_compression(&h).unwrap();
        // old 包含 summary + 2 turns, recent 包含 3 turns
        assert!(old.iter().any(|x| matches!(x, HistoryItem::Summary(_))));
        let recent_turns = recent
            .iter()
            .filter(|x| matches!(x, HistoryItem::Turn(_)))
            .count();
        assert_eq!(recent_turns, 3);
    }

    #[test]
    fn test_format_for_prompt_includes_tool_traces() {
        let h = vec![HistoryItem::Turn(TurnRecord {
            user_input: "fix the bug".to_string(),
            tool_traces: vec!["read_file(main.rs) → ok ✓".to_string()],
            assistant_output: "done".to_string(),
        })];
        let text = format_for_prompt(&h);
        assert!(text.contains("read_file(main.rs)"));
        assert!(text.contains("fix the bug"));
        assert!(text.contains("done"));
    }
}
