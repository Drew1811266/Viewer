use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;
use viewer_application::ReviewTaskCancellation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TaskError {
    Closed,
    Busy,
    WorkerFailed,
}

#[derive(Default)]
struct State {
    closed: bool,
    next: u64,
    active: HashMap<u64, ReviewTaskCancellation>,
}

/// Owns work, not just callers. Dropping a command future cannot detach its resource lifetime
/// from project close. Registration and closing are serialized under the same short lock.
#[derive(Default)]
pub(in crate::state) struct ReviewWorkspaceTasks {
    state: Mutex<State>,
    finished: Notify,
}
impl ReviewWorkspaceTasks {
    pub(super) async fn run<T: Send + 'static, F: Future<Output = T> + Send + 'static>(
        self: &Arc<Self>,
        operation: impl FnOnce(ReviewTaskCancellation) -> F + Send + 'static,
    ) -> Result<T, TaskError> {
        let task = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return Err(TaskError::Closed);
            }
            if state.active.len() >= 128 {
                return Err(TaskError::Busy);
            }
            let id = state.next;
            state.next = state.next.checked_add(1).ok_or(TaskError::Busy)?;
            let cancel = ReviewTaskCancellation::default();
            state.active.insert(id, cancel.clone());
            let guard = Work {
                owner: self.clone(),
                id,
            };
            tokio::spawn(async move {
                let _guard = guard;
                operation(cancel).await
            })
        };
        task.await.map_err(|_| TaskError::WorkerFailed)
    }

    pub fn cancel(&self) -> usize {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        for task in state.active.values() {
            task.cancel();
        }
        state.active.len()
    }

    pub fn revoke(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        for task in state.active.values() {
            task.cancel();
        }
    }

    pub async fn close(&self) {
        self.revoke();
        loop {
            let notified = self.finished.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .active
                .is_empty()
            {
                return;
            }
            notified.await;
        }
    }
}
struct Work {
    owner: Arc<ReviewWorkspaceTasks>,
    id: u64,
}
impl Drop for Work {
    fn drop(&mut self) {
        self.owner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active
            .remove(&self.id);
        self.owner.finished.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn review_workspace_close_waits_for_cancelled_work_even_if_caller_is_dropped() {
        let tasks = Arc::new(ReviewWorkspaceTasks::default());
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let owner = tasks.clone();
        let caller = tokio::spawn(async move {
            owner
                .run(move |cancel| async move {
                    entered_tx.send(cancel.clone()).unwrap();
                    release_rx.await.unwrap();
                    cancel.is_cancelled()
                })
                .await
        });
        let cancellation = entered_rx.await.unwrap();
        caller.abort();
        let owner = tasks.clone();
        let closing = tokio::spawn(async move { owner.close().await });
        tokio::task::yield_now().await;
        assert!(cancellation.is_cancelled());
        assert!(
            !closing.is_finished(),
            "close must not clean resources while detached work is alive"
        );
        release_tx.send(()).unwrap();
        closing.await.unwrap();
        assert!(tasks.run(|_| async { 7 }).await.is_err());
    }

    #[tokio::test]
    async fn review_workspace_cancel_does_not_poison_the_next_operation() {
        let tasks = Arc::new(ReviewWorkspaceTasks::default());
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let owner = tasks.clone();
        let work = tokio::spawn(async move {
            owner
                .run(move |cancel| async move {
                    entered_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    cancel.is_cancelled()
                })
                .await
                .unwrap()
        });
        entered_rx.await.unwrap();
        assert_eq!(tasks.cancel(), 1);
        release_tx.send(()).unwrap();
        assert!(work.await.unwrap());
        assert!(
            !tasks
                .run(|cancel| async move { cancel.is_cancelled() })
                .await
                .unwrap()
        );
        tasks.close().await;
    }
}
