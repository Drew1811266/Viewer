use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use viewer_domain::{SessionId, search::Generation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedWorkClass {
    CurrentQuery,
    FolderPublication,
    VisibleDerived,
    TextIndex,
    VideoProbe,
    Fingerprint,
}

impl DerivedWorkClass {
    pub const fn priority(self) -> u8 {
        match self {
            Self::CurrentQuery | Self::FolderPublication => 1,
            Self::VisibleDerived => 2,
            Self::TextIndex => 3,
            Self::VideoProbe => 4,
            Self::Fingerprint => 5,
        }
    }

    pub const fn queue_capacity(self) -> usize {
        match self {
            Self::CurrentQuery | Self::FolderPublication => 8,
            Self::VisibleDerived => 32,
            Self::TextIndex => 8,
            Self::VideoProbe => 4,
            Self::Fingerprint => 4,
        }
    }

    pub const fn max_concurrency(self) -> usize {
        match self {
            Self::VideoProbe => 1,
            _ => usize::MAX,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::CurrentQuery => 0,
            Self::FolderPublication => 1,
            Self::VisibleDerived => 2,
            Self::TextIndex => 3,
            Self::VideoProbe => 4,
            Self::Fingerprint => 5,
        }
    }
}

pub type TaskClass = DerivedWorkClass;

#[derive(Debug, Default)]
struct DerivedSchedulerState {
    waiting: [usize; 6],
    running: [usize; 6],
}

#[derive(Debug, Default)]
pub struct DerivedWorkScheduler {
    state: Mutex<DerivedSchedulerState>,
    changed: Notify,
}

impl DerivedWorkScheduler {
    pub async fn acquire(self: &Arc<Self>, class: DerivedWorkClass) -> DerivedWorkPermit {
        let index = class.index();
        {
            let mut state = self.lock_state();
            state.waiting[index] = state.waiting[index].saturating_add(1);
        }
        let mut registration = WaitingRegistration {
            scheduler: Arc::clone(self),
            class,
            registered: true,
        };
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let mut state = self.lock_state();
                if can_start(&state, class) {
                    state.waiting[index] -= 1;
                    state.running[index] = state.running[index].saturating_add(1);
                    registration.registered = false;
                    return DerivedWorkPermit {
                        scheduler: Arc::clone(self),
                        class,
                    };
                }
            }
            changed.await;
        }
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, DerivedSchedulerState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

struct WaitingRegistration {
    scheduler: Arc<DerivedWorkScheduler>,
    class: DerivedWorkClass,
    registered: bool,
}

impl Drop for WaitingRegistration {
    fn drop(&mut self) {
        if !self.registered {
            return;
        }
        let index = self.class.index();
        {
            let mut state = self.scheduler.lock_state();
            debug_assert!(state.waiting[index] > 0);
            state.waiting[index] = state.waiting[index].saturating_sub(1);
        }
        self.scheduler.changed.notify_waiters();
    }
}

#[derive(Debug)]
pub struct DerivedWorkPermit {
    scheduler: Arc<DerivedWorkScheduler>,
    class: DerivedWorkClass,
}

impl Drop for DerivedWorkPermit {
    fn drop(&mut self) {
        let index = self.class.index();
        {
            let mut state = self.scheduler.lock_state();
            debug_assert!(state.running[index] > 0);
            state.running[index] = state.running[index].saturating_sub(1);
        }
        self.scheduler.changed.notify_waiters();
    }
}

fn can_start(state: &DerivedSchedulerState, class: DerivedWorkClass) -> bool {
    let index = class.index();
    state.running[index] < class.max_concurrency()
        && [
            DerivedWorkClass::CurrentQuery,
            DerivedWorkClass::FolderPublication,
            DerivedWorkClass::VisibleDerived,
            DerivedWorkClass::TextIndex,
            DerivedWorkClass::VideoProbe,
            DerivedWorkClass::Fingerprint,
        ]
        .into_iter()
        .filter(|other| other.priority() < class.priority())
        .all(|higher| {
            let higher = higher.index();
            state.waiting[higher] == 0 && state.running[higher] == 0
        })
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
    use super::{DerivedWorkClass, DerivedWorkScheduler, TaskClass, TaskCoordinator};
    use std::{sync::Arc, time::Duration};
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
    fn video_probe_is_ordered_after_publication_and_before_fingerprints() {
        assert!(
            DerivedWorkClass::FolderPublication.priority()
                < DerivedWorkClass::VideoProbe.priority()
        );
        assert!(
            DerivedWorkClass::CurrentQuery.priority() < DerivedWorkClass::VideoProbe.priority()
        );
        assert!(DerivedWorkClass::VideoProbe.priority() < DerivedWorkClass::Fingerprint.priority());
        assert_eq!(DerivedWorkClass::VideoProbe.max_concurrency(), 1);
    }

    #[tokio::test]
    async fn scheduler_runs_at_most_one_video_probe_concurrently() {
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let first = scheduler.acquire(DerivedWorkClass::VideoProbe).await;
        let second = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move { scheduler.acquire(DerivedWorkClass::VideoProbe).await })
        };

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!second.is_finished());
        drop(first);
        tokio::time::timeout(Duration::from_millis(100), second)
            .await
            .expect("second probe should start after the first yields")
            .unwrap();
    }

    #[tokio::test]
    async fn queued_folder_publication_precedes_the_next_video_probe() {
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let first = scheduler.acquire(DerivedWorkClass::VideoProbe).await;
        let next_probe = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move { scheduler.acquire(DerivedWorkClass::VideoProbe).await })
        };
        tokio::task::yield_now().await;
        let publication = scheduler.acquire(DerivedWorkClass::FolderPublication).await;

        drop(first);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), async {
                while !next_probe.is_finished() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .is_err(),
            "next probe must yield while publication is active"
        );
        drop(publication);
        tokio::time::timeout(Duration::from_millis(100), next_probe)
            .await
            .expect("probe should resume after folder publication")
            .unwrap();
    }

    #[tokio::test]
    async fn cancelling_a_waiter_does_not_leave_lower_priority_work_blocked() {
        let scheduler = Arc::new(DerivedWorkScheduler::default());
        let active_probe = scheduler.acquire(DerivedWorkClass::VideoProbe).await;
        let waiting_probe = {
            let scheduler = Arc::clone(&scheduler);
            tokio::spawn(async move { scheduler.acquire(DerivedWorkClass::VideoProbe).await })
        };
        tokio::task::yield_now().await;
        waiting_probe.abort();
        assert!(waiting_probe.await.unwrap_err().is_cancelled());
        drop(active_probe);

        tokio::time::timeout(
            Duration::from_millis(100),
            scheduler.acquire(DerivedWorkClass::VideoProbe),
        )
        .await
        .expect("cancelled video waiter must unregister from the scheduler");
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
