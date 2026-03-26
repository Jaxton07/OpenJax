use std::sync::{Arc, Mutex, MutexGuard};

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, State};
use openjax_policy::overlay::SessionOverlay;
use openjax_policy::schema::{DecisionKind, PolicyRule};
use openjax_policy::store::PolicyStore;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{ApiError, now_rfc3339};
use crate::middleware::RequestContext;
use crate::state::{AppState, GatewayPolicyState, gateway_policy_state};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ApiDecisionKind {
    Allow,
    Ask,
    Escalate,
    Deny,
}

impl ApiDecisionKind {
    fn into_domain(self) -> DecisionKind {
        match self {
            Self::Allow => DecisionKind::Allow,
            Self::Ask => DecisionKind::Ask,
            Self::Escalate => DecisionKind::Escalate,
            Self::Deny => DecisionKind::Deny,
        }
    }
}

impl From<DecisionKind> for ApiDecisionKind {
    fn from(value: DecisionKind) -> Self {
        match value {
            DecisionKind::Allow => Self::Allow,
            DecisionKind::Ask => Self::Ask,
            DecisionKind::Escalate => Self::Escalate,
            DecisionKind::Deny => Self::Deny,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPolicyRule {
    id: String,
    decision: ApiDecisionKind,
    priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource: Option<String>,
    #[serde(default)]
    capabilities_all: Vec<String>,
    #[serde(default)]
    risk_tags_all: Vec<String>,
    reason: String,
}

impl ApiPolicyRule {
    fn from_domain(rule: PolicyRule) -> Self {
        Self {
            id: rule.id,
            decision: ApiDecisionKind::from(rule.decision),
            priority: rule.priority,
            tool_name: rule.tool_name,
            action: rule.action,
            session_id: rule.session_id,
            actor: rule.actor,
            resource: rule.resource,
            capabilities_all: rule.capabilities_all,
            risk_tags_all: rule.risk_tags_all,
            reason: rule.reason,
        }
    }

    fn into_domain_with_id(self, id: String) -> Result<PolicyRule, ApiError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(ApiError::invalid_argument(
                "rule id must not be empty",
                json!({}),
            ));
        }
        let reason = self.reason.trim();
        if reason.is_empty() {
            return Err(ApiError::invalid_argument(
                "rule reason must not be empty",
                json!({}),
            ));
        }
        Ok(PolicyRule {
            id: id.to_string(),
            decision: self.decision.into_domain(),
            priority: self.priority,
            tool_name: normalize_optional(self.tool_name),
            action: normalize_optional(self.action),
            session_id: normalize_optional(self.session_id),
            actor: normalize_optional(self.actor),
            resource: normalize_optional(self.resource),
            capabilities_all: normalize_vec(self.capabilities_all),
            risk_tags_all: normalize_vec(self.risk_tags_all),
            reason: reason.to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePolicyRuleRequest {
    id: String,
    decision: ApiDecisionKind,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    actor: Option<String>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    capabilities_all: Vec<String>,
    #[serde(default)]
    risk_tags_all: Vec<String>,
    reason: String,
}

impl CreatePolicyRuleRequest {
    fn into_domain(self) -> Result<PolicyRule, ApiError> {
        let id = self.id.trim().to_string();
        ApiPolicyRule {
            id: id.clone(),
            decision: self.decision,
            priority: self.priority,
            tool_name: self.tool_name,
            action: self.action,
            session_id: self.session_id,
            actor: self.actor,
            resource: self.resource,
            capabilities_all: self.capabilities_all,
            risk_tags_all: self.risk_tags_all,
            reason: self.reason,
        }
        .into_domain_with_id(id)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePolicyRuleRequest {
    decision: ApiDecisionKind,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    actor: Option<String>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    capabilities_all: Vec<String>,
    #[serde(default)]
    risk_tags_all: Vec<String>,
    reason: String,
}

impl UpdatePolicyRuleRequest {
    fn into_domain(self, rule_id: String) -> Result<PolicyRule, ApiError> {
        ApiPolicyRule {
            id: rule_id.clone(),
            decision: self.decision,
            priority: self.priority,
            tool_name: self.tool_name,
            action: self.action,
            session_id: self.session_id,
            actor: self.actor,
            resource: self.resource,
            capabilities_all: self.capabilities_all,
            risk_tags_all: self.risk_tags_all,
            reason: self.reason,
        }
        .into_domain_with_id(rule_id)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetSessionOverlayRequest {
    #[serde(default)]
    rules: Vec<CreatePolicyRuleRequest>,
}

#[derive(Debug, Serialize)]
pub struct PolicyRulesResponse {
    request_id: String,
    policy_version: u64,
    rules: Vec<ApiPolicyRule>,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyRuleMutationResponse {
    request_id: String,
    policy_version: u64,
    rule: ApiPolicyRule,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyRuleDeleteResponse {
    request_id: String,
    policy_version: u64,
    rule_id: String,
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyPublishResponse {
    request_id: String,
    policy_version: u64,
    rule_count: usize,
    timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyOverlayResponse {
    request_id: String,
    session_id: String,
    policy_version: u64,
    rule_count: usize,
    status: &'static str,
    timestamp: String,
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let normalized = text.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        }
    })
}

fn normalize_vec(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn lock_policy_state<'a>(
    policy_state: &'a Arc<Mutex<GatewayPolicyState>>,
) -> MutexGuard<'a, GatewayPolicyState> {
    match policy_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn policy_state_for(state: &AppState) -> Arc<Mutex<GatewayPolicyState>> {
    gateway_policy_state(&state.store)
}

fn parse_json_body<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    match payload {
        Ok(Json(value)) => Ok(value),
        Err(rejection) => Err(ApiError::invalid_argument(
            "invalid request body",
            json!({ "reason": rejection.body_text() }),
        )),
    }
}

pub async fn list_policy_rules(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<PolicyRulesResponse>, ApiError> {
    let policy_state = policy_state_for(&state);
    let guard = lock_policy_state(&policy_state);
    let rules = guard
        .draft_rules
        .iter()
        .cloned()
        .map(ApiPolicyRule::from_domain)
        .collect::<Vec<ApiPolicyRule>>();
    Ok(Json(PolicyRulesResponse {
        request_id: ctx.request_id,
        policy_version: guard.runtime.current_version(),
        rules,
        timestamp: now_rfc3339(),
    }))
}

pub async fn create_policy_rule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    payload: Result<Json<CreatePolicyRuleRequest>, JsonRejection>,
) -> Result<Json<PolicyRuleMutationResponse>, ApiError> {
    let payload = parse_json_body(payload)?;
    let rule = payload.into_domain()?;
    let policy_state = policy_state_for(&state);
    let mut guard = lock_policy_state(&policy_state);
    if guard.draft_rules.iter().any(|item| item.id == rule.id) {
        return Err(ApiError::conflict(
            "rule already exists",
            json!({ "rule_id": rule.id }),
        ));
    }
    guard.draft_rules.push(rule.clone());
    Ok(Json(PolicyRuleMutationResponse {
        request_id: ctx.request_id,
        policy_version: guard.runtime.current_version(),
        rule: ApiPolicyRule::from_domain(rule),
        timestamp: now_rfc3339(),
    }))
}

pub async fn update_policy_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    payload: Result<Json<UpdatePolicyRuleRequest>, JsonRejection>,
) -> Result<Json<PolicyRuleMutationResponse>, ApiError> {
    let payload = parse_json_body(payload)?;
    let trimmed_rule_id = rule_id.trim().to_string();
    if trimmed_rule_id.is_empty() {
        return Err(ApiError::invalid_argument(
            "rule id must not be empty",
            json!({}),
        ));
    }
    let updated_rule = payload.into_domain(trimmed_rule_id.clone())?;
    let policy_state = policy_state_for(&state);
    let mut guard = lock_policy_state(&policy_state);
    let Some(index) = guard
        .draft_rules
        .iter()
        .position(|item| item.id == trimmed_rule_id)
    else {
        return Err(ApiError::not_found(
            "rule not found",
            json!({ "rule_id": trimmed_rule_id }),
        ));
    };
    guard.draft_rules[index] = updated_rule.clone();
    Ok(Json(PolicyRuleMutationResponse {
        request_id: ctx.request_id,
        policy_version: guard.runtime.current_version(),
        rule: ApiPolicyRule::from_domain(updated_rule),
        timestamp: now_rfc3339(),
    }))
}

pub async fn delete_policy_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<PolicyRuleDeleteResponse>, ApiError> {
    let trimmed_rule_id = rule_id.trim().to_string();
    if trimmed_rule_id.is_empty() {
        return Err(ApiError::invalid_argument(
            "rule id must not be empty",
            json!({}),
        ));
    }
    let policy_state = policy_state_for(&state);
    let mut guard = lock_policy_state(&policy_state);
    let Some(index) = guard
        .draft_rules
        .iter()
        .position(|item| item.id == trimmed_rule_id)
    else {
        return Err(ApiError::not_found(
            "rule not found",
            json!({ "rule_id": trimmed_rule_id }),
        ));
    };
    guard.draft_rules.remove(index);
    Ok(Json(PolicyRuleDeleteResponse {
        request_id: ctx.request_id,
        policy_version: guard.runtime.current_version(),
        rule_id: trimmed_rule_id,
        status: "deleted",
        timestamp: now_rfc3339(),
    }))
}

pub async fn publish_policy(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<PolicyPublishResponse>, ApiError> {
    let policy_state = policy_state_for(&state);
    let guard = lock_policy_state(&policy_state);
    let store = PolicyStore::new(guard.default_decision.clone(), guard.draft_rules.clone());
    let policy_version = guard.runtime.publish(store);
    let rule_count = guard.draft_rules.len();
    Ok(Json(PolicyPublishResponse {
        request_id: ctx.request_id,
        policy_version,
        rule_count,
        timestamp: now_rfc3339(),
    }))
}

pub async fn set_session_policy_overlay(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
    payload: Result<Json<SetSessionOverlayRequest>, JsonRejection>,
) -> Result<Json<PolicyOverlayResponse>, ApiError> {
    let payload = parse_json_body(payload)?;
    let session_runtime = state.get_session(&session_id).await?;
    let mut overlay_rules = Vec::with_capacity(payload.rules.len());
    for rule in payload.rules {
        overlay_rules.push(rule.into_domain()?);
    }

    let policy_state = policy_state_for(&state);
    let policy_version = {
        let guard = lock_policy_state(&policy_state);
        guard.runtime.set_session_overlay(
            session_id.clone(),
            SessionOverlay::new(overlay_rules.clone()),
        )
    };

    {
        let mut session = session_runtime.lock().await;
        session.policy_overlay_rules = overlay_rules.clone();
    }

    Ok(Json(PolicyOverlayResponse {
        request_id: ctx.request_id,
        session_id,
        policy_version,
        rule_count: overlay_rules.len(),
        status: "set",
        timestamp: now_rfc3339(),
    }))
}

pub async fn clear_session_policy_overlay(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<PolicyOverlayResponse>, ApiError> {
    let session_runtime = state.get_session(&session_id).await?;
    let policy_state = policy_state_for(&state);
    let policy_version = {
        let guard = lock_policy_state(&policy_state);
        guard.runtime.clear_session_overlay(&session_id)
    };
    {
        let mut session = session_runtime.lock().await;
        session.policy_overlay_rules.clear();
    }
    Ok(Json(PolicyOverlayResponse {
        request_id: ctx.request_id,
        session_id,
        policy_version,
        rule_count: 0,
        status: "cleared",
        timestamp: now_rfc3339(),
    }))
}
