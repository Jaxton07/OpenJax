use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequestUi {
    pub request_id: String,
    pub turn_id: u64,
    pub target: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalSelection {
    Approve,
    Deny,
    Cancel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalOverlayState {
    pub request_id: String,
    pub summary: String,
    pub selected_index: usize,
}

#[derive(Debug, Default, Clone)]
pub struct ApprovalState {
    pub pending: VecDeque<String>,
    pub requests: HashMap<String, ApprovalRequestUi>,
    pub focus: Option<String>,
    pub overlay_visible: bool,
    pub overlay: Option<ApprovalOverlayState>,
}

impl ApprovalState {
    pub fn add_request(&mut self, req: ApprovalRequestUi) {
        let id = req.request_id.clone();
        self.requests.insert(id.clone(), req);
        self.pending.push_back(id.clone());
        self.focus = Some(id);
        self.overlay_visible = true;
        self.sync_overlay();
    }

    pub fn resolve_request(&mut self, request_id: &str) {
        self.requests.remove(request_id);
        let mut next = VecDeque::new();
        for id in self.pending.drain(..) {
            if id != request_id {
                next.push_back(id);
            }
        }
        self.pending = next;
        if self.focus.as_deref() == Some(request_id) {
            self.focus = self.pending.back().cloned();
        }
        if self.requests.is_empty() {
            self.focus = None;
        }
        self.overlay_visible = !self.pending.is_empty();
        self.sync_overlay();
    }

    pub fn sync_overlay(&mut self) {
        let Some(focus_id) = self.focus.as_ref() else {
            self.overlay = None;
            self.overlay_visible = false;
            return;
        };
        let Some(req) = self.requests.get(focus_id) else {
            self.overlay = None;
            self.overlay_visible = false;
            return;
        };
        self.overlay = Some(ApprovalOverlayState {
            request_id: req.request_id.clone(),
            summary: format!(
                "Approval required ({}) allow {} for turn-{}? Reason: {}",
                req.request_id.chars().take(8).collect::<String>(),
                req.target.replace('_', " "),
                req.turn_id,
                req.reason
            ),
            selected_index: self.overlay.as_ref().map(|v| v.selected_index).unwrap_or(0),
        });
        self.overlay_visible = true;
    }

    pub fn move_selection(&mut self, direction: i32) {
        let Some(overlay) = self.overlay.as_mut() else {
            return;
        };
        let current = overlay.selected_index as i32;
        overlay.selected_index = (current + direction).clamp(0, 2) as usize;
    }

    pub fn selection(&self) -> Option<ApprovalSelection> {
        let overlay = self.overlay.as_ref()?;
        Some(match overlay.selected_index {
            0 => ApprovalSelection::Approve,
            1 => ApprovalSelection::Deny,
            _ => ApprovalSelection::Cancel,
        })
    }

    pub fn pending_count(&self) -> usize {
        self.requests.len()
    }

    pub fn focused_request_id(&self) -> Option<String> {
        self.focus.clone()
    }
}
