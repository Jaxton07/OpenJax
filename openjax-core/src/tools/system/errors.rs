use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum SystemToolError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("not supported on this platform: {0}")]
    NotSupported(String),
    #[error("failed to collect metrics: {0}")]
    CollectionFailed(String),
}

impl SystemToolError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument(_) => "invalid_argument",
            Self::PermissionDenied(_) => "permission_denied",
            Self::NotSupported(_) => "not_supported",
            Self::CollectionFailed(_) => "collection_failed",
        }
    }
}
