use std::sync::Arc;

use async_trait::async_trait;
use openjax_core::approval::{ApprovalHandler, ApprovalRequest};
use openjax_core::tools::context::{SandboxPolicy, ToolInvocation, ToolPayload, ToolTurnContext};
use openjax_core::tools::error::FunctionCallError;
use openjax_core::tools::orchestrator::ToolOrchestrator;
use openjax_core::tools::registry::ToolRegistry;
use openjax_core::tools::shell::ShellType;
use openjax_policy::overlay::SessionOverlay;
use openjax_policy::runtime::PolicyRuntime;
use openjax_policy::schema::{DecisionKind, PolicyRule};
use openjax_policy::store::PolicyStore;
use openjax_protocol::ApprovalKind;
use openjax_protocol::Event;

#[derive(Debug)]
struct AcceptApproval;

#[async_trait]
impl ApprovalHandler for AcceptApproval {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(true)
    }
}

#[derive(Debug)]
struct RejectApproval;

#[async_trait]
impl ApprovalHandler for RejectApproval {
    async fn request_approval(&self, _request: ApprovalRequest) -> Result<bool, String> {
        Ok(false)
    }
}

#[tokio::test]
async fn unknown_tool_without_descriptor_defaults_to_ask() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let invocation = ToolInvocation {
        tool_name: "unknown_tool".to_string(),
        call_id: "call-unknown-1".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 42,
            session_id: None,
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(RejectApproval),
            event_sink: None,
            policy_runtime: None,
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let result = orchestrator.run(invocation).await;
    match result {
        Err(FunctionCallError::ApprovalRejected(_)) => {}
        other => panic!("expected ApprovalRejected for unknown descriptor, got: {other:?}"),
    }
}

