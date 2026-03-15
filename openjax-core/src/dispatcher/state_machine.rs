use super::{DispatchBranch, DispatchError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DispatchState {
    Init,
    Probing,
    LockedText,
    LockedToolCall,
    Completed,
    Error,
}

impl DispatchState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Probing => "probing",
            Self::LockedText => "locked_text",
            Self::LockedToolCall => "locked_tool_call",
            Self::Completed => "completed",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DispatchStateMachine {
    state: DispatchState,
}

impl DispatchStateMachine {
    pub(crate) fn new() -> Self {
        Self {
            state: DispatchState::Init,
        }
    }

    #[cfg(test)]
    pub(crate) fn state(self) -> DispatchState {
        self.state
    }

    pub(crate) fn enter_probing(&mut self) {
        self.state = DispatchState::Probing;
    }

    pub(crate) fn lock(&mut self, branch: DispatchBranch) {
        self.state = match branch {
            DispatchBranch::Text => DispatchState::LockedText,
            DispatchBranch::ToolCall => DispatchState::LockedToolCall,
        };
    }

    pub(crate) fn complete(&mut self) -> Result<(), DispatchError> {
        if matches!(
            self.state,
            DispatchState::LockedText | DispatchState::LockedToolCall
        ) {
            self.state = DispatchState::Completed;
            return Ok(());
        }
        let from = self.state.as_str();
        self.state = DispatchState::Error;
        Err(DispatchError::InvalidTransition {
            from,
            to: DispatchState::Completed.as_str(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_reaches_completed_from_locked_text() {
        let mut machine = DispatchStateMachine::new();
        machine.enter_probing();
        machine.lock(DispatchBranch::Text);
        assert!(machine.complete().is_ok());
        assert_eq!(machine.state(), DispatchState::Completed);
    }

    #[test]
    fn state_machine_rejects_invalid_complete_transition() {
        let mut machine = DispatchStateMachine::new();
        machine.enter_probing();
        let err = machine.complete().expect_err("complete should fail");
        assert!(matches!(err, DispatchError::InvalidTransition { .. }));
    }
}
