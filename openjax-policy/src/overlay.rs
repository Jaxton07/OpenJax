use std::collections::BTreeMap;

use crate::schema::PolicyRule;

#[derive(Debug, Clone, Default)]
pub struct SessionOverlay {
    pub rules: Vec<PolicyRule>,
}

impl SessionOverlay {
    pub fn new(rules: Vec<PolicyRule>) -> Self {
        Self { rules }
    }
}

pub type SessionOverlayMap = BTreeMap<String, SessionOverlay>;
