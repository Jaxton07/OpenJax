use axum::{Extension, Json, extract::Path, extract::State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::error::ApiError;
use crate::handlers::session::{clear_runtime, handle_compact_action};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommandsResponse {
    pub commands: Vec<SlashCommandDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommandDto {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub usage_hint: String,
    pub kind: String,
    pub replaces_input: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlashExecRequest {
    pub command: String,
}

#[derive(Debug, Serialize)]
pub struct SlashExecResponse {
    pub status: String,
    pub message: Option<String>,
    pub action: Option<String>,
}

/// GET /api/v1/slash_commands
pub async fn list_slash_commands(
    State(_state): State<AppState>,
    Extension(_ctx): Extension<crate::middleware::RequestContext>,
) -> Result<Json<SlashCommandsResponse>, ApiError> {
    use openjax_core::skills::SkillRegistry;
    use openjax_core::slash_commands::SlashCommandRegistry;

    // Ensure dynamic skill commands are refreshed before listing.
    let _ = SkillRegistry::load_from_default_locations();

    let commands = SlashCommandRegistry::all_commands();
    let dtos = commands
        .into_iter()
        .map(|cmd| {
            let replaces_input = cmd.kind.replaces_input();
            let kind = match &cmd.kind {
                openjax_core::slash_commands::SlashCommandKind::Builtin { .. } => "builtin",
                openjax_core::slash_commands::SlashCommandKind::SessionAction { .. } => {
                    "session_action"
                }
                openjax_core::slash_commands::SlashCommandKind::Skill { .. } => "skill",
            };
            SlashCommandDto {
                name: cmd.name.to_string(),
                aliases: cmd
                    .aliases
                    .iter()
                    .map(|alias| (*alias).to_string())
                    .collect(),
                description: cmd.description.to_string(),
                usage_hint: cmd.usage_hint.to_string(),
                kind: kind.to_string(),
                replaces_input,
            }
        })
        .collect::<Vec<_>>();

    Ok(Json(SlashCommandsResponse { commands: dtos }))
}

/// POST /api/v1/sessions/:session_id/slash
pub async fn exec_slash_command(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Extension(ctx): Extension<crate::middleware::RequestContext>,
    Json(payload): Json<SlashExecRequest>,
) -> Result<Json<SlashExecResponse>, ApiError> {
    use openjax_core::slash_commands::{SlashCommandRegistry, dispatch_slash_command};

    let result = dispatch_slash_command(&payload.command);

    match result {
        openjax_core::slash_commands::SlashResult::Ok(msg) => Ok(Json(SlashExecResponse {
            status: "ok".to_string(),
            message: Some(msg),
            action: None,
        })),
        openjax_core::slash_commands::SlashResult::Err(err) => {
            Err(ApiError::invalid_argument(err, serde_json::json!({})))
        }
        openjax_core::slash_commands::SlashResult::Pending => {
            // Look up the command to determine what action to take
            let normalized = payload
                .command
                .trim()
                .strip_prefix('/')
                .unwrap_or(&payload.command);
            let cmd = SlashCommandRegistry::find(normalized)
                .or_else(|| SlashCommandRegistry::find(&payload.command));

            let Some(cmd) = cmd else {
                return Err(ApiError::invalid_argument(
                    "command not found",
                    serde_json::json!({ "command": payload.command }),
                ));
            };

            match cmd.kind {
                openjax_core::slash_commands::SlashCommandKind::SessionAction { action } => {
                    let session_runtime = state.get_session(&session_id).await?;
                    if action == "clear" {
                        clear_runtime(&state, &session_runtime).await;
                        return Ok(Json(SlashExecResponse {
                            status: "ok".to_string(),
                            message: Some("session cleared".to_string()),
                            action: None,
                        }));
                    }
                    if action == "compact" {
                        let turn_id = format!("turn_cmd_{}", Uuid::new_v4().simple());
                        handle_compact_action(
                            &state,
                            &session_runtime,
                            &ctx.request_id,
                            &session_id,
                            &turn_id,
                        )
                        .await?;
                        return Ok(Json(SlashExecResponse {
                            status: "ok".to_string(),
                            message: Some("context compacted".to_string()),
                            action: None,
                        }));
                    }
                    Err(ApiError::invalid_argument(
                        format!("unknown session action: {}", action),
                        serde_json::json!({ "action": action }),
                    ))
                }
                openjax_core::slash_commands::SlashCommandKind::Skill { skill_name } => {
                    Ok(Json(SlashExecResponse {
                        status: "pending".to_string(),
                        message: None,
                        action: Some(format!("skill:{}", skill_name)),
                    }))
                }
                openjax_core::slash_commands::SlashCommandKind::Builtin { .. } => {
                    // Builtin should not reach here since it returns Ok
                    Err(ApiError::invalid_argument(
                        "unexpected builtin command",
                        serde_json::json!({}),
                    ))
                }
            }
        }
    }
}