#[tokio::test]
async fn published_runtime_rule_can_deny_known_tool() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let runtime = PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![PolicyRule {
            id: "deny_Read".to_string(),
            decision: DecisionKind::Deny,
            priority: 200,
            tool_name: Some("Read".to_string()),
            action: Some("read".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec![],
            reason: "deny read file".to_string(),
        }],
    ));
    let invocation = ToolInvocation {
        tool_name: "Read".to_string(),
        call_id: "call-read-1".to_string(),
        payload: ToolPayload::Function {
            arguments: r#"{"path":"Cargo.toml"}"#.to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 7,
            session_id: Some("sess_policy".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(RejectApproval),
            event_sink: None,
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let result = orchestrator.run(invocation).await;
    match result {
        Err(FunctionCallError::Internal(message)) => {
            assert!(message.contains("deny read file"));
        }
        other => panic!("expected deny result from published runtime, got: {other:?}"),
    }
}

#[tokio::test]
async fn session_overlay_rule_applies_by_session_id() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let runtime = PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![PolicyRule {
            id: "allow_Read".to_string(),
            decision: DecisionKind::Allow,
            priority: 100,
            tool_name: Some("Read".to_string()),
            action: Some("read".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec![],
            reason: "allow read file".to_string(),
        }],
    ));
    runtime.set_session_overlay(
        "sess_overlay",
        SessionOverlay::new(vec![PolicyRule {
            id: "overlay_deny_Read".to_string(),
            decision: DecisionKind::Deny,
            priority: 300,
            tool_name: Some("Read".to_string()),
            action: Some("read".to_string()),
            session_id: Some("sess_overlay".to_string()),
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec![],
            reason: "overlay deny read file".to_string(),
        }]),
    );
    let invocation = ToolInvocation {
        tool_name: "Read".to_string(),
        call_id: "call-read-overlay".to_string(),
        payload: ToolPayload::Function {
            arguments: r#"{"path":"Cargo.toml"}"#.to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 8,
            session_id: Some("sess_overlay".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(RejectApproval),
            event_sink: None,
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let result = orchestrator.run(invocation).await;
    match result {
        Err(FunctionCallError::Internal(message)) => {
            assert!(message.contains("overlay deny read file"));
        }
        other => panic!("expected overlay deny for matched session, got: {other:?}"),
    }
}

/// Shell 工具也应经过 Policy Center 决策路径，
/// ApprovalRequested 事件的 approval_kind 应为 Some(Normal) 而非 None。
#[tokio::test]
async fn shell_tool_goes_through_policy_center() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-shell-policy".to_string(),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": "echo hello" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 100,
            session_id: Some("sess_shell".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(AcceptApproval),
            event_sink: Some(tx),
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    // 执行 shell 工具（无论执行结果如何，我们只关心审批事件）
    let _ = orchestrator.run(invocation).await;

    // 收集所有已发送的事件
    let mut approval_kind_found = None;
    while let Ok(event) = rx.try_recv() {
        if let Event::ApprovalRequested { approval_kind, .. } = event {
            approval_kind_found = Some(approval_kind);
            break;
        }
    }

    let approval_kind =
        approval_kind_found.expect("shell tool should emit ApprovalRequested event");
    assert_eq!(
        approval_kind,
        Some(ApprovalKind::Normal),
        "shell tool approval_kind should be Some(Normal), not None"
    );
}

/// 沙箱降级触发的审批事件 approval_kind 应为 Some(Escalation)（无 policy runtime 时）。
#[tokio::test]
async fn degrade_approval_has_escalation_kind() {
    use openjax_core::sandbox::degrade::request_degrade_approval;
    use tokio::sync::mpsc::unbounded_channel;

    let (tx, mut rx) = unbounded_channel::<Event>();

    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-degrade-1".to_string(),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": "echo test" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 200,
            session_id: Some("sess_degrade".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            // AcceptApproval：允许审批通过，以便函数能正常返回
            approval_handler: Arc::new(AcceptApproval),
            event_sink: Some(tx),
            // 无 policy_runtime：应回退为 Escalate，对应 ApprovalKind::Escalation
            policy_runtime: None,
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let result = request_degrade_approval(
        &invocation,
        "echo test",
        "none_escalated",
        "seatbelt unavailable",
    )
    .await;
    assert!(
        result.is_ok(),
        "degrade approval should succeed when handler accepts: {result:?}"
    );

    // 检查发出的 ApprovalRequested 事件
    let mut found_kind = None;
    while let Ok(event) = rx.try_recv() {
        if let Event::ApprovalRequested { approval_kind, .. } = event {
            found_kind = Some(approval_kind);
            break;
        }
    }

    let approval_kind = found_kind.expect("degrade path should emit ApprovalRequested event");
    assert_eq!(
        approval_kind,
        Some(ApprovalKind::Escalation),
        "degrade approval_kind should be Some(Escalation) when no policy runtime, got: {approval_kind:?}"
    );
}

/// 当 Policy Center 有 Escalate 规则时，降级审批应发出 Escalation 类型。
#[tokio::test]
async fn degrade_approval_escalation_from_policy_center() {
    use openjax_core::sandbox::degrade::request_degrade_approval;
    use tokio::sync::mpsc::unbounded_channel;

    let (tx, mut rx) = unbounded_channel::<Event>();

    // 构造一个强制 Escalate 的 policy runtime
    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Escalate, vec![]));

    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-degrade-2".to_string(),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": "echo test" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 201,
            session_id: Some("sess_degrade_policy".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(AcceptApproval),
            event_sink: Some(tx),
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let result = request_degrade_approval(
        &invocation,
        "echo test",
        "none_escalated",
        "backend unavailable",
    )
    .await;
    assert!(
        result.is_ok(),
        "degrade approval should succeed: {result:?}"
    );

    let mut found_kind = None;
    while let Ok(event) = rx.try_recv() {
        if let Event::ApprovalRequested { approval_kind, .. } = event {
            found_kind = Some(approval_kind);
            break;
        }
    }

    let approval_kind = found_kind.expect("degrade path should emit ApprovalRequested event");
    assert_eq!(
        approval_kind,
        Some(ApprovalKind::Escalation),
        "degrade with Escalate policy should have ApprovalKind::Escalation, got: {approval_kind:?}"
    );
}

/// 当 Policy Center 明确 Deny 时，降级路径应直接返回错误，不触发审批事件。
#[tokio::test]
async fn degrade_approval_denied_by_policy_center() {
    use openjax_core::sandbox::degrade::request_degrade_approval;
    use openjax_core::tools::error::FunctionCallError;
    use tokio::sync::mpsc::unbounded_channel;

    let (tx, mut rx) = unbounded_channel::<Event>();

    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Deny, vec![]));

    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-degrade-deny".to_string(),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": "echo test" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 202,
            session_id: Some("sess_degrade_deny".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(AcceptApproval),
            event_sink: Some(tx),
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let result = request_degrade_approval(
        &invocation,
        "echo test",
        "none_escalated",
        "backend unavailable",
    )
    .await;
    assert!(
        matches!(result, Err(FunctionCallError::Internal(_))),
        "policy Deny should produce Internal error, got: {result:?}"
    );

    // 确认没有 ApprovalRequested 事件被发出
    let has_approval_event = rx
        .try_recv()
        .map(|e| matches!(e, Event::ApprovalRequested { .. }))
        .unwrap_or(false);
    assert!(
        !has_approval_event,
        "policy Deny should NOT emit ApprovalRequested event"
    );
}

/// 带 destructive 风险标签的 shell 命令（如 rm -rf /tmp/test_dir）经 Policy Center 后，
/// sandbox 层将 destructive 命令映射为 PolicyDecision::AskEscalation，
/// 最终触发 Escalation 类型审批事件，用户可以选择批准或拒绝。
#[tokio::test]
async fn destructive_shell_triggers_escalation() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    // DecisionKind::Ask 为默认，系统内置 destructive_escalate 规则自动注入
    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-shell-destructive".to_string(),
        payload: ToolPayload::Function {
            // rm -rf /tmp/test_dir 包含 "rm -rf /" 模式，触发 destructive 标签
            arguments: serde_json::json!({ "cmd": "rm -rf /tmp/test_dir" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 300,
            session_id: Some("sess_shell_destructive".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(AcceptApproval),
            event_sink: Some(tx),
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let _ = orchestrator.run(invocation).await;

    // 确认发出了 ApprovalRequested 事件且 approval_kind 为 Escalation
    let mut approval_kind_found = None;
    while let Ok(event) = rx.try_recv() {
        if let Event::ApprovalRequested { approval_kind, .. } = event {
            approval_kind_found = Some(approval_kind);
            break;
        }
    }

    let approval_kind =
        approval_kind_found.expect("destructive shell command should emit ApprovalRequested event");
    assert_eq!(
        approval_kind,
        Some(ApprovalKind::Escalation),
        "destructive shell command approval_kind should be Some(Escalation), got: {approval_kind:?}"
    );
}

/// shell 工具配置了明确 Allow 规则时，不应触发 ApprovalRequested 事件。
#[tokio::test]
async fn shell_allow_rule_skips_approval() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let runtime = PolicyRuntime::new(PolicyStore::new(
        DecisionKind::Ask,
        vec![PolicyRule {
            id: "allow_shell_exec".to_string(),
            decision: DecisionKind::Allow,
            priority: 10,
            tool_name: Some("shell".to_string()),
            action: Some("exec".to_string()),
            session_id: None,
            actor: None,
            resource: None,
            capabilities_all: vec![],
            risk_tags_all: vec![],
            reason: "allow shell exec".to_string(),
        }],
    ));
    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-shell-allow".to_string(),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": "echo hello" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 400,
            session_id: Some("sess_shell_allow".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(RejectApproval),
            event_sink: Some(tx),
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let _ = orchestrator.run(invocation).await;

    // Allow 规则应跳过审批，不应有任何 ApprovalRequested 事件
    let has_approval_event = rx
        .try_recv()
        .map(|e| matches!(e, Event::ApprovalRequested { .. }))
        .unwrap_or(false);
    assert!(
        !has_approval_event,
        "shell with Allow rule should NOT emit ApprovalRequested event"
    );
}

/// destructive 命令应通过内置 system:destructive_escalate 规则触发 Escalation 审批，
/// 无需任何自定义规则。
#[tokio::test]
async fn destructive_command_triggers_escalation_via_builtin_rule() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    // 无自定义规则，只有内置 system:destructive_escalate 规则
    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-shell-builtin-destructive".to_string(),
        payload: ToolPayload::Function {
            // rm -rf / 触发内置 destructive 风险标签 → system:destructive_escalate 规则
            arguments: serde_json::json!({ "cmd": "rm -rf /" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 401,
            session_id: Some("sess_builtin_destructive".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            // AcceptApproval 防止 rejection 掩盖事件发出
            approval_handler: Arc::new(AcceptApproval),
            event_sink: Some(tx),
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let _ = orchestrator.run(invocation).await;

    let mut approval_kind_found = None;
    while let Ok(event) = rx.try_recv() {
        if let Event::ApprovalRequested { approval_kind, .. } = event {
            approval_kind_found = Some(approval_kind);
            break;
        }
    }

    let approval_kind = approval_kind_found
        .expect("destructive shell via builtin rule should emit ApprovalRequested event");
    assert_eq!(
        approval_kind,
        Some(ApprovalKind::Escalation),
        "builtin destructive_escalate rule should produce ApprovalKind::Escalation, got: {approval_kind:?}"
    );
}

/// 普通 shell 命令（如 ls -la）经 Policy Center 后，
/// 默认 Ask 决策映射为 Normal 审批，approval_kind 应为 Some(Normal)。
#[tokio::test]
async fn safe_shell_triggers_normal_approval() {
    let orchestrator = ToolOrchestrator::new(Arc::new(ToolRegistry::new()));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
    let invocation = ToolInvocation {
        tool_name: "shell".to_string(),
        call_id: "call-shell-safe".to_string(),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": "ls -la" }).to_string(),
        },
        turn: ToolTurnContext {
            turn_id: 301,
            session_id: Some("sess_shell_safe".to_string()),
            cwd: std::path::PathBuf::from("."),
            sandbox_policy: SandboxPolicy::Write,
            shell_type: ShellType::default(),
            approval_handler: Arc::new(AcceptApproval),
            event_sink: Some(tx),
            policy_runtime: Some(runtime),
            windows_sandbox_level: None,
            prevent_shell_skill_trigger: true,
        },
    };

    let _ = orchestrator.run(invocation).await;

    let mut approval_kind_found = None;
    while let Ok(event) = rx.try_recv() {
        if let Event::ApprovalRequested { approval_kind, .. } = event {
            approval_kind_found = Some(approval_kind);
            break;
        }
    }

    let approval_kind =
        approval_kind_found.expect("safe shell tool should emit ApprovalRequested event");
    assert_eq!(
        approval_kind,
        Some(ApprovalKind::Normal),
        "safe shell command approval_kind should be Some(Normal), got: {approval_kind:?}"
    );
}
