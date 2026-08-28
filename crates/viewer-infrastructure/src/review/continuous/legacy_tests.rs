use super::*;
use viewer_domain::{review::continuous::*, *};

#[test]
fn shared_legacy_origins_are_verified_once_per_current_operation() {
    let root = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/review-protocol/review-round-v1.valid.json"),
    )
    .unwrap();
    let old = protocol::decode_completed(&bytes).unwrap();
    std::fs::create_dir(root.path().join("rounds")).unwrap();
    std::fs::write(
        root.path()
            .join(format!("rounds/{}.json", old.review_round_id)),
        &bytes,
    )
    .unwrap();
    let digest = *blake3::hash(&bytes).as_bytes();
    let view = View {
        directory: Directory::open(root.path()).unwrap(),
        index: v3::ReviewIndexV3 {
            project_id: old.project_id,
            legacy_index: None,
            streams: vec![v3::ReviewStreamV3 {
                review_stream_id: old.review_stream_id,
                task_id: old
                    .production
                    .as_ref()
                    .map(|s| s.task_id.as_str().to_owned()),
                batch_id: old
                    .production
                    .as_ref()
                    .map(|s| s.batch_id.as_str().to_owned()),
                current_ref: None,
                archive_refs: vec![],
                usage_refs: vec![],
                legacy_refs: vec![v3::LegacyRecordRef {
                    kind: v3::LegacyRecordKind::Completed,
                    round_id: old.review_round_id,
                    protocol_version: protocol::REVIEW_PROTOCOL_V1.into(),
                    location: format!("rounds/{}.json", old.review_round_id),
                    blake3: digest,
                }],
            }],
        },
        index_bytes: None,
        ancestry: Default::default(),
    };
    let mut state = ContinuousReviewState::empty(
        old.project_id,
        old.review_stream_id,
        ReviewSnapshotId::new(),
    );
    state.assets = old.assets.clone();
    for _ in 0..100 {
        state.feedback.push(VersionedFeedback {
            id: FeedbackId::new(),
            text_revision_id: ReviewTextRevisionId::new(),
            text: "continue".into(),
            created_at_ms: 100,
            targets: vec![VersionedTarget::asset(
                ReviewTargetId::new(),
                ReviewTargetRevisionId::new(),
                old.feedback[0].targets[0].asset_version_id,
            )],
            history_ref: Some(HistoryRef {
                project_id: old.project_id,
                stream_id: old.review_stream_id,
                source: HistorySource::Legacy {
                    round_id: old.review_round_id,
                    record_blake3: digest,
                    targets: vec![LegacyTargetRef {
                        round_id: old.review_round_id,
                        feedback_id: old.feedback[0].id,
                        target_index: 0,
                    }],
                },
            }),
        });
    }
    let record = v3::ReviewStateRecord {
        state,
        command_id: ReviewCommandId::new(),
        payload_digest: [0; 32],
        changes: vec![],
        evidence: vec![],
    };
    LEGACY_READS.with(|c| c.set(0));
    super::super::references::feedback_origins(&view, &record).unwrap();
    assert_eq!(LEGACY_READS.with(|c| c.get()), 1);
}
