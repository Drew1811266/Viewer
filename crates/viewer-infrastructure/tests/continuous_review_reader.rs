use serde_json::{Value, json};
use std::{fs, process::Command};
use viewer_domain::{ReviewSnapshotId, ReviewStreamId, review::continuous::*};
use viewer_infrastructure::review::{run_review_reader, v3};
#[path = "support/continuous_review.rs"]
mod support;
use support::*;

fn node_read(project: &std::path::Path, operation: &str, selector: Value) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("node").current_dir(&root)
        .env("VIEWER_REVIEW_READER", env!("CARGO_BIN_EXE_viewer-review-reader"))
        .args(["--input-type=module", "-e", r#"
            import {readCurrentReview} from './scripts/review-protocol/read-current.mjs';
            import {readReviewHistory} from './scripts/review-protocol/read-history.mjs';
            const [projectRoot, operation, selector] = process.argv.slice(1);
            const result = await (operation === 'current' ? readCurrentReview : readReviewHistory)({projectRoot, ...JSON.parse(selector)});
            process.stdout.write(JSON.stringify(result));
        "#]).arg(project).arg(operation).arg(selector.to_string()).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    v3::decode_read_result_v3(&output.stdout).unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn rust_writer_to_node_reader_preserves_partial_archive_and_source_change_separation() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let initial = writer.commit(image_request(&root)).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let current = node_read(root.path(), "current", json!({}));
    assert_eq!(current["actionable"].as_array().unwrap().len(), 2);
    assert_eq!(current["feedback"][0]["text"], "保留原材质，收紧袖口。");
    let archive = archive_request(&basis, &basis, 4);
    let archive_id = archive.archives[0].archive_id;
    let archived = writer.commit(archive).unwrap();
    let current = node_read(
        root.path(),
        "current",
        json!({"sinceSnapshotId":initial.snapshot.snapshot_id}),
    );
    assert_eq!(current["actionable"].as_array().unwrap().len(), 1);
    assert_eq!(current["delta"]["targets"][0]["removalReason"], "archived");
    let history = node_read(
        root.path(),
        "history",
        json!({"reviewStreamId":ReviewStreamId::from_u128(2), "archiveId":archive_id}),
    );
    assert_eq!(
        history["entries"][0]["selectedTargets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(history.get("actionable").is_none());
    fs::write(root.path().join("prepared.png"), b"replacement").unwrap();
    let changed = node_read(
        root.path(),
        "current",
        json!({"sinceSnapshotId":archived.snapshot.snapshot_id}),
    );
    assert_eq!(
        changed["snapshotRef"]["snapshotId"],
        json!(archived.snapshot.snapshot_id)
    );
    assert_eq!(changed["actionable"], json!([]));
    assert_eq!(changed["needsConfirmation"].as_array().unwrap().len(), 1);
    assert_eq!(changed["delta"]["targets"], json!([]));
}

#[test]
fn net_delta_omits_reverted_prose_and_distinguishes_withdrawal_from_archival() {
    use viewer_domain::ReviewTextRevisionId;
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let initial = writer.commit(image_request(&root)).unwrap();
    for (sequence, text) in [(4, "收紧袖口，不要改颜色。"), (5, "保留原材质，收紧袖口。")]
    {
        let current = writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap();
        let old = &current.state.feedback[0];
        let edited =
            update_feedback_text(old, ReviewTextRevisionId::from_u128(sequence + 200), text)
                .unwrap();
        let mut next = request(sequence, Some(current.reference));
        next.next.state = current.state.clone();
        next.next.state.snapshot_id = ReviewSnapshotId::from_u128(sequence);
        next.next.state.parent = Some(current.reference);
        next.next.changes = old
            .targets
            .iter()
            .enumerate()
            .map(|(i, t)| ReviewChange {
                target_id: t.id,
                before: Some(key(old, i)),
                after: Some(key(&edited, i)),
                kind: ReviewChangeKind::Edited,
                archive_id: None,
                historical_key: None,
            })
            .collect();
        next.next.state.feedback[0] = edited;
        next.next.evidence = current.evidence;
        writer.commit(next).unwrap();
    }
    let reverted = node_read(
        root.path(),
        "current",
        json!({"sinceSnapshotId":initial.snapshot.snapshot_id}),
    );
    assert_eq!(reverted["delta"]["targets"], json!([]));
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let target = current.state.feedback[0].targets[0].id;
    let mut next = request(6, Some(current.reference));
    next.next.state =
        withdraw_targets(&current.state, &[target], ReviewSnapshotId::from_u128(6)).unwrap();
    next.next.changes = vec![ReviewChange {
        target_id: target,
        before: current.state.target_key(target),
        after: None,
        kind: ReviewChangeKind::Withdrawn,
        archive_id: None,
        historical_key: None,
    }];
    next.next.evidence = current.evidence;
    writer.commit(next).unwrap();
    let withdrawn = node_read(
        root.path(),
        "current",
        json!({"sinceSnapshotId":initial.snapshot.snapshot_id}),
    );
    assert_eq!(withdrawn["delta"]["targets"].as_array().unwrap().len(), 1);
    assert_eq!(
        withdrawn["delta"]["targets"][0]["removalReason"],
        "withdrawn"
    );
    assert!(withdrawn["delta"]["targets"][0].get("text").is_none());
}

fn core_read(root: &std::path::Path, fields: Value) -> Value {
    let mut request = json!({"protocolVersion":"viewer.review.reader/1", "operation":"current", "projectRoot":root});
    request
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    let mut output = vec![];
    run_review_reader(request.to_string().as_bytes(), &mut output);
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn exact_history_limit_returns_current_with_unavailable_delta_and_never_an_old_head() {
    use viewer_domain::{ProjectId, ReviewCommandId};
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join(".viewer/reviews");
    fs::create_dir_all(repository.join("states")).unwrap();
    let mut parent = None;
    let mut first = None;
    for i in 1..=10_001 {
        let mut state = ContinuousReviewState::empty(
            ProjectId::from_u128(1),
            ReviewStreamId::from_u128(2),
            ReviewSnapshotId::from_u128(i),
        );
        state.parent = parent;
        let record = v3::ReviewStateRecord {
            state,
            command_id: ReviewCommandId::from_u128(i),
            payload_digest: [0; 32],
            changes: vec![],
            evidence: vec![],
        };
        let bytes = v3::encode_state_v3(&record).unwrap();
        fs::write(
            repository.join(format!("states/{}.json", record.state.snapshot_id)),
            &bytes,
        )
        .unwrap();
        parent = Some(SnapshotRef {
            snapshot_id: record.state.snapshot_id,
            blake3: *blake3::hash(&bytes).as_bytes(),
        });
        if first.is_none() {
            first = parent;
        }
    }
    let index = v3::ReviewIndexV3 {
        project_id: ProjectId::from_u128(1),
        legacy_index: None,
        streams: vec![v3::ReviewStreamV3 {
            review_stream_id: ReviewStreamId::from_u128(2),
            task_id: None,
            batch_id: None,
            current_ref: parent,
            archive_refs: vec![],
            legacy_refs: vec![],
            usage_refs: vec![],
        }],
    };
    fs::write(
        repository.join("index.json"),
        v3::encode_index_v3(&index).unwrap(),
    )
    .unwrap();
    let within = core_read(
        temp.path(),
        json!({"sinceSnapshotId":ReviewSnapshotId::from_u128(2)}),
    );
    assert_eq!(within["result"]["delta"]["status"], "available");
    let beyond = core_read(
        temp.path(),
        json!({"sinceSnapshotId":first.unwrap().snapshot_id}),
    );
    assert_eq!(beyond["result"]["status"], "ok");
    assert_eq!(
        beyond["result"]["snapshotRef"]["snapshotId"],
        json!(parent.unwrap().snapshot_id)
    );
    assert_eq!(
        beyond["result"]["delta"],
        json!({"status":"unavailable", "reason":"limit_exceeded"})
    );
}
