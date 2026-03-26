use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
    decide,
    overlay::{SessionOverlay, SessionOverlayMap},
    schema::{PolicyDecision, PolicyInput},
    store::PolicyStore,
};

#[derive(Debug, Clone)]
pub struct PolicySnapshot {
    pub version: u64,
    pub store: PolicyStore,
    pub overlays: SessionOverlayMap,
}

#[derive(Clone)]
pub struct PolicyRuntime {
    current_snapshot: Arc<RwLock<Arc<PolicySnapshot>>>,
}

impl PolicyRuntime {
    pub fn new(initial_store: PolicyStore) -> Self {
        let initial_snapshot = PolicySnapshot {
            version: 1,
            store: initial_store,
            overlays: SessionOverlayMap::new(),
        };
        Self {
            current_snapshot: Arc::new(RwLock::new(Arc::new(initial_snapshot))),
        }
    }

    pub fn current_version(&self) -> u64 {
        self.snapshot().version
    }

    pub fn handle(&self) -> PolicyHandle {
        PolicyHandle {
            snapshot: self.snapshot(),
        }
    }

    pub fn publish(&self, store: PolicyStore) -> u64 {
        self.swap_snapshot(|snapshot| PolicySnapshot {
            version: snapshot.version + 1,
            store,
            overlays: snapshot.overlays.clone(),
        })
    }

    pub fn set_session_overlay(
        &self,
        session_id: impl Into<String>,
        overlay: SessionOverlay,
    ) -> u64 {
        let session_id = session_id.into();
        self.swap_snapshot(|snapshot| {
            let mut overlays = snapshot.overlays.clone();
            overlays.insert(session_id, overlay);
            PolicySnapshot {
                version: snapshot.version + 1,
                store: snapshot.store.clone(),
                overlays,
            }
        })
    }

    pub fn clear_session_overlay(&self, session_id: &str) -> u64 {
        self.swap_snapshot(|snapshot| {
            let mut overlays = snapshot.overlays.clone();
            overlays.remove(session_id);
            PolicySnapshot {
                version: snapshot.version + 1,
                store: snapshot.store.clone(),
                overlays,
            }
        })
    }

    fn snapshot(&self) -> Arc<PolicySnapshot> {
        self.read_guard().clone()
    }

    fn swap_snapshot(&self, update: impl FnOnce(&PolicySnapshot) -> PolicySnapshot) -> u64 {
        let mut guard = self.write_guard();
        let next = Arc::new(update(guard.as_ref()));
        let version = next.version;
        *guard = next;
        version
    }

    fn read_guard(&self) -> RwLockReadGuard<'_, Arc<PolicySnapshot>> {
        match self.current_snapshot.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn write_guard(&self) -> RwLockWriteGuard<'_, Arc<PolicySnapshot>> {
        match self.current_snapshot.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Clone)]
pub struct PolicyHandle {
    snapshot: Arc<PolicySnapshot>,
}

impl PolicyHandle {
    pub fn policy_version(&self) -> u64 {
        self.snapshot.version
    }

    pub fn default_decision(&self) -> crate::schema::DecisionKind {
        self.snapshot.store.default_decision.clone()
    }

    pub fn decide(&self, input: &PolicyInput) -> PolicyDecision {
        let input_for_snapshot = self.with_snapshot_version(input);

        if let Some(session_id) = input_for_snapshot.session_id.as_deref()
            && let Some(overlay) = self.snapshot.overlays.get(session_id)
        {
            let overlay_decision = decide(
                &input_for_snapshot,
                &overlay.rules,
                self.snapshot.store.default_decision.clone(),
            );
            if overlay_decision.matched_rule_id.is_some() {
                return overlay_decision;
            }
        }

        decide(
            &input_for_snapshot,
            &self.snapshot.store.rules,
            self.snapshot.store.default_decision.clone(),
        )
    }

    fn with_snapshot_version(&self, input: &PolicyInput) -> PolicyInput {
        let mut input_for_snapshot = input.clone();
        input_for_snapshot.policy_version = self.snapshot.version;
        input_for_snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::DecisionKind;

    #[test]
    fn policy_handle_exposes_default_decision() {
        let runtime = PolicyRuntime::new(PolicyStore::new(DecisionKind::Ask, vec![]));
        assert_eq!(runtime.handle().default_decision(), DecisionKind::Ask);

        let runtime2 = PolicyRuntime::new(PolicyStore::new(DecisionKind::Deny, vec![]));
        assert_eq!(runtime2.handle().default_decision(), DecisionKind::Deny);
    }
}
