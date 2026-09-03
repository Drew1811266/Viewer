//! Explicit, repeatable development measurement; no wall-clock pass/fail threshold.
#[path = "support/continuous_review.rs"]
mod support;
use std::time::Instant;
use support::*;
use viewer_application::review_workspace::{
    CURRENT_REVIEW_EVIDENCE_ACTION_POLICY, evidence_action_key,
};
use viewer_domain::{
    review::{FeedbackAnchor, NormalizedArrow, NormalizedPoint, continuous::*},
    *,
};

#[test]
fn measure_extended_anchor_action_key_cost_and_stability() {
    let (root, _) = setup();
    let mut request = image_request(&root);
    let asset = request.next.state.assets[0].id;
    request.next.state.feedback[0].targets[0].anchor = FeedbackAnchor::ImageArrow(
        NormalizedArrow::new(
            NormalizedPoint::new(0.2, 0.3).unwrap(),
            NormalizedPoint::new(0.7, 0.6).unwrap(),
        )
        .unwrap(),
    );
    let expected = evidence_action_key(
        &request.next.state,
        asset,
        CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,
    )
    .unwrap();
    let started = Instant::now();
    for _ in 0..1_000 {
        assert_eq!(
            evidence_action_key(
                &request.next.state,
                asset,
                CURRENT_REVIEW_EVIDENCE_ACTION_POLICY,
            )
            .unwrap(),
            expected
        );
    }
    eprintln!(
        "extended_anchor_action_key_1000={}us",
        started.elapsed().as_micros()
    );
}

#[test]
#[ignore = "archive-heavy IO measurement; run explicitly with --ignored --nocapture"]
fn measure_archive_heavy_save_and_fixed_current_read() {
    for archives in [10_u128, 30, 60] {
        let (_root, provider) = setup();
        let writer = provider.continuous_writer().unwrap();
        writer.commit(feedback_request()).unwrap();
        for i in 0..archives {
            let current = writer
                .load_current(ReviewStreamId::from_u128(2))
                .unwrap()
                .unwrap();
            let archived = archive_request(&current, &current, 1000 + i * 2);
            let checkpoint = archived.archives[0].clone();
            writer.commit(archived).unwrap();
            let after = writer
                .load_current(ReviewStreamId::from_u128(2))
                .unwrap()
                .unwrap();
            let historical_key = key(&current.state.feedback[0], 0);
            let decisions = vec![RestoreDecision {
                historical_key,
                choice: RestoreChoice::UseHistorical,
            }];
            let (state, _) = apply_restore(
                &after.state,
                &checkpoint,
                std::slice::from_ref(&current.state),
                &decisions,
                ReviewSnapshotId::from_u128(1001 + i * 2),
            )
            .unwrap();
            let mut restore = request(1001 + i * 2, Some(after.reference));
            restore.next.state = state;
            restore.next.evidence = after.evidence;
            restore.next.changes = vec![ReviewChange {
                target_id: historical_key.target_id,
                before: None,
                after: Some(historical_key),
                kind: ReviewChangeKind::Restored,
                archive_id: Some(checkpoint.archive_id),
                historical_key: Some(historical_key),
            }];
            writer.commit(restore).unwrap();
        }
        let current = writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap();
        let read_start = Instant::now();
        for _ in 0..20 {
            assert_eq!(
                writer
                    .load_current(ReviewStreamId::from_u128(2))
                    .unwrap()
                    .unwrap()
                    .reference,
                current.reference
            );
        }
        let read_us = read_start.elapsed().as_micros() / 20;
        let old = key(&current.state.feedback[0], 0);
        let mut save = request(10000 + archives, Some(current.reference));
        save.next.state = current.state.clone();
        save.next.state.parent = None;
        save.next.state.snapshot_id = ReviewSnapshotId::from_u128(10000 + archives);
        save.next.state.feedback[0] = update_feedback_text(
            &current.state.feedback[0],
            ReviewTextRevisionId::new(),
            "量测新的原文",
        )
        .unwrap();
        save.next.evidence = current.evidence;
        save.next.changes = save.next.state.feedback[0]
            .targets
            .iter()
            .map(|t| ReviewChange {
                target_id: t.id,
                before: current.state.target_key(t.id),
                after: save.next.state.target_key(t.id),
                kind: ReviewChangeKind::Edited,
                archive_id: None,
                historical_key: None,
            })
            .collect();
        let write_start = Instant::now();
        writer.commit(save).unwrap();
        let write_ms = write_start.elapsed().as_millis();
        let head = writer
            .load_current_ref(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap();
        let coverage_start = Instant::now();
        writer
            .load_coverage(ReviewStreamId::from_u128(2), head, &[old])
            .unwrap();
        eprintln!(
            "archives={archives} snapshots={} read_current_us={read_us} save_ms={write_ms} coverage_ms={}",
            archives * 2 + 2,
            coverage_start.elapsed().as_millis()
        );
    }
}
