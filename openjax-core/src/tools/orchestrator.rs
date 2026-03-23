use crate::approval::{ApprovalRequest, approval_timeout_ms_from_env};
use crate::tools::ToolsConfig;
use crate::tools::context::{ApprovalPolicy, ToolInvocation, ToolOutput};
use crate::tools::events::{AfterToolUse, BeforeToolUse, HookEvent};
use crate::tools::hooks::HookExecutor;
use crate::tools::policy::{
    ApprovalContext, PolicyDecision, PolicyOutcome, evaluate_tool_invocation_policy,
};
use crate::tools::registry::ToolRegistry;
use crate::tools::sandboxing::SandboxManager;
use openjax_policy::runtime::PolicyRuntime;
use openjax_policy::schema::DecisionKind as PolicyCenterDecisionKind;
use openjax_policy::store::PolicyStore;
use openjax_protocol::Event;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

/// 工具编排器
pub struct ToolOrchestrator {
    registry: Arc<ToolRegistry>,
    hook_executor: HookExecutor,
    sandbox_manager: SandboxManager,
    approved_mutating_turns: Mutex<HashSet<u64>>,
    _config: ToolsConfig,
}

impl ToolOrchestrator {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            hook_executor: HookExecutor::new(),
            sandbox_manager: SandboxManager::new(),
            approved_mutating_turns: Mutex::new(HashSet::new()),
            _config: ToolsConfig::default(),
        }
    }

    pub fn with_config(registry: Arc<ToolRegistry>, config: ToolsConfig) -> Self {
        Self {
            registry,
            hook_executor: HookExecutor::new(),
            sandbox_manager: SandboxManager::new(),
            approved_mutating_turns: Mutex::new(HashSet::new()),
            _config: config,
        }
    }

    /// 注册动态工具
    pub fn register_tool(
        &self,
        name: String,
        handler: Arc<dyn crate::tools::registry::ToolHandler>,
    ) {
        self.registry.register(name, handler);
    }

    /// 执行工具调用
    pub async fn run(
        &self,
        invocation: ToolInvocation,
    ) -> Result<ToolOutput, crate::tools::error::FunctionCallError> {
        // 1. 执行前钩子
        self.hook_executor
            .execute(&HookEvent::BeforeToolUse(BeforeToolUse {
                tool_name: invocation.tool_name.clone(),
                call_id: invocation.call_id.clone(),
                tool_input: format!("{:?}", invocation.payload),
            }));

        // 2. 检查是否需要批准（shell 工具在 sandbox façade 内统一处理）
        let is_mutating = self
            .sandbox_manager
            .is_mutating_operation(&invocation.tool_name);
        if !is_shell_like_tool(&invocation.tool_name) {
            let policy_outcome = merge_policy_center_outcome(
                &invocation,
                evaluate_tool_invocation_policy(&invocation, is_mutating),
            );
            if matches!(policy_outcome.trace.decision, PolicyDecision::Deny) {
                return Err(crate::tools::error::FunctionCallError::Internal(
                    policy_outcome.trace.reason,
                ));
            }
            let has_reusable_approval =
                self.should_reuse_mutating_approval(&invocation, is_mutating);
            let requires_approval = matches!(
                policy_outcome.trace.decision,
                PolicyDecision::AskApproval | PolicyDecision::AskEscalation
            ) && !has_reusable_approval;

            if has_reusable_approval {
                tracing::info!(
                    turn_id = invocation.turn.turn_id,
                    tool_name = %invocation.tool_name,
                    "approval reused for mutating tool"
                );
            }

            if requires_approval {
                let request_id = Uuid::new_v4().to_string();
                let context = policy_outcome.approval_context.as_ref();
                let target = approval_target(&invocation, context);
                let reason = approval_reason(&policy_outcome, invocation.turn.approval_policy);
                let risk_tags = if context
                    .map(|ctx| !ctx.risk_tags.is_empty())
                    .unwrap_or(false)
                {
                    context.map(|ctx| ctx.risk_tags.clone()).unwrap_or_default()
                } else {
                    policy_outcome.trace.risk_tags.clone()
                };
                let sandbox_backend = context
                    .and_then(|ctx| ctx.sandbox_backend)
                    .map(|backend| backend.as_str().to_string());
                tracing::info!(
                    turn_id = invocation.turn.turn_id,
                    request_id = %request_id,
                    tool_name = %invocation.tool_name,
                    target_preview = %truncate_preview(&target, 160),
                    policy_decision = ?policy_outcome.trace.decision,
                    risk_tags = ?risk_tags,
                    sandbox_backend = ?sandbox_backend,
                    "approval_request_logged"
                );
                if let Some(sink) = &invocation.turn.event_sink {
                    let _ = sink.send(Event::ApprovalRequested {
                        turn_id: invocation.turn.turn_id,
                        request_id: request_id.clone(),
                        target: target.clone(),
                        reason: reason.clone(),
                        tool_name: Some(invocation.tool_name.clone()),
                        command_preview: context.and_then(|ctx| ctx.command_preview.clone()),
                        risk_tags: risk_tags.clone(),
                        sandbox_backend: sandbox_backend.clone(),
                        degrade_reason: context.and_then(|ctx| ctx.degrade_reason.clone()),
                    });
                }

                let timeout_ms = approval_timeout_ms_from_env();
                let approval_start = Instant::now();
                let request = ApprovalRequest {
                    request_id: request_id.clone(),
                    target,
                    reason,
                };
                let approved = match timeout(
                    Duration::from_millis(timeout_ms),
                    invocation.turn.approval_handler.request_approval(request),
                )
                .await
                {
                    Ok(result) => {
                        result.map_err(crate::tools::error::FunctionCallError::Internal)?
                    }
                    Err(_) => {
                        if let Some(sink) = &invocation.turn.event_sink {
                            let _ = sink.send(Event::ApprovalResolved {
                                turn_id: invocation.turn.turn_id,
                                request_id: request_id.clone(),
                                approved: false,
                            });
                        }
                        tracing::info!(
                            turn_id = invocation.turn.turn_id,
                            request_id = %request_id,
                            tool_name = %invocation.tool_name,
                            approved = false,
                            timed_out = true,
                            latency_ms = approval_start.elapsed().as_millis() as u64,
                            "approval_result_logged"
                        );
                        return Err(crate::tools::error::FunctionCallError::ApprovalTimedOut(
                            format!(
                                "approval request timed out after {}ms ({request_id})",
                                timeout_ms
                            ),
                        ));
                    }
                };

                if let Some(sink) = &invocation.turn.event_sink {
                    let _ = sink.send(Event::ApprovalResolved {
                        turn_id: invocation.turn.turn_id,
                        request_id: request_id.clone(),
                        approved,
                    });
                }
                tracing::info!(
                    turn_id = invocation.turn.turn_id,
                    request_id = %request_id,
                    tool_name = %invocation.tool_name,
                    approved = approved,
                    timed_out = false,
                    latency_ms = approval_start.elapsed().as_millis() as u64,
                    "approval_result_logged"
                );

                if !approved {
                    return Err(crate::tools::error::FunctionCallError::ApprovalRejected(
                        "command rejected by user".to_string(),
                    ));
                }

                self.record_mutating_approval(&invocation, is_mutating);
            }
        }

        // 3. 选择合适的沙箱
        let sandbox = self
            .sandbox_manager
            .select_sandbox(invocation.turn.sandbox_policy);

        // 4. 执行工具
        let start = Instant::now();
        let result = self.registry.dispatch(invocation.clone()).await;
        let duration = start.elapsed();

        // 5. 执行后钩子
        let is_success = matches!(
            result.as_ref(),
            Ok(ToolOutput::Function {
                success: Some(true),
                ..
            }) | Ok(ToolOutput::Function { success: None, .. })
                | Ok(ToolOutput::Mcp { result: Ok(_), .. })
        );
        let output_preview = result.as_ref().ok().map(|o| format!("{:?}", o));

        self.hook_executor
            .execute(&HookEvent::AfterToolUse(AfterToolUse {
                tool_name: invocation.tool_name.clone(),
                call_id: invocation.call_id.clone(),
                tool_input: format!("{:?}", invocation.payload),
                executed: is_success,
                success: is_success,
                duration_ms: duration.as_millis() as u64,
                mutating: is_mutating,
                sandbox: sandbox.as_str().to_string(),
                sandbox_policy: invocation.turn.sandbox_policy.as_str().to_string(),
                output_preview,
            }));

        result
    }

    fn should_reuse_mutating_approval(
        &self,
        invocation: &ToolInvocation,
        is_mutating: bool,
    ) -> bool {
        if !is_mutating || is_shell_like_tool(&invocation.tool_name) {
            return false;
        }
        let Ok(approved_turns) = self.approved_mutating_turns.lock() else {
            return false;
        };
        approved_turns.contains(&invocation.turn.turn_id)
    }

    fn record_mutating_approval(&self, invocation: &ToolInvocation, is_mutating: bool) {
        if !is_mutating || is_shell_like_tool(&invocation.tool_name) {
            return;
        }
        let Ok(mut approved_turns) = self.approved_mutating_turns.lock() else {
            return;
        };
        approved_turns.insert(invocation.turn.turn_id);
        if approved_turns.len() > 256 {
            approved_turns.clear();
        }
    }
}

