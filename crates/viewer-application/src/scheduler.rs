use std::sync::Mutex;
use viewer_domain::{SessionId, search::Generation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskClass {
    CurrentQuery,
    FolderPublication,
    VisibleDerived,
    TextIndex,
    Fingerprint,
}

impl TaskClass {
    pub const fn priority(self) -> u8 {
        match self {
            Self::CurrentQuery | Self::FolderPublication => 1,
            Self::VisibleDerived => 2,
            Self::TextIndex => 3,
            Self::Fingerprint => 4,
        }
    }

    pub const fn queue_capacity(self) -> usize {
        match self {
            Self::CurrentQuery | Self::FolderPublication => 8,
            Self::VisibleDerived => 32,
            Self::TextIndex => 8,
            Self::Fingerprint => 4,
        }
    }
}

#[derive(Debug, Default)]
struct CoordinatorState {
    epoch: u64,
    active: Option<(SessionId, Generation)>,
}

#[derive(Debug, Default)]
pub struct TaskCoordinator {
    state: Mutex<CoordinatorState>,
}

impl TaskCoordinator {
    pub fn begin_session(&self, session_id: SessionId) -> Generation {
        let mut state = self.lock_state();
        let generation = next_generation(&mut state);
        state.active = Some((session_id, generation));
        generation
    }

    pub fn bump_generation(&self, session_id: SessionId) -> Option<Generation> {
        let mut state = self.lock_state();
        if !matches!(state.active, Some((active, _)) if active == session_id) {
            return None;
        }
        let generation = next_generation(&mut state);
        state.active = Some((session_id, generation));
        Some(generation)
    }

    pub fn is_publishable(&self, session_id: SessionId, generation: Generation) -> bool {
        self.lock_state().active == Some((session_id, generation))
    }

    /// Runs a short synchronous action while the session token is held current.
    /// Session cancellation and generation changes wait until the action returns.
    pub fn run_if_current<T>(
        &self,
        session_id: SessionId,
        generation: Generation,
        action: impl FnOnce() -> T,
    ) -> Option<T> {
        let state = self.lock_state();
        if state.active != Some((session_id, generation)) {
            return None;
        }
        let result = action();
        drop(state);
        Some(result)
    }

    pub(crate) fn publish_if_current<T>(
        &self,
        session_id: SessionId,
        generation: Generation,
        publish: impl FnOnce() -> T,
    ) -> Option<T> {
        self.run_if_current(session_id, generation, publish)
    }

    pub fn cancel_session(&self, session_id: SessionId) -> bool {
        let mut state = self.lock_state();
        if !matches!(state.active, Some((active, _)) if active == session_id) {
            return false;
        }
        state.active = None;
        true
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, CoordinatorState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn next_generation(state: &mut CoordinatorState) -> Generation {
    state.epoch = state.epoch.wrapping_add(1);
    if state.epoch == 0 {
        state.epoch = 1;
    }
    Generation::new(state.epoch)
}

#[cfg(test)]
mod tests {
    use super::{TaskClass, TaskCoordinator};
    use viewer_domain::SessionId;

    #[test]
    fn coordinator_invalidates_old_and_cancelled_session_work() {
        let coordinator = TaskCoordinator::default();
        let session = SessionId::new();
        let old = coordinator.begin_session(session);
        let current = coordinator.bump_generation(session).unwrap();
        assert!(!coordinator.is_publishable(session, old));
        assert!(coordinator.is_publishable(session, current));
        assert!(coordinator.cancel_session(session));
        assert!(!coordinator.is_publishable(session, current));
        assert_eq!(coordinator.bump_generation(session), None);
    }

    #[test]
    fn scheduler_queues_are_bounded_and_priority_ordered() {
        assert_eq!(TaskClass::CurrentQuery.priority(), 1);
        assert_eq!(TaskClass::FolderPublication.priority(), 1);
        assert!(TaskClass::VisibleDerived.priority() < TaskClass::TextIndex.priority());
        assert!(TaskClass::TextIndex.priority() < TaskClass::Fingerprint.priority());
        for class in [
            TaskClass::CurrentQuery,
            TaskClass::FolderPublication,
            TaskClass::VisibleDerived,
            TaskClass::TextIndex,
            TaskClass::Fingerprint,
        ] {
            assert!(class.queue_capacity() > 0);
        }
    }

    #[test]
    fn publication_callback_does_not_run_for_stale_work() {
        let coordinator = TaskCoordinator::default();
        let session = SessionId::new();
        let old = coordinator.begin_session(session);
        coordinator.bump_generation(session).unwrap();
        let mut published = false;
        let result = coordinator.publish_if_current(session, old, || published = true);
        assert_eq!(result, None);
        assert!(!published);
    }

    #[test]
    fn guarded_action_does_not_run_after_the_session_is_cancelled() {
        let coordinator = TaskCoordinator::default();
        let session = SessionId::new();
        let generation = coordinator.begin_session(session);
        assert!(coordinator.cancel_session(session));

        let mut ran = false;
        let result = coordinator.run_if_current(session, generation, || ran = true);

        assert_eq!(result, None);
        assert!(!ran);
    }
}
