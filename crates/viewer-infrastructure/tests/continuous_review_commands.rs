use viewer_application::review_workspace::*;
use viewer_domain::{
    review::{FeedbackAnchor, continuous::*},
    *,
};
use viewer_infrastructure::review::ContinuousReviewCommandCodec;

fn envelope() -> ReviewCommandEnvelope {
    ReviewCommandEnvelope {
        context: ReviewWorkspaceContext {
            project_id: ProjectId::from_u128(1),
            stream_id: ReviewStreamId::from_u128(2),
            production: None,
        },
        command_id: ReviewCommandId::from_u128(3),
        expected_snapshot_id: None,
        payload_digest: [0; 32],
        generated: GeneratedReviewIds {
            snapshot_id: ReviewSnapshotId::from_u128(4),
            feedback_id: FeedbackId::from_u128(5),
            text_revision_id: ReviewTextRevisionId::from_u128(6),
            archive_id: ReviewArchiveId::from_u128(7),
            targets: vec![(
                ReviewTargetId::from_u128(8),
                ReviewTargetRevisionId::from_u128(9),
            )],
            created_at_ms: 100,
        },
        command: ReviewWorkspaceCommand::SaveFeedback {
            feedback_id: None,
            text: "保留自然语言\n不要修改颜色".into(),
            targets: vec![TargetEdit::Add {
                asset_version_id: AssetVersionId::from_u128(10),
                anchor: FeedbackAnchor::Asset,
            }],
        },
    }
}
#[test]
fn canonical_digest_ignores_digest_field_but_binds_every_command_field() {
    let codec = ContinuousReviewCommandCodec;
    let e = envelope();
    let digest = codec.digest(&e).unwrap();
    let mut changed = e.clone();
    changed.payload_digest = [9; 32];
    assert_eq!(codec.digest(&changed).unwrap(), digest);
    let mut variants = vec![];
    let mut v = e.clone();
    v.context.project_id = ProjectId::new();
    variants.push(v);
    let mut v = e.clone();
    v.context.stream_id = ReviewStreamId::new();
    variants.push(v);
    let mut v = e.clone();
    v.expected_snapshot_id = Some(ReviewSnapshotId::new());
    variants.push(v);
    let mut v = e.clone();
    v.generated.created_at_ms += 1;
    variants.push(v);
    let mut v = e.clone();
    v.generated.snapshot_id = ReviewSnapshotId::new();
    variants.push(v);
    let mut v = e.clone();
    v.generated.targets[0].1 = ReviewTargetRevisionId::new();
    variants.push(v);
    let mut v = e.clone();
    if let ReviewWorkspaceCommand::SaveFeedback { text, .. } = &mut v.command {
        text.push('。');
    }
    variants.push(v);
    for v in variants {
        assert_ne!(codec.digest(&v).unwrap(), digest);
    }
    assert_eq!(codec.digest(&e).unwrap(), digest);
}
#[test]
fn archive_proof_and_selection_are_in_the_digest() {
    let codec = ContinuousReviewCommandCodec;
    let mut e = envelope();
    let key = TargetVersionKey {
        feedback_id: e.generated.feedback_id,
        text_revision_id: e.generated.text_revision_id,
        target_id: e.generated.targets[0].0,
        target_revision_id: e.generated.targets[0].1,
    };
    e.command = ReviewWorkspaceCommand::Archive(ArchiveSelection {
        expected_snapshot_id: e.generated.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Unknown,
            targets: vec![key],
        }],
    });
    let digest = codec.digest(&e).unwrap();
    if let ReviewWorkspaceCommand::Archive(selection) = &mut e.command {
        selection.groups[0].basis = ArchiveBasis::Known {
            snapshot: SnapshotRef {
                snapshot_id: e.generated.snapshot_id,
                blake3: [2; 32],
            },
            source: ArchiveBasisSource::AgentDeclared {
                usage_id: ReviewUsageId::new(),
            },
        };
    }
    assert_ne!(codec.digest(&e).unwrap(), digest);
}
