use serde_json::Value;

use crate::Agent;
use crate::agent::decision::ModelDecision;
use crate::agent::planner::ToolActionContext;
use crate::agent::tool_projection::{observe_tool_args, tool_input_to_args};

pub(super) enum NativeToolExecOutcome {
    Result { model_content: String, ok: bool },
    Aborted,
}

impl Agent {
    pub(super) async fn execute_native_tool_call(
        &mut self,
        turn_id: u64,
        tool_call_id: &str,
        tool_name: &str,
        input: &Value,
        ctx: &mut ToolActionContext<'_>,
    ) -> NativeToolExecOutcome {
        if let Err(outcome) = self.ensure_native_tool_name(turn_id, tool_name, ctx) {
            return outcome;
        }

        let args = tool_input_to_args(input);
        observe_tool_args(&args, ctx);

        if let Some(outcome) = self.guard_duplicate_native_tool_call(turn_id, tool_name, &args, ctx)
        {
            return outcome;
        }

        self.execute_native_tool_call_body(turn_id, tool_call_id, tool_name, args, ctx)
            .await
    }

    #[allow(dead_code)]
    pub(super) async fn handle_tool_action(
        &mut self,
        turn_id: u64,
        decision: &ModelDecision,
        ctx: &mut ToolActionContext<'_>,
    ) -> bool {
        let Some(tool_name) = self.ensure_model_decision_tool_name(turn_id, decision, ctx) else {
            return false;
        };

        let args = decision.args.clone().unwrap_or_default();
        observe_tool_args(&args, ctx);

        if let Some(outcome) =
            self.guard_duplicate_legacy_tool_call(turn_id, &tool_name, &args, ctx)
        {
            return outcome;
        }

        self.execute_legacy_tool_action_body(turn_id, tool_name, args, ctx)
            .await
    }
}
