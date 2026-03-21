use openjax_core::{Agent, ApprovalPolicy, SandboxMode, tools};
use openjax_protocol::{AgentSource, Event, Op, StreamSource, ThreadId};
use std::path::PathBuf;

fn assert_synthetic_response_pair(events: &[Event], expected_fragment: &str) {
    assert_eq!(
        events.len(),
        2,
        "placeholder op should emit exactly 2 events"
    );

    let started_turn_id = match &events[0] {
        Event::ResponseStarted {
            turn_id,
            stream_source,
        } => {
            assert_eq!(
                stream_source,
                &StreamSource::Synthetic,
                "placeholder started event must use synthetic source"
            );
            *turn_id
        }
        other => panic!("expected ResponseStarted as first event, got: {other:?}"),
    };

    match &events[1] {
        Event::ResponseCompleted {
            turn_id,
            content,
            stream_source,
        } => {
            assert_eq!(
                stream_source,
                &StreamSource::Synthetic,
                "placeholder completed event must use synthetic source"
            );
            assert_eq!(
                *turn_id, started_turn_id,
                "started/completed must share the same turn id"
            );
            assert!(
                content.contains(expected_fragment),
                "unexpected completion content: {content}"
            );
        }
        other => panic!("expected ResponseCompleted as second event, got: {other:?}"),
    }

    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::AssistantMessage { .. })),
        "placeholder flow should no longer depend on AssistantMessage"
    );
}

#[tokio::test]
async fn send_to_agent_emits_response_completed_event() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );

    let events = agent
        .submit(Op::SendToAgent {
            thread_id: ThreadId::new(),
            input: "ping".to_string(),
        })
        .await;

    assert_synthetic_response_pair(&events, "SendToAgent not yet implemented");
}

#[tokio::test]
async fn placeholder_ops_do_not_require_assistant_message_for_completion() {
    let mut agent = Agent::with_runtime(
        ApprovalPolicy::Never,
        SandboxMode::WorkspaceWrite,
        PathBuf::from("."),
    );

    let interrupt_events = agent
        .submit(Op::InterruptAgent {
            thread_id: ThreadId::new(),
        })
        .await;
    assert_synthetic_response_pair(&interrupt_events, "InterruptAgent not yet implemented");

    let resume_events = agent
        .submit(Op::ResumeAgent {
            rollout_path: "rollout.json".to_string(),
            source: AgentSource::Root,
        })
        .await;
    assert_synthetic_response_pair(&resume_events, "ResumeAgent not yet implemented");

    let mut child = agent;
    for _ in 0..tools::MAX_AGENT_DEPTH {
        child = child
            .spawn_sub_agent("child")
            .expect("should spawn until reaching max depth");
    }
    assert_eq!(child.depth(), tools::MAX_AGENT_DEPTH);

    let depth_events = child
        .submit(Op::SpawnAgent {
            input: "deeper".to_string(),
            source: AgentSource::Root,
        })
        .await;
    assert_synthetic_response_pair(&depth_events, "max depth");
}
