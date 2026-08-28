use super::*;
use viewer_domain::{
    review::{FeedbackAnchor, NormalizedRect, continuous::*},
    *,
};

#[test]
fn recovery_keeps_confirmed_geometry_and_exact_history_or_source_bindings() {
    let context = ReviewWorkspaceContext {
        project_id: ProjectId::new(),
        stream_id: ReviewStreamId::new(),
        production: None,
    };
    let key = TargetVersionKey {
        feedback_id: FeedbackId::new(),
        text_revision_id: ReviewTextRevisionId::new(),
        target_id: ReviewTargetId::new(),
        target_revision_id: ReviewTargetRevisionId::new(),
    };
    let asset = AssetVersionId::new();
    let anchor = FeedbackAnchor::ImageRect(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap());
    let old = LegacyTargetRef {
        round_id: ReviewRoundId::new(),
        feedback_id: key.feedback_id,
        target_index: 0,
    };
    let restore = RestoreChoice::ContinueAsNew {
        feedback_id: FeedbackId::new(),
        text_revision_id: ReviewTextRevisionId::new(),
        target_id: ReviewTargetId::new(),
        target_revision_id: ReviewTargetRevisionId::new(),
        target_asset_version_id: asset,
        confirmed_anchor: Some(anchor.clone()),
        created_at_ms: 10,
    };
    let commands = vec![
        ReviewWorkspaceCommand::ContinueHistorical {
            history_ref: HistoryRef {
                project_id: context.project_id,
                stream_id: context.stream_id,
                source: HistorySource::Snapshot {
                    snapshot: SnapshotRef {
                        snapshot_id: ReviewSnapshotId::new(),
                        blake3: [1; 32],
                    },
                    keys: vec![key],
                },
            },
            bindings: vec![SourceBindingDecision {
                target_key: key,
                new_asset_version_id: asset,
                anchor: anchor.clone(),
                confirmation: SourceBindingConfirmation::UserConfirmed,
            }],
        },
        ReviewWorkspaceCommand::ContinueLegacy {
            history_ref: HistoryRef {
                project_id: context.project_id,
                stream_id: context.stream_id,
                source: HistorySource::Legacy {
                    round_id: old.round_id,
                    record_blake3: [2; 32],
                    targets: vec![old],
                },
            },
            bindings: vec![MigrationBinding {
                legacy_target: old,
                new_asset_version_id: asset,
                anchor: anchor.clone(),
                position_confirmed: true,
            }],
        },
        ReviewWorkspaceCommand::ConfirmSource(SourceBindingDecision {
            target_key: key,
            new_asset_version_id: asset,
            anchor: anchor.clone(),
            confirmation: SourceBindingConfirmation::UserConfirmed,
        }),
        ReviewWorkspaceCommand::ConfirmApplicability {
            key,
            asset_version_id: asset,
            anchor: anchor.clone(),
        },
        ReviewWorkspaceCommand::Restore {
            archive_id: ReviewArchiveId::new(),
            decisions: vec![RestoreDecision {
                historical_key: key,
                choice: restore.clone(),
            }],
        },
    ];
    for command in commands {
        let mut envelope = ReviewCommandEnvelope {
            context: context.clone(),
            command_id: ReviewCommandId::new(),
            expected_snapshot_id: Some(ReviewSnapshotId::new()),
            payload_digest: [3; 32],
            usage_selections: vec![],
            command,
            generated: GeneratedReviewIds {
                snapshot_id: ReviewSnapshotId::new(),
                feedback_id: FeedbackId::new(),
                text_revision_id: ReviewTextRevisionId::new(),
                archive_id: ReviewArchiveId::new(),
                targets: vec![(ReviewTargetId::new(), ReviewTargetRevisionId::new())],
                migration: vec![],
                created_at_ms: 10,
            },
        };
        let recovered = super::recovery::draft(&envelope, ReviewRecoveryFailure::Cancelled)
            .unwrap()
            .editor_input;
        assert_eq!(recovered.targets.len(), 1);
        assert_eq!(recovered.selections.len(), 1);
        assert_eq!(recovered.targets[0].asset_version_id, asset);
        assert_eq!(recovered.targets[0].anchor, anchor);
        assert_eq!(
            recovered.selections[0].confirmation,
            RecoveryTargetConfirmation::UserConfirmed
        );
        if let ReviewWorkspaceCommand::Restore { decisions, .. } = &mut envelope.command {
            let RestoreChoice::ContinueAsNew {
                confirmed_anchor, ..
            } = &mut decisions[0].choice
            else {
                unreachable!()
            };
            *confirmed_anchor = None;
            let unconfirmed = super::recovery::draft(&envelope, ReviewRecoveryFailure::Cancelled)
                .unwrap()
                .editor_input;
            assert!(
                unconfirmed.targets.is_empty(),
                "no invented geometry for an unconfirmed restoration"
            );
            assert_eq!(
                unconfirmed.selections[0].confirmation,
                RecoveryTargetConfirmation::Unconfirmed
            );
        }
    }
}
