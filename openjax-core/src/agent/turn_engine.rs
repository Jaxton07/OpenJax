#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnEnginePhase {
    Planning,
    StreamingText,
    ToolBatchRunning,
    StreamingResumed,
    Completed,
    Failed,
}

pub(crate) struct TurnEngine {
    phase: TurnEnginePhase,
}

impl TurnEngine {
    pub(crate) fn new() -> Self {
        Self {
            phase: TurnEnginePhase::Planning,
        }
    }

    pub(crate) fn phase(&self) -> TurnEnginePhase {
        self.phase
    }

    pub(crate) fn on_response_started(&mut self) {
        self.phase = TurnEnginePhase::StreamingText;
    }

    pub(crate) fn on_tool_batch_started(&mut self) {
        self.phase = TurnEnginePhase::ToolBatchRunning;
    }

    pub(crate) fn on_response_resumed(&mut self) {
        self.phase = TurnEnginePhase::StreamingResumed;
    }

    pub(crate) fn on_completed(&mut self) {
        self.phase = TurnEnginePhase::Completed;
    }

    pub(crate) fn on_failed(&mut self) {
        self.phase = TurnEnginePhase::Failed;
    }
}