fn is_shell_like_tool(tool_name: &str) -> bool {
    matches!(tool_name, "shell" | "exec_command")
}

fn approval_target(invocation: &ToolInvocation, context: Option<&ApprovalContext>) -> String {
    if let Some(ctx) = context
        && let Some(command) = &ctx.raw_command
    {
        return command.clone();
    }
    invocation.tool_name.clone()
}

fn approval_reason(
    outcome: &crate::tools::policy::PolicyOutcome,
    approval_policy: ApprovalPolicy,
) -> String {
    if let Some(context) = &outcome.approval_context {
        return context.reason.clone();
    }
    if matches!(approval_policy, ApprovalPolicy::AlwaysAsk) {
        return "approval policy requires confirmation".to_string();
    }
    outcome.trace.reason.clone()
}

fn truncate_preview(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push_str("...");
    out
}

fn merge_policy_center_outcome(
    invocation: &ToolInvocation,
    mut legacy: PolicyOutcome,
) -> PolicyOutcome {
    let center = evaluate_policy_center_decision(invocation);
    let center_decision = map_policy_center_decision(&center.kind);
    if decision_rank(center_decision) > decision_rank(legacy.trace.decision) {
        legacy.trace.decision = center_decision;
        legacy.trace.reason = center.reason.clone();

        if matches!(
            legacy.trace.decision,
            PolicyDecision::AskApproval | PolicyDecision::AskEscalation
        ) && legacy.approval_context.is_none()
        {
            legacy.approval_context = Some(ApprovalContext {
                tool_name: invocation.tool_name.clone(),
                raw_command: None,
                normalized_command: None,
                command_preview: None,
                risk_tags: center
                    .matched_rule_id
                    .as_ref()
                    .map(|_| Vec::new())
                    .unwrap_or_else(|| vec!["unknown_tool_descriptor".to_string()]),
                reason: center.reason,
                sandbox_backend: None,
                degrade_reason: None,
                fallback_plan: None,
            });
        }
    }
    legacy
}

