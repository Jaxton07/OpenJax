use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum DispatchError {
    #[error("invalid dispatcher state transition from {from} to {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
}
