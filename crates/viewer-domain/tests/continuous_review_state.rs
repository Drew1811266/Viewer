#[path = "support/continuous.rs"]
mod support;

use support::*;
use viewer_domain::review::continuous::*;
use viewer_domain::review::*;
use viewer_domain::*;

#[test]
fn empty_current_is_valid_and_does_not_invent_pass_outcomes() {
    let state = ContinuousReviewState::empty(
        ProjectId::from_u128(1),
        ReviewStreamId::from_u128(2),
        ReviewSnapshotId::from_u128(3),
    );
    assert!(state.feedback.is_empty());
    assert!(state.assets.is_empty());
    assert_eq!(state.validate(), Ok(()));
}

#[test]
fn targets_require_registered_assets() {
    let mut state = state();
    state.assets.pop();
    assert_eq!(
        state.validate(),
        Err(ContinuousReviewError::MissingReference)
    );
}

#[test]
fn identities_are_unique_across_the_entire_state() {
    let original = state();
    let mut duplicate_asset = original.clone();
    duplicate_asset.assets.push(asset(1));
    let mut duplicate_feedback = original.clone();
    duplicate_feedback.feedback.push(feedback(10, &[(13, 1)]));
    let mut duplicate_target = original.clone();
    duplicate_target.feedback.push(feedback(20, &[(11, 1)]));
    let mut duplicate_revision = original.clone();
    duplicate_revision.feedback[0].targets[1].revision_id = ReviewTargetRevisionId::from_u128(11);
    for invalid in [
        duplicate_asset,
        duplicate_feedback,
        duplicate_target,
        duplicate_revision,
    ] {
        assert_eq!(
            invalid.validate(),
            Err(ContinuousReviewError::DuplicateIdentity)
        );
    }
}

#[test]
fn different_asset_versions_may_share_a_path() {
    let mut state = state();
    state.assets[1].relative_path = state.assets[0].relative_path.clone();
    state.assets[1].parent_asset_version_id = Some(state.assets[0].id);
    assert_eq!(state.validate(), Ok(()));
}

#[test]
fn feedback_requires_text_targets_and_a_valid_timestamp() {
    for (text, targets, time) in [(" \n", true, 10), ("评审", false, 10), ("评审", true, -1)] {
        let mut state = state();
        state.feedback[0].text = text.into();
        state.feedback[0].created_at_ms = time;
        if !targets {
            state.feedback[0].targets.clear();
        }
        assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
    }
}

#[test]
fn pending_reasons_are_nonempty_and_unique() {
    let mut state = state();
    for reasons in [vec![], vec![ReviewPendingReason::SourceChanged; 2]] {
        state.feedback[0].targets[0].availability = ReviewAvailability::NeedsConfirmation(reasons);
        assert!(state.validate().is_err());
    }
    state.feedback[0].targets[0].availability =
        ReviewAvailability::NeedsConfirmation(vec![ReviewPendingReason::SourceChanged]);
    assert_eq!(state.validate(), Ok(()));
}

#[test]
fn anchor_and_media_validation_cannot_be_bypassed() {
    let mut state = state();
    state.feedback[0].targets[0].anchor = FeedbackAnchor::VideoPoint { position_us: 0 };
    assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
    state.assets[0].media = ReviewMedia::Video {
        duration_us: Some(100),
        display_width: None,
        display_height: None,
    };
    for anchor in [
        FeedbackAnchor::VideoRange {
            start_us: 80,
            end_us: 20,
        },
        FeedbackAnchor::VideoRange {
            start_us: 0,
            end_us: 101,
        },
        FeedbackAnchor::VideoPoint { position_us: 101 },
    ] {
        state.feedback[0].targets[0].anchor = anchor;
        assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
    }
    assert!(NormalizedRect::new(f64::NAN, 0.1, 0.2, 0.3).is_err());
    assert!(NormalizedPoint::new(f64::INFINITY, 0.1).is_err());
}