fn evaluate_policy_center_decision(invocation: &ToolInvocation) -> openjax_policy::PolicyDecision {
    let descriptor = invocation.policy_descriptor();
    let rules = descriptor
        .as_ref()
        .map(|item| vec![item.allow_rule_for_tool(&invocation.tool_name)])
        .unwrap_or_default();
    let runtime = PolicyRuntime::new(PolicyStore::new(PolicyCenterDecisionKind::Ask, rules));
    let input = invocation.to_policy_center_input(descriptor.as_ref(), runtime.current_version());
    runtime.handle().decide(&input)
}

fn map_policy_center_decision(decision: &PolicyCenterDecisionKind) -> PolicyDecision {
    match decision {
        PolicyCenterDecisionKind::Allow => PolicyDecision::Allow,
        PolicyCenterDecisionKind::Ask => PolicyDecision::AskApproval,
        PolicyCenterDecisionKind::Escalate => PolicyDecision::AskEscalation,
        PolicyCenterDecisionKind::Deny => PolicyDecision::Deny,
    }
}

fn decision_rank(decision: PolicyDecision) -> u8 {
    match decision {
        PolicyDecision::Allow => 0,
        PolicyDecision::AskApproval => 1,
        PolicyDecision::AskEscalation => 2,
        PolicyDecision::Deny => 3,
    }
}
