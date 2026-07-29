use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::Notify;
use viewer_domain::{EntityId, ImageRequestId, SessionId, image::ImageRepresentationKind};

#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub request_id: ImageRequestId,
    pub cancellation: ImageRequestCancellation,
    pub session_id: SessionId,
    pub entity_id: EntityId,
    pub source: PathBuf,
    pub kind: ImageRepresentationKind,
}

#[derive(Clone, Debug)]
pub struct ImageRequestCancellation {
    inner: Arc<ImageRequestCancellationState>,
}

#[derive(Debug)]
struct ImageRequestCancellationState {
    cancelled: AtomicBool,
    changed: Notify,
}

impl ImageRequestCancellation {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ImageRequestCancellationState {
                cancelled: AtomicBool::new(false),
                changed: Notify::new(),
            }),
        }
    }

    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::AcqRel) {
            self.inner.changed.notify_waiters();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let changed = self.inner.changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if self.is_cancelled() {
            return;
        }
        changed.await;
    }
}

impl Default for ImageRequestCancellation {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageBackend {
    QuickLook,
    ImageIo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageArtifact {
    pub cache_path: PathBuf,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub backend: ImageBackend,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unsupported image")]
    Unsupported,
    #[error("image is corrupt")]
    Corrupt,
    #[error("decode exceeds budget")]
    BudgetExceeded,
    #[error("image request was cancelled")]
    Cancelled,
    #[error("image io failed: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::ImageRequestCancellation;
    use std::time::Duration;

    #[tokio::test]
    async fn cancelling_an_image_request_wakes_its_waiters() {
        let cancellation = ImageRequestCancellation::new();
        let waiting = {
            let cancellation = cancellation.clone();
            tokio::spawn(async move { cancellation.cancelled().await })
        };
        tokio::task::yield_now().await;

        cancellation.cancel();

        tokio::time::timeout(Duration::from_millis(100), waiting)
            .await
            .expect("cancelled request waiter should wake")
            .unwrap();
        assert!(cancellation.is_cancelled());
    }

    #[tokio::test]
    async fn waiting_after_image_request_cancellation_returns_immediately() {
        let cancellation = ImageRequestCancellation::new();
        cancellation.cancel();

        tokio::time::timeout(Duration::from_millis(100), cancellation.cancelled())
            .await
            .expect("already cancelled request should not block");
    }
}
