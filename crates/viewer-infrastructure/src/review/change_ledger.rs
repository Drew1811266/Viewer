use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use viewer_domain::EntityId;

#[derive(Clone, Default)]
pub struct ReviewChangeLedger {
    state: Arc<Mutex<ReviewChangeLedgerState>>,
}

#[derive(Default)]
struct ReviewChangeLedgerState {
    next_revision: u64,
    member_revisions: HashMap<EntityId, u64>,
}

impl ReviewChangeLedger {
    pub fn replace_members(&self, entity_ids: &[EntityId]) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.member_revisions.clear();
        state
            .member_revisions
            .extend(entity_ids.iter().copied().map(|entity_id| (entity_id, 0)));
    }

    pub fn record(&self, entity_id: EntityId) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if !state.member_revisions.contains_key(&entity_id) {
            return;
        }
        state.next_revision = state.next_revision.saturating_add(1);
        let revision = state.next_revision;
        state.member_revisions.insert(entity_id, revision);
    }

    pub fn revision(&self, entity_id: EntityId) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .member_revisions
            .get(&entity_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn clear(&self) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .member_revisions
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_records_only_current_members_with_monotonic_revisions() {
        let ledger = ReviewChangeLedger::default();
        let first = EntityId::from_u128(1);
        let second = EntityId::from_u128(2);
        let ignored = EntityId::from_u128(3);
        ledger.replace_members(&[first, second, first]);

        ledger.record(ignored);
        ledger.record(first);
        ledger.record(second);

        assert_eq!(ledger.revision(ignored), 0);
        assert_eq!(ledger.revision(first), 1);
        assert_eq!(ledger.revision(second), 2);
        ledger.clear();
        assert_eq!(ledger.revision(first), 0);
    }
}
