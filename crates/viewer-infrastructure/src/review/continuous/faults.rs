use viewer_application::review_workspace::ReviewCommitError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewCommitFaultPoint {
    AfterRecovery,
    AfterEvidence,
    AfterState,
    AfterArchive,
    BeforeIndex,
    AfterIndex,
}

pub trait ReviewCommitFaultInjector: Send + Sync {
    fn check(&self, point: ReviewCommitFaultPoint) -> Result<(), ReviewCommitError>;
}

pub struct NoReviewCommitFaults;
impl ReviewCommitFaultInjector for NoReviewCommitFaults {
    fn check(&self, _point: ReviewCommitFaultPoint) -> Result<(), ReviewCommitError> {
        Ok(())
    }
}
