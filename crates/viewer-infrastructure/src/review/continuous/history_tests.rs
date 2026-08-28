use super::*;
use crate::review::continuous::repository::ContinuousReviewRepository;
use viewer_application::review_workspace::*;
use viewer_domain::{ProjectId, ReviewSnapshotId, review::continuous::ContinuousReviewState};

#[test]
fn repeated_ancestry_proofs_reuse_only_the_operation_local_verified_edges() {
    let root = tempfile::tempdir().unwrap();
    let project = ProjectId::from_u128(1);
    let stream_id = ReviewStreamId::from_u128(2);
    let repository = ContinuousReviewRepository::open(root.path(), project, true).unwrap();
    let mut expected = None;
    let mut first = None;
    for i in 10..20 {
        let receipt = repository
            .commit(ReviewCommitRequest {
                expected,
                production: None,
                next: PreparedContinuousSnapshot {
                    state: ContinuousReviewState::empty(
                        project,
                        stream_id,
                        ReviewSnapshotId::from_u128(i),
                    ),
                    command_id: ReviewCommandId::from_u128(i),
                    payload_digest: [i as u8; 32],
                    changes: vec![],
                    evidence: vec![],
                },
                archives: vec![],
                adopted_usage: vec![],
                staged_evidence: vec![],
            })
            .unwrap();
        expected = Some(receipt.snapshot);
        if first.is_none() {
            first = expected;
        }
    }
    let first = first.unwrap();
    let view = repository.view().unwrap().unwrap();
    reachable(&view, stream_id, &first).unwrap();
    DOCUMENT_READS.with(|count| count.set(0));
    reachable(&view, stream_id, &first).unwrap();
    assert_eq!(
        DOCUMENT_READS.with(|count| count.get()),
        1,
        "intermediate edges already have hash proofs; still re-read the requested record and its evidence"
    );
    let next_view = repository.view().unwrap().unwrap();
    DOCUMENT_READS.with(|count| count.set(0));
    reachable(&next_view, stream_id, &first).unwrap();
    assert_eq!(
        DOCUMENT_READS.with(|count| count.get()),
        10,
        "a new operation never inherits old trust"
    );
    std::fs::write(
        root.path()
            .join(format!(".viewer/reviews/states/{}.json", first.snapshot_id)),
        b"corrupt requested record",
    )
    .unwrap();
    assert_eq!(
        reachable(&view, stream_id, &first),
        Err(ReviewCommitError::Integrity)
    );
}
