use crate::approval::{ApprovalRequest, approval_timeout_ms_from_env};
use crate::tools::ToolsConfig;
use crate::tools::context::{ToolInvocation, ToolOutput};
use crate::tools::events::{AfterToolUse, BeforeToolUse, HookEvent};
use crate::tools::hooks::HookExecutor;
use crate::tools::policy::{
    ApprovalContext, PolicyDecision, PolicyOutcome, evaluate_tool_invocation_policy,
    extract_shell_command, extract_shell_risk_tags,
};
use crate::tools::registry::ToolRegistry;
use crate::tools::sandboxing::SandboxManager;
use openjax_policy::runtime::PolicyRuntime;
use openjax_policy::schema::DecisionKind as PolicyCenterDecisionKind;
use openjax_policy::store::PolicyStore;
use openjax_protocol::{ApprovalKind, Event};
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

        // 2. 统一决策路径：所有工具（包括 shell）都经过 Policy Center
        let is_mutating = self
            .sandbox_manager
            .is_mutating_operation(&invocation.tool_name);

        let policy_center_decision = evaluate_policy_center_decision(&invocation);
        let legacy_outcome = evaluate_tool_invocation_policy(&invocation, is_mutating);
        let policy_outcome =
            select_stricter_outcome(&invocation, &policy_center_decision, legacy_outcome);

        if matches!(policy_outcome.trace.decision, PolicyDecision::Deny) {
            return Err(crate::tools::error::FunctionCallError::Internal(
                policy_outcome.trace.reason,
            ));
        }

        let has_reusable_approval = self.should_reuse_mutating_approval(&invocation, is_mutating);
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
            let reason = approval_reason(&policy_outcome);
            let policy_version = Some(policy_center_decision.policy_version);
            let matched_rule_id = policy_center_decision.matched_rule_id.clone();
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
            let approval_kind = match policy_outcome.trace.decision {
                PolicyDecision::AskApproval => Some(ApprovalKind::Normal),
                PolicyDecision::AskEscalation => Some(ApprovalKind::Escalation),
                _ => None,
            };
            tracing::info!(
                turn_id = invocation.turn.turn_id,
                request_id = %request_id,
                tool_name = %invocation.tool_name,
                target_preview = %truncate_preview(&target, 160),
                policy_decision = ?policy_outcome.trace.decision,
                risk_tags = ?risk_tags,
                sandbox_backend = ?sandbox_backend,
                approval_kind = ?approval_kind,
                "approval_request_logged"
            );
            if let Some(sink) = &invocation.turn.event_sink {
                let _ = sink.send(Event::ApprovalRequested {
                    turn_id: invocation.turn.turn_id,
                    request_id: request_id.clone(),
                    target: target.clone(),
                    reason: reason.clone(),
                    policy_version,
                    matched_rule_id,
                    tool_name: Some(invocation.tool_name.clone()),
                    command_preview: context.and_then(|ctx| ctx.command_preview.clone()),
                    risk_tags: risk_tags.clone(),
                    sandbox_backend: sandbox_backend.clone(),
                    degrade_reason: context.and_then(|ctx| ctx.degrade_reason.clone()),
                    approval_kind,
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
                Ok(result) => result.map_err(crate::tools::error::FunctionCallError::Internal)?,
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

fn approval_reason(outcome: &crate::tools::policy::PolicyOutcome) -> String {
    if let Some(context) = &outcome.approval_context {
        return context.reason.clone();
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

/// 选择更严格的决策结果：对比 Policy Center 决策与 legacy 决策，取更严格的一方。
fn select_stricter_outcome(
    invocation: &ToolInvocation,
    center: &openjax_policy::PolicyDecision,
    mut legacy: PolicyOutcome,
) -> PolicyOutcome {
    let center_decision = match center.kind {
        PolicyCenterDecisionKind::Allow => PolicyDecision::Allow,
        PolicyCenterDecisionKind::Ask => PolicyDecision::AskApproval,
        PolicyCenterDecisionKind::Escalate => PolicyDecision::AskEscalation,
        PolicyCenterDecisionKind::Deny => PolicyDecision::Deny,
    };

    let rank = |d: PolicyDecision| -> u8 {
        match d {
            PolicyDecision::Allow => 0,
            PolicyDecision::AskApproval => 1,
            PolicyDecision::AskEscalation => 2,
            PolicyDecision::Deny => 3,
        }
    };

    if rank(center_decision) > rank(legacy.trace.decision) {
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
                reason: center.reason.clone(),
                sandbox_backend: None,
                degrade_reason: None,
                fallback_plan: None,
            });
        }
    }
    legacy
}

fn evaluate_policy_center_decision(invocation: &ToolInvocation) -> openjax_policy::PolicyDecision {
    let mut descriptor = invocation.policy_descriptor();

    // 为 shell 工具注入命令级风险标签到 descriptor 中，
    // 使 Policy Center 能基于具体命令内容进行决策
    if is_shell_like_tool(&invocation.tool_name) {
        if let Some((command, require_escalated)) = extract_shell_command(invocation) {
            let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
            let shell_risk_tags = extract_shell_risk_tags(&normalized, require_escalated);
            if let Some(ref mut desc) = descriptor {
                for tag in shell_risk_tags {
                    if !desc.risk_tags.contains(&tag) {
                        desc.risk_tags.push(tag);
                    }
                }
            }
        }
    }

    if let Some(runtime) = invocation.turn.policy_runtime.as_ref() {
        let handle = runtime.handle();
        let input = invocation.to_policy_center_input(descriptor.as_ref(), handle.policy_version());
        return handle.decide(&input);
    }

    // 无 policy_runtime 时的回退：
    // - 已知工具（有 descriptor）创建 Allow 规则，保持与 OnRequest 策略等效的默认行为
    // - 未知工具（无 descriptor）使用 Ask 默认，要求审批
    // 注：需要强制审批的测试场景应显式注入 PolicyRuntime(Ask)
    let rules = descriptor
        .as_ref()
        .map(|item| vec![item.allow_rule_for_tool(&invocation.tool_name)])
        .unwrap_or_default();
    let runtime = PolicyRuntime::new(PolicyStore::new(PolicyCenterDecisionKind::Ask, rules));
    let input = invocation.to_policy_center_input(descriptor.as_ref(), runtime.current_version());
    runtime.handle().decide(&input)
}
