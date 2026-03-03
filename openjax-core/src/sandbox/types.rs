#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClass {
    General,
    ProcessObserve,
    NetworkHeavy,
    WriteHeavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxResultClass {
    Success,
    PartialSuccess,
    Failure,
}

impl SandboxResultClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::PartialSuccess => "partial_success",
            Self::Failure => "failure",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SandboxRuntimeOutcome {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub backend_used: String,
    pub degrade_reason: Option<String>,
    pub policy_decision: String,
    pub runtime_allowed: bool,
    pub runtime_deny_reason: Option<String>,
}
