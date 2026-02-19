#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalOverlay {
    pub request_id: String,
    pub prompt: String,
}

impl ApprovalOverlay {
    pub fn new(request_id: String, prompt: String) -> Self {
        Self { request_id, prompt }
    }
}
