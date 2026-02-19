#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalOverlay {
    pub visible: bool,
    pub prompt: String,
}

impl ApprovalOverlay {
    pub fn hidden() -> Self {
        Self {
            visible: false,
            prompt: String::new(),
        }
    }
}
