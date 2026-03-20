use openjax_protocol::{Event, Op, ThreadId};
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

use crate::agent::prompt::summarize_user_input;
use crate::{Agent, USER_INPUT_LOG_PREVIEW_CHARS, tools};

impl Agent {
    pub async fn submit_with_sink(&mut self, op: Op, sink: UnboundedSender<Event>) -> Vec<Event> {
        self.event_sink = Some(sink);
        let events = self.submit(op).await;
        self.event_sink = None;
        events
    }

    pub async fn submit(&mut self, op: Op) -> Vec<Event> {
        match op {
            Op::UserTurn { input } => {
                let turn_id = self.next_turn_id;
                self.next_turn_id += 1;
                let (input_preview, input_truncated) =
                    summarize_user_input(&input, USER_INPUT_LOG_PREVIEW_CHARS);
                info!(
                    turn_id = turn_id,
                    phase = "received",
                    input_len = input.chars().count(),
                    input_preview = ?input_preview,
                    input_truncated = input_truncated,
                    "user_turn received"
                );
                // Duplicate-call protection should only apply within one user turn.
                // Keeping records across turns can incorrectly block legitimate reads/writes.
                self.recent_tool_calls.clear();
                self.state_epoch = 0;

                let mut events = Vec::new();
                self.push_event(&mut events, Event::TurnStarted { turn_id });

                if let Some(call) = tools::parse_tool_call(&input) {
                    // parse_tool_call only borrows &input, input ownership is preserved
                    if let Some((traces, output)) = self
                        .execute_single_tool_call(turn_id, call, &mut events)
                        .await
                    {
                        self.commit_turn(input.clone(), traces, output);
                    }
                    // None path: all retries exhausted, turn failed, not committed to history
                } else {
                    // else branch (natural language path): history write is entirely handled by planner.rs commit_turn,
                    // turn.rs does no history operations here
                    self.execute_natural_language_turn(turn_id, &input, &mut events)
                        .await;
                }

                self.push_event(&mut events, Event::TurnCompleted { turn_id });
                events
            }
            // Multi-agent operations (预留扩展)
            Op::SpawnAgent {
                input: _,
                source: _,
            } => {
                // Check depth limit
                if self.depth >= tools::MAX_AGENT_DEPTH {
                    return vec![Event::AssistantMessage {
                        turn_id: self.next_turn_id,
                        content: format!(
                            "Cannot spawn sub-agent: max depth {} reached",
                            tools::MAX_AGENT_DEPTH
                        ),
                    }];
                }

                let new_thread_id = ThreadId::new();
                self.next_turn_id += 1;

                vec![Event::AgentSpawned {
                    parent_thread_id: Some(self.thread_id),
                    new_thread_id,
                }]
            }
            Op::SendToAgent {
                thread_id: _,
                input: _,
            } => {
                // 预留扩展：向指定代理发送消息
                vec![Event::AssistantMessage {
                    turn_id: self.next_turn_id,
                    content: "SendToAgent not yet implemented".to_string(),
                }]
            }
            Op::InterruptAgent { thread_id: _ } => {
                // 预留扩展：中断指定代理
                vec![Event::AssistantMessage {
                    turn_id: self.next_turn_id,
                    content: "InterruptAgent not yet implemented".to_string(),
                }]
            }
            Op::ResumeAgent {
                rollout_path: _,
                source: _,
            } => {
                // 预留扩展：从持久化状态恢复代理
                vec![Event::AssistantMessage {
                    turn_id: self.next_turn_id,
                    content: "ResumeAgent not yet implemented".to_string(),
                }]
            }
            Op::Shutdown => vec![Event::ShutdownComplete],
        }
    }
}
