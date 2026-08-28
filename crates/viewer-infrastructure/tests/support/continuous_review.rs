#![allow(dead_code)]
use std::fs;
use tempfile::TempDir;
use viewer_application::review_workspace::*;
use viewer_domain::review::{AssetEvidence, AssetVersion, ReviewMedia, continuous::*};
use viewer_domain::{
    AssetVersionId, FeedbackId, ProjectId, RelativePath, ReviewArchiveId, ReviewCommandId,
    ReviewSnapshotId, ReviewStreamId, ReviewTargetId, ReviewTargetRevisionId, ReviewTextRevisionId,
};
use viewer_infrastructure::review::ProjectReviewRepositoryProvider;

pub fn request(sequence: u128, expected: Option<SnapshotRef>) -> ReviewCommitRequest {
    ReviewCommitRequest {
        expected,
        production: None,
        next: PreparedContinuousSnapshot {
            state: ContinuousReviewState::empty(
                ProjectId::from_u128(1),
                ReviewStreamId::from_u128(2),
                ReviewSnapshotId::from_u128(sequence),
            ),
            command_id: ReviewCommandId::from_u128(sequence),
            payload_digest: [sequence as u8; 32],
            changes: vec![],
            evidence: vec![],
        },
        archives: vec![],
        adopted_usage: vec![],
        staged_evidence: vec![],
    }
}

pub fn setup() -> (TempDir, ProjectReviewRepositoryProvider) {
    let root = TempDir::new().unwrap();
    let provider = ProjectReviewRepositoryProvider::new(root.path(), ProjectId::from_u128(1));
    (root, provider)
}

pub fn key(feedback: &VersionedFeedback, index: usize) -> TargetVersionKey {
    TargetVersionKey {
        feedback_id: feedback.id,
        text_revision_id: feedback.text_revision_id,
        target_id: feedback.targets[index].id,
        target_revision_id: feedback.targets[index].revision_id,
    }
}

pub fn feedback_request() -> ReviewCommitRequest {
    let mut value = request(3, None);
    value.next.state.assets = vec![AssetVersion {
        id: AssetVersionId::from_u128(10),
        source_entity_id: None,
        relative_path: RelativePath::parse("clip.mp4").unwrap(),
        evidence: AssetEvidence {
            size_bytes: 1,
            modified_ns: 1,
            blake3: Some([1; 32]),
        },
        media: ReviewMedia::Video {
            duration_us: Some(100),
            display_width: Some(640),
            display_height: Some(480),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }];
    let feedback = VersionedFeedback {
        id: FeedbackId::from_u128(20),
        text_revision_id: ReviewTextRevisionId::from_u128(21),
        text: "保留原材质，收紧袖口。".into(),
        created_at_ms: 1,
        history_ref: None,
        targets: vec![
            VersionedTarget::asset(
                ReviewTargetId::from_u128(30),
                ReviewTargetRevisionId::from_u128(31),
                AssetVersionId::from_u128(10),
            ),
            VersionedTarget::asset(
                ReviewTargetId::from_u128(40),
                ReviewTargetRevisionId::from_u128(41),
                AssetVersionId::from_u128(10),
            ),
        ],
    };
    value.next.changes = (0..2)
        .map(|index| ReviewChange {
            target_id: feedback.targets[index].id,
            before: None,
            after: Some(key(&feedback, index)),
            kind: ReviewChangeKind::Added,
            archive_id: None,
            historical_key: None,
        })
        .collect();
    value.next.state.feedback = vec![feedback];
    value.next.evidence = vec![ReviewEvidenceBinding {
        asset_version_id: AssetVersionId::from_u128(10),
        capability: EvidenceCapability::NotImage,
    }];
    value
}

pub fn archive_request(
    current: &StoredContinuousSnapshot,
    basis: &StoredContinuousSnapshot,
    sequence: u128,
) -> ReviewCommitRequest {
    let selection = ArchiveSelection {
        expected_snapshot_id: current.state.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Known {
                snapshot: basis.reference,
                source: ArchiveBasisSource::UserSelected,
            },
            targets: vec![key(&basis.state.feedback[0], 0)],
        }],
    };
    let (state, plan) = apply_archive(
        &current.state,
        std::slice::from_ref(&basis.state),
        &selection,
        &[],
        ReviewSnapshotId::from_u128(sequence),
    )
    .unwrap();
    let id = ReviewArchiveId::from_u128(sequence + 100);
    let checkpoint =
        ArchiveCheckpoint::from_plan(&current.state, current.reference, &plan, id, 2).unwrap();
    let mut value = request(sequence, Some(current.reference));
    value.next.state = state;
    value.next.changes = plan.changes(id);
    value.next.evidence = current.evidence.clone();
    value.archives = vec![checkpoint];
    value
}

pub fn image_request(root: &TempDir) -> ReviewCommitRequest {
    let mut value = feedback_request();
    let path = root.path().join("prepared.png");
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        &path,
    )
    .unwrap();
    let bytes = fs::read(&path).unwrap();
    let digest = *blake3::hash(&bytes).as_bytes();
    let reference = EvidenceRef {
        blake3: digest,
        size_bytes: bytes.len() as u64,
        width: 640,
        height: 480,
    };
    value.next.state.assets[0].relative_path = RelativePath::parse("prepared.png").unwrap();
    value.next.state.assets[0].media = ReviewMedia::Image {
        width: Some(640),
        height: Some(480),
    };
    value.next.state.assets[0].evidence = AssetEvidence {
        size_bytes: bytes.len() as u64,
        modified_ns: 1,
        blake3: Some(digest),
    };
    value.next.evidence[0].capability = EvidenceCapability::Image {
        base: reference.clone(),
        annotated: None,
        annotations: vec![],
    };
    value.staged_evidence = vec![PreparedEvidenceFile { path, reference }];
    value
}
