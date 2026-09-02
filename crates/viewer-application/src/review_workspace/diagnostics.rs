use std::{sync::Arc, time::Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewSaveStage {
    AuthoringCommit,
    EvidenceMaterialization,
    PublicV3Publish,
    WorkspaceRefresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewSaveMeasurement {
    pub stage: ReviewSaveStage,
    pub elapsed_us: u64,
}

pub trait ReviewSaveObserverPort: Send + Sync {
    fn record(&self, measurement: ReviewSaveMeasurement);
}

#[derive(Default)]
pub struct NoReviewSaveObserver;

impl ReviewSaveObserverPort for NoReviewSaveObserver {
    fn record(&self, _: ReviewSaveMeasurement) {}
}

pub(super) struct ReviewSaveSpan {
    observer: Arc<dyn ReviewSaveObserverPort>,
    stage: ReviewSaveStage,
    started_at: Instant,
}

impl ReviewSaveSpan {
    pub(super) fn start(observer: Arc<dyn ReviewSaveObserverPort>, stage: ReviewSaveStage) -> Self {
        Self {
            observer,
            stage,
            started_at: Instant::now(),
        }
    }
}

impl Drop for ReviewSaveSpan {
    fn drop(&mut self) {
        self.observer.record(ReviewSaveMeasurement {
            stage: self.stage,
            elapsed_us: self
                .started_at
                .elapsed()
                .as_micros()
                .min(u128::from(u64::MAX)) as u64,
        });
    }
}
