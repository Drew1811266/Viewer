#[path = "support/continuous_review.rs"]
mod continuous_review;

use continuous_review::Fixture;
use viewer_application::{ReviewTaskCancellation, review_workspace::*};
use viewer_domain::{
    EntityId, ReviewCommandId,
    review::{FeedbackAnchor, NormalizedPoint, NormalizedRect},
};

fn save(asset_version_id: viewer_domain::AssetVersionId, text: &str) -> ReviewWorkspaceCommand {
    save_anchor(asset_version_id, text, FeedbackAnchor::Asset)
}

fn save_anchor(
    asset_version_id: viewer_domain::AssetVersionId,
    text: &str,
    anchor: FeedbackAnchor,
) -> ReviewWorkspaceCommand {
    ReviewWorkspaceCommand::SaveFeedback {
        feedback_id: None,
        text: text.to_owned(),
        targets: vec![TargetEdit::Add {
            asset_version_id,
            anchor,
        }],
    }
}

async fn prepared_asset(service: &ContinuousReviewService) -> viewer_domain::AssetVersionId {
    service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap()
        .remove(0)
        .id
}

#[tokio::test]
async fn extended_anchor_promotes_publication_to_v4_without_later_downgrade() {
    let fixture = Fixture::new();
    let service = fixture.authoring_service();
    let asset = prepared_asset(&service).await;
    let first = service
        .apply_authoring_with_cancellation(
            service
                .prepare(
                    ReviewCommandId::from_u128(70),
                    None,
                    save(asset, "整图意见"),
                )
                .await
                .unwrap(),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let first_view = first.patch.apply(None).unwrap();
    assert_eq!(
        first_view.authoring.publication_protocol,
        ReviewPublicationProtocol::V3
    );

    let second = service
        .apply_authoring_with_cancellation(
            service
                .prepare(
                    ReviewCommandId::from_u128(71),
                    Some(first_view.authoring.head.snapshot_id),
                    save_anchor(
                        asset,
                        "点标记",
                        FeedbackAnchor::ImagePoint(NormalizedPoint::new(0.25, 0.4).unwrap()),
                    ),
                )
                .await
                .unwrap(),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let second_view = second.patch.apply(Some(first_view)).unwrap();
    assert_eq!(
        second_view.authoring.publication_protocol,
        ReviewPublicationProtocol::V4
    );
    let point_feedback = second_view
        .authoring
        .state
        .feedback
        .iter()
        .find(|feedback| matches!(feedback.targets[0].anchor, FeedbackAnchor::ImagePoint(_)))
        .unwrap();
    let point_target = &point_feedback.targets[0];
    let point_key = viewer_domain::review::continuous::TargetVersionKey {
        feedback_id: point_feedback.id,
        text_revision_id: point_feedback.text_revision_id,
        target_id: point_target.id,
        target_revision_id: point_target.revision_id,
    };

    let third = service
        .apply_authoring_with_cancellation(
            service
                .prepare(
                    ReviewCommandId::from_u128(72),
                    Some(second_view.authoring.head.snapshot_id),
                    ReviewWorkspaceCommand::Withdraw {
                        targets: vec![point_key],
                    },
                )
                .await
                .unwrap(),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let third_view = third.patch.apply(Some(second_view)).unwrap();
    assert_eq!(
        third_view.authoring.publication_protocol,
        ReviewPublicationProtocol::V4
    );
}

#[tokio::test]
async fn authoring_apply_does_not_touch_evidence_or_reload_the_view() {
    let fixture = Fixture::new();
    fixture.evidence.fail_if_called();
    fixture.repository.fail_if_opened();
    fixture.assets.fail_if_sources_checked();
    let service = fixture.authoring_service();
    let asset = service
        .prepare_assets(
            &[EntityId::from_u128(10)],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap()
        .remove(0);
    let envelope = service
        .prepare(
            ReviewCommandId::from_u128(80),
            None,
            save(asset.id, "收紧袖口"),
        )
        .await
        .unwrap();

    let result = service
        .apply_authoring_with_cancellation(envelope, ReviewTaskCancellation::default())
        .await
        .unwrap();

    assert!(matches!(
        result.publication,
        ReviewPublicationStatus::Pending {
            pending_revisions: 1
        }
    ));
    assert_eq!(result.patch.upsert_feedback.len(), 1);
    assert_eq!(
        result.patch.apply(None).unwrap(),
        service
            .view_authoring_current(viewer_domain::ReviewStreamId::from_u128(2))
            .await
            .unwrap()
            .unwrap()
    );
    assert_eq!(fixture.evidence.captures(), 0);
    assert_eq!(fixture.evidence.renders(), 0);
    assert_eq!(fixture.repository.opens(), 0);
    assert_eq!(fixture.assets.source_checks(), 0);
}

#[tokio::test]
async fn duplicate_authoring_command_returns_the_original_receipt_and_patch() {
    let fixture = Fixture::new();
    let service = fixture.authoring_service();
    let asset = prepared_asset(&service).await;
    let envelope = service
        .prepare(
            ReviewCommandId::from_u128(81),
            None,
            save(asset, "收紧袖口"),
        )
        .await
        .unwrap();

    let first = service
        .apply_authoring_with_cancellation(envelope.clone(), ReviewTaskCancellation::default())
        .await
        .unwrap();
    let duplicate = service
        .apply_authoring_with_cancellation(envelope, ReviewTaskCancellation::default())
        .await
        .unwrap();

    assert_eq!(duplicate.receipt, first.receipt);
    assert_eq!(duplicate.patch, first.patch);
    assert_eq!(duplicate.publication, first.publication);
}

#[tokio::test]
async fn foreground_region_and_text_only_saves_keep_slow_ports_out_of_the_path() {
    let fixture = Fixture::new();
    fixture.evidence.fail_if_called();
    fixture.repository.fail_if_opened();
    fixture.assets.fail_if_sources_checked();
    let service = fixture.authoring_service();
    let asset = prepared_asset(&service).await;
    let first_envelope = service
        .prepare(
            ReviewCommandId::from_u128(82),
            None,
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: None,
                text: "局部需要收紧".into(),
                targets: vec![TargetEdit::Add {
                    asset_version_id: asset,
                    anchor: FeedbackAnchor::ImageRect(
                        NormalizedRect::new(0.1, 0.2, 0.3, 0.25).unwrap(),
                    ),
                }],
            },
        )
        .await
        .unwrap();
    let first = service
        .apply_authoring_with_cancellation(first_envelope, ReviewTaskCancellation::default())
        .await
        .unwrap();
    let before = first.patch.apply(None).unwrap();
    let feedback_id = before.authoring.state.feedback[0].id;
    let second_envelope = service
        .prepare(
            ReviewCommandId::from_u128(83),
            Some(before.authoring.head.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(feedback_id),
                text: "局部需要明显收紧".into(),
                targets: vec![],
            },
        )
        .await
        .unwrap();

    let second = service
        .apply_authoring_with_cancellation(second_envelope, ReviewTaskCancellation::default())
        .await
        .unwrap();
    let after = second.patch.apply(Some(before)).unwrap();

    assert_eq!(after.authoring.state.feedback[0].text, "局部需要明显收紧");
    assert_eq!(
        after,
        service
            .view_authoring_current(viewer_domain::ReviewStreamId::from_u128(2))
            .await
            .unwrap()
            .unwrap()
    );
    assert_eq!(fixture.evidence.captures(), 0);
    assert_eq!(fixture.evidence.renders(), 0);
    assert_eq!(fixture.repository.opens(), 0);
    assert_eq!(fixture.assets.source_checks(), 0);
}

#[tokio::test]
async fn redraw_and_withdraw_patches_match_the_authoring_projection() {
    let fixture = Fixture::new();
    let service = fixture.authoring_service();
    let asset = prepared_asset(&service).await;
    let first = service
        .apply_authoring_with_cancellation(
            service
                .prepare(ReviewCommandId::from_u128(84), None, save(asset, "袖口"))
                .await
                .unwrap(),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    let initial = first.patch.apply(None).unwrap();
    let feedback = &initial.authoring.state.feedback[0];
    let target = &feedback.targets[0];
    let key = viewer_domain::review::continuous::TargetVersionKey {
        feedback_id: feedback.id,
        text_revision_id: feedback.text_revision_id,
        target_id: target.id,
        target_revision_id: target.revision_id,
    };
    let redraw = service
        .prepare(
            ReviewCommandId::from_u128(85),
            Some(initial.authoring.head.snapshot_id),
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id: Some(feedback.id),
                text: feedback.text.clone(),
                targets: vec![TargetEdit::Redraw {
                    key,
                    asset_version_id: asset,
                    anchor: FeedbackAnchor::ImageRect(
                        NormalizedRect::new(0.2, 0.25, 0.2, 0.2).unwrap(),
                    ),
                }],
            },
        )
        .await
        .unwrap();
    let redrawn = service
        .apply_authoring_with_cancellation(redraw, ReviewTaskCancellation::default())
        .await
        .unwrap();
    let after_redraw = redrawn.patch.apply(Some(initial)).unwrap();
    let feedback = &after_redraw.authoring.state.feedback[0];
    let target = &feedback.targets[0];
    let redrawn_key = viewer_domain::review::continuous::TargetVersionKey {
        feedback_id: feedback.id,
        text_revision_id: feedback.text_revision_id,
        target_id: target.id,
        target_revision_id: target.revision_id,
    };
    let withdraw = service
        .prepare(
            ReviewCommandId::from_u128(86),
            Some(after_redraw.authoring.head.snapshot_id),
            ReviewWorkspaceCommand::Withdraw {
                targets: vec![redrawn_key],
            },
        )
        .await
        .unwrap();
    let withdrawn = service
        .apply_authoring_with_cancellation(withdraw, ReviewTaskCancellation::default())
        .await
        .unwrap();
    let final_view = withdrawn.patch.apply(Some(after_redraw)).unwrap();

    assert!(final_view.authoring.state.feedback.is_empty());
    assert_eq!(
        final_view,
        service
            .view_authoring_current(viewer_domain::ReviewStreamId::from_u128(2))
            .await
            .unwrap()
            .unwrap()
    );
}

#[tokio::test]
async fn reused_command_identity_with_a_different_payload_is_rejected() {
    let fixture = Fixture::new();
    let service = fixture.authoring_service();
    let asset = prepared_asset(&service).await;
    let command = ReviewCommandId::from_u128(87);
    let first = service
        .prepare(command, None, save(asset, "第一条"))
        .await
        .unwrap();
    service
        .apply_authoring_with_cancellation(first, ReviewTaskCancellation::default())
        .await
        .unwrap();
    let conflicting = service
        .prepare(command, None, save(asset, "不同内容"))
        .await
        .unwrap();

    assert_eq!(
        service
            .apply_authoring_with_cancellation(conflicting, ReviewTaskCancellation::default(),)
            .await,
        Err(ReviewCommitError::CommandConflict.into())
    );
}
