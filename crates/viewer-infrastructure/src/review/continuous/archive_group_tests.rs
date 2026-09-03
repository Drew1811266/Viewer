use super::*;
use crate::review::continuous::{archives, repository::ContinuousReviewRepository};
use viewer_application::review_workspace::*;
use viewer_domain::{
    review::{continuous::*, *},
    *,
};

#[test]
fn shared_known_archive_basis_reads_one_state_per_checkpoint_without_reordering_groups() {
    let root = tempfile::tempdir().unwrap();
    let project = ProjectId::new();
    let stream = ReviewStreamId::new();
    let repository = ContinuousReviewRepository::open(root.path(), project, true).unwrap();
    let png = viewer_test_support::image_fixtures::image_fixture("alpha.png");
    let bytes = std::fs::read(&png).unwrap();
    let reference = EvidenceRef {
        blake3: *blake3::hash(&bytes).as_bytes(),
        size_bytes: bytes.len() as u64,
        width: 640,
        height: 480,
    };
    let asset = AssetVersion {
        id: AssetVersionId::new(),
        source_entity_id: None,
        relative_path: RelativePath::parse("alpha.png").unwrap(),
        evidence: AssetEvidence {
            size_bytes: reference.size_bytes,
            modified_ns: 10,
            blake3: Some(reference.blake3),
        },
        media: ReviewMedia::Image {
            width: Some(640),
            height: Some(480),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    };
    let mut state = ContinuousReviewState::empty(project, stream, ReviewSnapshotId::new());
    state.feedback = vec![VersionedFeedback {
        id: FeedbackId::new(),
        text_revision_id: ReviewTextRevisionId::new(),
        text: "保留每组原始顺序".into(),
        created_at_ms: 10,
        history_ref: None,
        targets: (0..101)
            .map(|_| {
                VersionedTarget::asset(
                    ReviewTargetId::new(),
                    ReviewTargetRevisionId::new(),
                    asset.id,
                )
            })
            .collect(),
    }];
    state.assets = vec![asset.clone()];
    let keys: Vec<_> = state.feedback[0]
        .targets
        .iter()
        .map(|t| state.target_key(t.id).unwrap())
        .collect();
    let evidence = vec![ReviewEvidenceBinding {
        asset_version_id: asset.id,
        capability: EvidenceCapability::Image {
            base: reference.clone(),
            annotated: None,
            annotations: vec![],
        },
    }];
    let first = repository
        .commit(ReviewCommitRequest {
            expected: None,
            production: None,
            next: PreparedContinuousSnapshot {
                publication_protocol: ReviewPublicationProtocol::V3,
                state: state.clone(),
                command_id: ReviewCommandId::new(),
                payload_digest: [1; 32],
                changes: keys
                    .iter()
                    .map(|k| ReviewChange {
                        target_id: k.target_id,
                        before: None,
                        after: Some(*k),
                        kind: ReviewChangeKind::Added,
                        archive_id: None,
                        historical_key: None,
                    })
                    .collect(),
                evidence: evidence.clone(),
            },
            archives: vec![],
            adopted_usage: vec![],
            staged_evidence: vec![PreparedEvidenceFile {
                path: png,
                reference: reference.clone(),
            }],
        })
        .unwrap();
    let selection = ArchiveSelection {
        expected_snapshot_id: state.snapshot_id,
        groups: keys
            .iter()
            .enumerate()
            .map(|(i, key)| ArchiveGroup {
                basis: if i == 50 {
                    ArchiveBasis::Unknown
                } else {
                    ArchiveBasis::Known {
                        snapshot: first.snapshot,
                        source: ArchiveBasisSource::UserSelected,
                    }
                },
                targets: vec![*key],
            })
            .collect(),
    };
    let (next, plan) = apply_archive(
        &state,
        std::slice::from_ref(&state),
        &selection,
        &[],
        ReviewSnapshotId::new(),
    )
    .unwrap();
    let checkpoint =
        ArchiveCheckpoint::from_plan(&state, first.snapshot, &plan, ReviewArchiveId::new(), 20)
            .unwrap();
    DOCUMENT_READS.with(|c| c.set(0));
    assert_eq!(
        archives::verify_checkpoint(&repository.view().unwrap().unwrap(), &checkpoint, &state)
            .unwrap(),
        plan
    );
    assert_eq!(
        DOCUMENT_READS.with(|c| c.get()),
        1,
        "100 Known groups sharing an exact basis must not reread its JSON/PNG 100 times"
    );
    repository
        .commit(ReviewCommitRequest {
            expected: Some(first.snapshot),
            production: None,
            next: PreparedContinuousSnapshot {
                publication_protocol: ReviewPublicationProtocol::V3,
                state: next,
                command_id: ReviewCommandId::new(),
                payload_digest: [2; 32],
                changes: plan.changes(checkpoint.archive_id),
                evidence,
            },
            archives: vec![checkpoint.clone()],
            adopted_usage: vec![],
            staged_evidence: vec![],
        })
        .unwrap();
    DOCUMENT_READS.with(|c| c.set(0));
    assert_eq!(
        repository
            .load_archive(stream, checkpoint.archive_id)
            .unwrap(),
        checkpoint
    );
    assert_eq!(
        DOCUMENT_READS.with(|c| c.get()),
        4,
        "full public archive lookup stays constant in the number of shared groups"
    );
    let mut forged = checkpoint.clone();
    if let ArchiveBasis::Known { snapshot, .. } = &mut forged.groups[100].basis {
        snapshot.blake3 = [9; 32];
    }
    assert_eq!(
        archives::verify_checkpoint(&repository.view().unwrap().unwrap(), &forged, &state),
        Err(ReviewCommitError::Integrity)
    );
    std::fs::write(
        root.path().join(format!(
            ".viewer/reviews/evidence/{}.png",
            blake3::Hash::from_bytes(reference.blake3).to_hex()
        )),
        b"corrupt",
    )
    .unwrap();
    assert_eq!(
        repository.load_archive(stream, checkpoint.archive_id),
        Err(ReviewCommitError::Integrity)
    );
}
