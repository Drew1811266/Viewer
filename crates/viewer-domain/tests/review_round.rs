use viewer_domain::review::{
    AssetEvidence, AssetVersion, Feedback, FeedbackAnchor, FeedbackTarget, ImageStroke,
    MAX_ASSETS_PER_ROUND, MAX_FEEDBACK_ITEMS_PER_ROUND, MAX_IMAGE_STROKE_POINTS,
    MAX_IMAGE_STROKE_POINTS_PER_ROUND, NormalizedArrow, NormalizedPoint, NormalizedRect,
    ReviewDraft, ReviewMedia, ReviewOutcomeKind, ReviewRoundError, ReviewValueError,
    ReviewabilityFailure,
};
use viewer_domain::{
    AssetVersionId, EntityId, FeedbackId, ProjectId, RelativePath, ReviewRoundId, ReviewStreamId,
};

fn image_asset(id: u128) -> AssetVersion {
    AssetVersion {
        id: AssetVersionId::from_u128(id),
        source_entity_id: None,
        relative_path: RelativePath::parse(&format!("images/{id}.png")).unwrap(),
        evidence: AssetEvidence {
            size_bytes: 100,
            modified_ns: 10,
            blake3: None,
        },
        media: ReviewMedia::Image {
            width: Some(100),
            height: Some(100),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }
}

fn video_asset(id: u128, duration_us: Option<u64>) -> AssetVersion {
    AssetVersion {
        id: AssetVersionId::from_u128(id),
        source_entity_id: None,
        relative_path: RelativePath::parse(&format!("videos/{id}.mp4")).unwrap(),
        evidence: AssetEvidence {
            size_bytes: 200,
            modified_ns: 20,
            blake3: None,
        },
        media: ReviewMedia::Video {
            duration_us,
            display_width: Some(1920),
            display_height: Some(1080),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }
}

fn review_draft(assets: Vec<AssetVersion>) -> ReviewDraft {
    ReviewDraft::new(
        ProjectId::from_u128(1),
        ReviewStreamId::from_u128(2),
        ReviewRoundId::from_u128(3),
        None,
        None,
        10,
        assets,
    )
    .unwrap()
}

fn feedback(
    id: u128,
    asset_version_id: AssetVersionId,
    text: &str,
    anchor: FeedbackAnchor,
) -> Feedback {
    Feedback::new(
        FeedbackId::from_u128(id),
        text.to_owned(),
        12,
        vec![FeedbackTarget {
            asset_version_id,
            anchor,
        }],
    )
    .unwrap()
}

fn diagonal_stroke(point_count: usize) -> ImageStroke {
    ImageStroke::new(
        (0..point_count)
            .map(|index| {
                let value = index as f64 / (point_count - 1) as f64;
                NormalizedPoint::new(value, value).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn normalized_arrows_preserve_direction_and_reject_zero_length() {
    let tail = NormalizedPoint::new(0.1, 0.2).unwrap();
    let head = NormalizedPoint::new(0.8, 0.7).unwrap();
    let arrow = NormalizedArrow::new(tail, head).unwrap();

    assert_eq!(arrow.tail(), tail);
    assert_eq!(arrow.head(), head);
    assert_eq!(
        NormalizedArrow::new(head, head),
        Err(ReviewValueError::InvalidNumber)
    );
}

#[test]
fn image_anchors_accept_normalized_rectangles_and_strokes() {
    let rect = NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap();
    let stroke = ImageStroke::new(vec![
        NormalizedPoint::new(0.1, 0.2).unwrap(),
        NormalizedPoint::new(0.4, 0.6).unwrap(),
    ])
    .unwrap();

    assert_eq!(FeedbackAnchor::ImageRect(rect).kind_name(), "imageRect");
    assert_eq!(
        FeedbackAnchor::ImageStroke(stroke).kind_name(),
        "imageStroke"
    );
}

#[test]
fn image_strokes_reject_invalid_coordinates_and_unbounded_payloads() {
    assert_eq!(
        NormalizedPoint::new(f64::NAN, 0.2),
        Err(ReviewValueError::InvalidNumber)
    );
    assert_eq!(
        NormalizedPoint::new(1.1, 0.2),
        Err(ReviewValueError::InvalidNumber)
    );
    assert_eq!(
        ImageStroke::new(vec![NormalizedPoint::new(0.2, 0.2).unwrap(); 2]),
        Err(ReviewValueError::InvalidNumber),
    );
    assert_eq!(
        ImageStroke::new(vec![
            NormalizedPoint::new(0.2, 0.1).unwrap(),
            NormalizedPoint::new(0.2, 0.8).unwrap(),
        ]),
        Err(ReviewValueError::InvalidNumber),
    );
    assert_eq!(
        ImageStroke::new(
            (0..=MAX_IMAGE_STROKE_POINTS)
                .map(|index| {
                    NormalizedPoint::new(
                        index as f64 / MAX_IMAGE_STROKE_POINTS as f64,
                        index as f64 / MAX_IMAGE_STROKE_POINTS as f64,
                    )
                    .unwrap()
                })
                .collect(),
        ),
        Err(ReviewValueError::LimitExceeded),
    );
}

#[test]
fn local_image_anchors_require_confirmed_image_dimensions() {
    let mut unknown_size = image_asset(1);
    unknown_size.media = ReviewMedia::Image {
        width: None,
        height: None,
    };
    let unknown_id = unknown_size.id;
    let mut image_draft = review_draft(vec![unknown_size]);
    assert_eq!(
        image_draft.upsert_feedback(feedback(
            1,
            unknown_id,
            "标出领口",
            FeedbackAnchor::ImageRect(NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap()),
        )),
        Err(ReviewRoundError::AnchorUnavailable),
    );

    let video = video_asset(2, Some(1_000));
    let video_id = video.id;
    let mut video_draft = review_draft(vec![video]);
    assert_eq!(
        video_draft.upsert_feedback(feedback(
            2,
            video_id,
            "错误媒体",
            FeedbackAnchor::ImageStroke(
                ImageStroke::new(vec![
                    NormalizedPoint::new(0.1, 0.1).unwrap(),
                    NormalizedPoint::new(0.2, 0.3).unwrap(),
                ])
                .unwrap(),
            ),
        )),
        Err(ReviewRoundError::AnchorMediaMismatch),
    );
}

#[test]
fn draft_limits_the_total_number_of_image_stroke_points_after_replacement() {
    let asset = image_asset(1);
    let mut draft = review_draft(vec![asset.clone()]);
    for id in 1..=97 {
        draft
            .upsert_feedback(feedback(
                id,
                asset.id,
                "密集画笔标注",
                FeedbackAnchor::ImageStroke(diagonal_stroke(MAX_IMAGE_STROKE_POINTS)),
            ))
            .unwrap();
    }
    draft
        .upsert_feedback(feedback(
            98,
            asset.id,
            "达到轮次上限",
            FeedbackAnchor::ImageStroke(diagonal_stroke(
                MAX_IMAGE_STROKE_POINTS_PER_ROUND - 97 * MAX_IMAGE_STROKE_POINTS,
            )),
        ))
        .unwrap();

    assert_eq!(
        draft.upsert_feedback(feedback(
            99,
            asset.id,
            "超过轮次上限",
            FeedbackAnchor::ImageStroke(diagonal_stroke(2)),
        )),
        Err(ReviewRoundError::LimitExceeded),
    );
    assert!(
        draft
            .upsert_feedback(feedback(
                98,
                asset.id,
                "替换为更短路径",
                FeedbackAnchor::ImageStroke(diagonal_stroke(2)),
            ))
            .is_ok()
    );
}

#[test]
fn completion_derives_revise_unreviewable_and_default_pass_in_frozen_order() {
    let revise = image_asset(1);
    let unavailable = image_asset(2);
    let pass = image_asset(3);
    let mut draft = review_draft(vec![revise.clone(), unavailable.clone(), pass]);
    draft
        .upsert_feedback(feedback(11, revise.id, "修正手部", FeedbackAnchor::Asset))
        .unwrap();
    draft
        .mark_unreviewable(unavailable.id, ReviewabilityFailure::DecodeFailed)
        .unwrap();

    let snapshot = draft.complete(20).unwrap();
    assert_eq!(
        snapshot
            .outcomes
            .iter()
            .map(|item| item.kind)
            .collect::<Vec<_>>(),
        vec![
            ReviewOutcomeKind::Revise,
            ReviewOutcomeKind::Unreviewable,
            ReviewOutcomeKind::Pass,
        ],
    );
    assert_eq!(
        snapshot.outcomes[0].feedback_ids,
        vec![FeedbackId::from_u128(11)]
    );
    assert_eq!(snapshot.outcomes[0].failure, None);
    assert_eq!(
        snapshot.outcomes[1].failure,
        Some(ReviewabilityFailure::DecodeFailed)
    );
    assert!(snapshot.outcomes[2].feedback_ids.is_empty());
}

#[test]
fn draft_rejects_unknown_targets_media_mismatches_and_early_completion_time() {
    let image = image_asset(1);
    let image_id = image.id;
    let mut draft = review_draft(vec![image]);
    assert_eq!(
        draft.upsert_feedback(feedback(
            1,
            AssetVersionId::from_u128(99),
            "未知素材",
            FeedbackAnchor::Asset,
        )),
        Err(ReviewRoundError::UnknownAsset)
    );
    assert_eq!(
        draft.upsert_feedback(feedback(
            2,
            image_id,
            "错误锚点",
            FeedbackAnchor::VideoPoint { position_us: 1 },
        )),
        Err(ReviewRoundError::AnchorMediaMismatch)
    );
    assert_eq!(draft.complete(0), Err(ReviewRoundError::InvalidTimestamp));
}

#[test]
fn completion_consumes_the_draft_and_default_pass_has_no_exception_payload() {
    let snapshot = review_draft(vec![image_asset(1)]).complete(20).unwrap();
    assert_eq!(snapshot.outcomes[0].kind, ReviewOutcomeKind::Pass);
    assert!(snapshot.outcomes[0].feedback_ids.is_empty());
    assert_eq!(snapshot.outcomes[0].failure, None);
}

#[test]
fn draft_rejects_empty_duplicate_and_invalid_asset_sets() {
    assert_eq!(
        ReviewDraft::new(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewRoundId::from_u128(3),
            None,
            None,
            10,
            vec![],
        ),
        Err(ReviewRoundError::EmptyAssets)
    );

    let duplicate = image_asset(1);
    assert_eq!(
        ReviewDraft::new(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewRoundId::from_u128(3),
            None,
            None,
            10,
            vec![duplicate.clone(), duplicate],
        ),
        Err(ReviewRoundError::DuplicateAssetId)
    );

    let mut duplicate_path = image_asset(2);
    duplicate_path.relative_path = image_asset(1).relative_path;
    assert_eq!(
        ReviewDraft::new(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewRoundId::from_u128(3),
            None,
            None,
            10,
            vec![image_asset(1), duplicate_path],
        ),
        Err(ReviewRoundError::DuplicateAssetPath)
    );

    let mut invalid = image_asset(3);
    invalid.media = ReviewMedia::Image {
        width: Some(0),
        height: Some(100),
    };
    assert_eq!(
        ReviewDraft::new(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewRoundId::from_u128(3),
            None,
            None,
            10,
            vec![invalid],
        ),
        Err(ReviewRoundError::InvalidAsset)
    );
}

#[test]
fn video_anchors_obey_known_bounds_but_unknown_duration_remains_reviewable() {
    let known = video_asset(1, Some(100));
    let unknown = video_asset(2, None);
    let mut draft = review_draft(vec![known.clone(), unknown.clone()]);
    assert_eq!(
        draft.upsert_feedback(feedback(
            1,
            known.id,
            "越界",
            FeedbackAnchor::VideoPoint { position_us: 101 },
        )),
        Err(ReviewRoundError::AnchorOutOfBounds)
    );
    draft
        .upsert_feedback(feedback(
            2,
            known.id,
            "结束帧",
            FeedbackAnchor::VideoRange {
                start_us: 90,
                end_us: 100,
            },
        ))
        .unwrap();
    draft
        .upsert_feedback(feedback(
            3,
            unknown.id,
            "未知时长",
            FeedbackAnchor::VideoPoint {
                position_us: u64::MAX,
            },
        ))
        .unwrap();
}

#[test]
fn feedback_replacement_is_atomic_and_keeps_its_insertion_position() {
    let first = image_asset(1);
    let second = image_asset(2);
    let mut draft = review_draft(vec![first.clone(), second.clone()]);
    draft
        .upsert_feedback(feedback(1, first.id, "第一版", FeedbackAnchor::Asset))
        .unwrap();
    draft
        .upsert_feedback(feedback(2, second.id, "第二条", FeedbackAnchor::Asset))
        .unwrap();
    draft
        .upsert_feedback(feedback(1, second.id, "替换版", FeedbackAnchor::Asset))
        .unwrap();

    assert_eq!(draft.feedback.len(), 2);
    assert_eq!(draft.feedback[0].id, FeedbackId::from_u128(1));
    assert_eq!(draft.feedback[0].text, "替换版");
    assert_eq!(draft.feedback[1].id, FeedbackId::from_u128(2));

    let invalid = feedback(
        1,
        AssetVersionId::from_u128(99),
        "无效替换",
        FeedbackAnchor::Asset,
    );
    assert_eq!(
        draft.upsert_feedback(invalid),
        Err(ReviewRoundError::UnknownAsset)
    );
    assert_eq!(draft.feedback[0].text, "替换版");
}

#[test]
fn feedback_takes_precedence_over_an_unreviewable_marker() {
    let asset = image_asset(1);
    let mut draft = review_draft(vec![asset.clone()]);
    draft
        .mark_unreviewable(asset.id, ReviewabilityFailure::PermissionDenied)
        .unwrap();
    draft
        .upsert_feedback(feedback(1, asset.id, "仍需修正", FeedbackAnchor::Asset))
        .unwrap();

    let outcome = draft.complete(20).unwrap().outcomes.remove(0);
    assert_eq!(outcome.kind, ReviewOutcomeKind::Revise);
    assert_eq!(outcome.feedback_ids, vec![FeedbackId::from_u128(1)]);
    assert_eq!(outcome.failure, None);
}

#[test]
fn image_rects_cannot_target_video_assets() {
    let video = video_asset(1, Some(100));
    let mut draft = review_draft(vec![video.clone()]);
    assert_eq!(
        draft.upsert_feedback(feedback(
            1,
            video.id,
            "错误区域",
            FeedbackAnchor::ImageRect(NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap()),
        )),
        Err(ReviewRoundError::AnchorMediaMismatch)
    );
}

#[test]
fn draft_enforces_the_asset_limit_before_scanning_duplicate_values() {
    assert_eq!(
        ReviewDraft::new(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewRoundId::from_u128(3),
            None,
            None,
            10,
            vec![image_asset(1); MAX_ASSETS_PER_ROUND + 1],
        ),
        Err(ReviewRoundError::LimitExceeded)
    );
}

#[test]
fn draft_rejects_self_parent_zero_duration_and_partial_video_dimensions() {
    let mut self_parent = image_asset(1);
    self_parent.parent_asset_version_id = Some(self_parent.id);

    let mut zero_duration = video_asset(2, Some(0));
    let mut partial_dimensions = video_asset(3, Some(10));
    partial_dimensions.media = ReviewMedia::Video {
        duration_us: Some(10),
        display_width: Some(1920),
        display_height: None,
    };

    for invalid in [self_parent, zero_duration.clone(), partial_dimensions] {
        assert_eq!(
            ReviewDraft::new(
                ProjectId::from_u128(1),
                ReviewStreamId::from_u128(2),
                ReviewRoundId::from_u128(3),
                None,
                None,
                10,
                vec![invalid],
            ),
            Err(ReviewRoundError::InvalidAsset)
        );
    }

    zero_duration.media = ReviewMedia::Video {
        duration_us: None,
        display_width: None,
        display_height: None,
    };
    assert!(review_draft(vec![zero_duration]).assets.len() == 1);
}

#[test]
fn draft_rejects_feedback_before_creation_and_over_the_round_limit() {
    let asset = image_asset(1);
    let mut draft = review_draft(vec![asset.clone()]);
    let early = Feedback::new(
        FeedbackId::from_u128(1),
        "过早".to_owned(),
        9,
        vec![FeedbackTarget {
            asset_version_id: asset.id,
            anchor: FeedbackAnchor::Asset,
        }],
    )
    .unwrap();
    assert_eq!(
        draft.upsert_feedback(early),
        Err(ReviewRoundError::InvalidTimestamp)
    );

    draft.feedback = (0..MAX_FEEDBACK_ITEMS_PER_ROUND)
        .map(|index| feedback(index as u128 + 1, asset.id, "待修正", FeedbackAnchor::Asset))
        .collect();
    assert_eq!(
        draft.upsert_feedback(feedback(
            MAX_FEEDBACK_ITEMS_PER_ROUND as u128 + 1,
            asset.id,
            "超限",
            FeedbackAnchor::Asset,
        )),
        Err(ReviewRoundError::LimitExceeded)
    );
}

#[test]
fn removing_feedback_reports_whether_the_id_was_present() {
    let asset = image_asset(1);
    let mut draft = review_draft(vec![asset.clone()]);
    draft
        .upsert_feedback(feedback(1, asset.id, "待修正", FeedbackAnchor::Asset))
        .unwrap();
    assert!(draft.remove_feedback(FeedbackId::from_u128(1)));
    assert!(!draft.remove_feedback(FeedbackId::from_u128(1)));
    assert!(draft.feedback.is_empty());
}

#[test]
fn unreviewable_markers_require_a_frozen_asset_and_replace_prior_failure() {
    let asset = image_asset(1);
    let mut draft = review_draft(vec![asset.clone()]);
    assert_eq!(
        draft.mark_unreviewable(AssetVersionId::from_u128(99), ReviewabilityFailure::Missing),
        Err(ReviewRoundError::UnknownAsset)
    );
    draft
        .mark_unreviewable(asset.id, ReviewabilityFailure::Unreadable)
        .unwrap();
    draft
        .mark_unreviewable(asset.id, ReviewabilityFailure::PermissionDenied)
        .unwrap();
    let snapshot = draft.complete(20).unwrap();
    assert_eq!(snapshot.outcomes.len(), 1);
    assert_eq!(
        snapshot.outcomes[0].failure,
        Some(ReviewabilityFailure::PermissionDenied)
    );
}

#[test]
fn completion_rejects_feedback_timestamped_after_the_snapshot() {
    let asset = image_asset(1);
    let mut draft = review_draft(vec![asset.clone()]);
    draft
        .upsert_feedback(
            Feedback::new(
                FeedbackId::from_u128(1),
                "未来意见".to_owned(),
                30,
                vec![FeedbackTarget {
                    asset_version_id: asset.id,
                    anchor: FeedbackAnchor::Asset,
                }],
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(draft.complete(20), Err(ReviewRoundError::InvalidTimestamp));
}

#[test]
fn source_identity_and_unavailable_image_bounds_are_explicit_domain_facts() {
    let mut asset = image_asset(1);
    asset.source_entity_id = Some(EntityId::from_u128(99));
    asset.media = ReviewMedia::Image {
        width: None,
        height: None,
    };

    let mut draft = review_draft(vec![asset.clone()]);
    assert_eq!(
        draft.assets[0].source_entity_id,
        Some(EntityId::from_u128(99))
    );
    assert_eq!(
        draft.upsert_feedback(feedback(
            1,
            asset.id,
            "没有真实边界时不能创建区域意见",
            FeedbackAnchor::ImageRect(NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap()),
        )),
        Err(ReviewRoundError::AnchorUnavailable)
    );
    assert_eq!(
        draft.clone().complete(20),
        Err(ReviewRoundError::InvalidAsset)
    );

    draft
        .mark_unreviewable(asset.id, ReviewabilityFailure::DecodeFailed)
        .unwrap();
    assert_eq!(
        draft.complete(20).unwrap().outcomes[0].kind,
        ReviewOutcomeKind::Unreviewable
    );
}

#[test]
fn unavailable_image_bounds_must_be_absent_or_present_as_a_pair() {
    for media in [
        ReviewMedia::Image {
            width: Some(100),
            height: None,
        },
        ReviewMedia::Image {
            width: None,
            height: Some(100),
        },
        ReviewMedia::Image {
            width: Some(0),
            height: Some(100),
        },
    ] {
        let mut asset = image_asset(1);
        asset.media = media;
        assert_eq!(
            ReviewDraft::new(
                ProjectId::from_u128(1),
                ReviewStreamId::from_u128(2),
                ReviewRoundId::from_u128(3),
                None,
                None,
                10,
                vec![asset],
            ),
            Err(ReviewRoundError::InvalidAsset)
        );
    }
}