#[test]
fn malformed_asset_dimensions_and_self_parent_are_rejected() {
    let mut state = state();
    state.assets[0].media = ReviewMedia::Image {
        width: Some(0),
        height: Some(100),
    };
    assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
    state.assets[0] = asset(1);
    state.assets[0].parent_asset_version_id = Some(state.assets[0].id);
    assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
    state.assets[0] = asset(1);
    state.parent = Some(reference(&state));
    assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
}

#[test]
fn state_and_feedback_payloads_are_bounded() {
    let mut state = state();
    state.feedback[0].text = "a".repeat(MAX_FEEDBACK_TEXT_BYTES + 1);
    assert_eq!(state.validate(), Err(ContinuousReviewError::LimitExceeded));
    state.feedback[0].text = "a".repeat(MAX_FEEDBACK_TEXT_BYTES);
    assert_eq!(state.validate(), Ok(()));
    state.feedback[0].targets =
        vec![state.feedback[0].targets[0].clone(); MAX_TARGETS_PER_FEEDBACK + 1];
    assert_eq!(state.validate(), Err(ContinuousReviewError::LimitExceeded));
    state.feedback.clear();
    state.assets = vec![asset(1); MAX_ASSETS_PER_ROUND + 1];
    assert_eq!(state.validate(), Err(ContinuousReviewError::LimitExceeded));
    state.assets.clear();
    state.feedback = vec![feedback(1, &[(1, 1)]); MAX_FEEDBACK_ITEMS_PER_ROUND + 1];
    assert_eq!(state.validate(), Err(ContinuousReviewError::LimitExceeded));
}

#[test]
fn aggregate_stroke_points_are_bounded() {
    let mut state = state();
    let stroke = ImageStroke::new(
        (0..MAX_IMAGE_STROKE_POINTS)
            .map(|i| {
                let coordinate = i as f64 / (MAX_IMAGE_STROKE_POINTS - 1) as f64;
                NormalizedPoint::new(coordinate, coordinate).unwrap()
            })
            .collect(),
    )
    .unwrap();
    state.feedback[0].targets = (0..(MAX_IMAGE_STROKE_POINTS_PER_ROUND / MAX_IMAGE_STROKE_POINTS
        + 1))
        .map(|i| {
            let mut target = VersionedTarget::asset(
                ReviewTargetId::from_u128(i as u128),
                ReviewTargetRevisionId::from_u128(i as u128),
                AssetVersionId::from_u128(1),
            );
            target.anchor = FeedbackAnchor::ImageStroke(stroke.clone());
            target
        })
        .collect();
    assert_eq!(state.validate(), Err(ContinuousReviewError::LimitExceeded));
}

#[test]
fn history_is_nonempty_unique_and_bound_to_the_same_context() {
    let mut state = state();
    let history = HistoryRef {
        project_id: state.project_id,
        stream_id: state.stream_id,
        source: HistorySource::Snapshot {
            snapshot: SnapshotRef {
                snapshot_id: ReviewSnapshotId::from_u128(1),
                blake3: [1; 32],
            },
            keys: vec![key(&state, 11)],
        },
    };
    state.feedback[0].history_ref = Some(history.clone());
    assert_eq!(state.validate(), Ok(()));
    state.feedback[0].history_ref.as_mut().unwrap().stream_id = ReviewStreamId::from_u128(99);
    assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
    state.feedback[0].history_ref = Some(history);
    if let HistorySource::Snapshot { keys, .. } =
        &mut state.feedback[0].history_ref.as_mut().unwrap().source
    {
        keys.clear();
    }
    assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
}

#[test]
fn legacy_history_is_hash_bound_and_uses_only_the_declared_round() {
    let mut state = state();
    state.feedback[0].history_ref = Some(HistoryRef {
        project_id: state.project_id,
        stream_id: state.stream_id,
        source: HistorySource::Legacy {
            round_id: ReviewRoundId::from_u128(1),
            record_blake3: [2; 32],
            targets: vec![LegacyTargetRef {
                round_id: ReviewRoundId::from_u128(2),
                feedback_id: FeedbackId::from_u128(10),
                target_index: 0,
            }],
        },
    });
    assert_eq!(state.validate(), Err(ContinuousReviewError::InvalidData));
}
