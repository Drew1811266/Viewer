use serde_json::{Value, json};
use std::{fs, process::Command};
use viewer_application::{ProjectAccess, review_workspace::*};
use viewer_domain::{ReviewSnapshotId, ReviewStreamId, review::continuous::*};
use viewer_infrastructure::{
    portable::PortableProjectMetadata,
    review::{ProjectReviewRepositoryProvider, run_review_reader, v3},
};
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

fn core_operation(root: &std::path::Path, operation: &str, fields: Value) -> Value {
    let mut request = json!({"protocolVersion":"viewer.review.reader/1", "operation":operation, "projectRoot":root});
    request
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    let mut output = vec![];
    run_review_reader(request.to_string().as_bytes(), &mut output);
    serde_json::from_slice(&output).unwrap()
}

fn core_read(root: &std::path::Path, fields: Value) -> Value {
    core_operation(root, "current", fields)
}

fn published_project_with_metadata() -> (
    tempfile::TempDir,
    ProjectReviewRepositoryProvider,
    ReviewStreamId,
    ReviewSnapshotId,
) {
    let root = tempfile::tempdir().unwrap();
    let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let project_id = metadata.project_id();
    drop(metadata);
    let stream = ReviewStreamId::from_u128(2);
    let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);
    let mut initial = image_request(&root);
    initial.next.state.project_id = project_id;
    let published = initial.next.state.snapshot_id;
    provider
        .continuous_writer()
        .unwrap()
        .commit(initial)
        .unwrap();
    (root, provider, stream, published)
}

fn published_then_authored_project() -> (tempfile::TempDir, ReviewStreamId, ReviewSnapshotId) {
    let (root, provider, stream, published) = published_project_with_metadata();
    provider.bootstrap_authoring(stream).unwrap();
    advance_authoring(&provider, stream);
    (root, stream, published)
}

fn advance_authoring(provider: &ProjectReviewRepositoryProvider, stream: ReviewStreamId) {
    let authoring = provider.authoring_writer().unwrap();
    let current = authoring.load_current(stream).unwrap().unwrap();
    let mut next = current.clone();
    next.head = ReviewAuthoringHead {
        sequence: current.head.sequence + 1,
        snapshot_id: ReviewSnapshotId::from_u128(90),
    };
    next.state.snapshot_id = next.head.snapshot_id;
    next.state.parent = Some(SnapshotRef {
        snapshot_id: current.head.snapshot_id,
        blake3: current.payload_digest,
    });
    next.command_id = viewer_domain::ReviewCommandId::from_u128(91);
    next.payload_digest = [92; 32];
    next.generated.snapshot_id = next.head.snapshot_id;
    next.generated.created_at_ms += 1;
    next.changes.clear();
    next.barrier = ReviewBarrierKind::None;
    authoring
        .commit(stream, next.command_id, next.payload_digest, &mut |_| {
            Ok(ReviewAuthoringCommitRequest {
                expected_snapshot_id: Some(current.head.snapshot_id),
                next: next.clone(),
            })
        })
        .unwrap();
}

#[test]
fn current_and_list_return_no_payload_when_authoring_is_ahead() {
    let (root, _, _) = published_then_authored_project();

    for operation in ["current", "list"] {
        let value = core_operation(root.path(), operation, json!({}));
        assert_eq!(value["error"]["code"], "publication_pending");
        assert!(value.get("result").is_none());
    }
    let node = node_read(root.path(), "current", json!({}));
    assert_eq!(node["code"], "publication_pending");
    assert!(node.get("actionable").is_none());
}

#[test]
fn exact_published_history_remains_readable_while_current_is_pending() {
    let (root, stream, published) = published_then_authored_project();

    let value = core_operation(
        root.path(),
        "history",
        json!({"reviewStreamId":stream, "snapshotId":published}),
    );

    assert_eq!(value["result"]["status"], "ok");
    assert_eq!(value["result"]["role"], "history");
}

#[test]
fn equal_heads_and_v4_metadata_without_a_control_row_preserve_current_reads() {
    let (equal_root, equal_provider, stream, published) = published_project_with_metadata();
    equal_provider.bootstrap_authoring(stream).unwrap();
    for operation in ["current", "list"] {
        let value = core_operation(equal_root.path(), operation, json!({}));
        assert_eq!(value["result"]["status"], "ok");
        if operation == "current" {
            assert_eq!(
                value["result"]["snapshotRef"]["snapshotId"],
                json!(published)
            );
        }
    }

    let (legacy_root, _, _, published) = published_project_with_metadata();
    let value = core_read(legacy_root.path(), json!({}));
    assert_eq!(value["result"]["status"], "ok");
    assert_eq!(
        value["result"]["snapshotRef"]["snapshotId"],
        json!(published)
    );
}

#[test]
fn explicit_unknown_stream_stays_typed_when_all_controlled_heads_are_ready() {
    let (root, provider, stream, _) = published_project_with_metadata();
    provider.bootstrap_authoring(stream).unwrap();

    let value = core_read(
        root.path(),
        json!({"reviewStreamId":ReviewStreamId::from_u128(999)}),
    );

    assert_eq!(value["error"]["code"], "unknown_stream");
    assert!(value.get("result").is_none());
}

#[test]
fn malformed_control_head_fails_closed_without_public_payload() {
    let (root, provider, stream, _) = published_project_with_metadata();
    provider.bootstrap_authoring(stream).unwrap();
    let database = root.path().join(".viewer/metadata.sqlite");
    let connection = rusqlite::Connection::open(database).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = OFF")
        .unwrap();
    connection
        .execute(
            "UPDATE review_authoring_streams
             SET authoring_snapshot_id = ?2
             WHERE stream_id = ?1",
            rusqlite::params![stream.to_string(), "z".repeat(36)],
        )
        .unwrap();
    drop(connection);

    let value = core_read(root.path(), json!({}));

    assert_eq!(value["error"]["code"], "integrity");
    assert!(value.get("result").is_none());
}

#[test]
fn first_authoring_revision_is_pending_even_before_a_public_stream_exists() {
    let root = tempfile::tempdir().unwrap();
    let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let project_id = metadata.project_id();
    drop(metadata);
    let stream = ReviewStreamId::from_u128(2);
    let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);
    provider.bootstrap_authoring(stream).unwrap();
    let authoring = provider.authoring_writer().unwrap();
    let snapshot_id = ReviewSnapshotId::from_u128(70);
    let target = StoredAuthoringState {
        head: ReviewAuthoringHead {
            sequence: 1,
            snapshot_id,
        },
        production: None,
        state: ContinuousReviewState::empty(project_id, stream, snapshot_id),
        command_id: viewer_domain::ReviewCommandId::from_u128(71),
        payload_digest: [72; 32],
        generated: GeneratedReviewIds {
            snapshot_id,
            feedback_id: viewer_domain::FeedbackId::from_u128(73),
            text_revision_id: viewer_domain::ReviewTextRevisionId::from_u128(74),
            archive_id: viewer_domain::ReviewArchiveId::from_u128(75),
            targets: vec![],
            migration: vec![],
            created_at_ms: 2,
        },
        changes: vec![],
        archives: vec![],
        adopted_usage: vec![],
        barrier: ReviewBarrierKind::None,
    };
    authoring
        .commit(
            stream,
            target.command_id,
            target.payload_digest,
            &mut |_| {
                Ok(ReviewAuthoringCommitRequest {
                    expected_snapshot_id: None,
                    next: target.clone(),
                })
            },
        )
        .unwrap();

    for (operation, fields) in [
        ("current", json!({"reviewStreamId":stream})),
        ("list", json!({})),
    ] {
        let value = core_operation(root.path(), operation, fields);
        assert_eq!(value["error"]["code"], "publication_pending");
        assert!(value.get("result").is_none());
    }

    let unknown = core_read(
        root.path(),
        json!({"reviewStreamId":ReviewStreamId::from_u128(999)}),
    );
    assert_eq!(unknown["error"]["code"], "unknown_stream");
    assert!(unknown.get("result").is_none());
}

#[test]
fn corrupt_control_database_is_integrity_not_transient_io() {
    let (root, _, _, _) = published_project_with_metadata();
    fs::write(root.path().join(".viewer/metadata.sqlite"), b"not sqlite").unwrap();

    let value = core_read(root.path(), json!({}));

    assert_eq!(value["error"]["code"], "integrity");
    assert!(value.get("result").is_none());
}

#[test]
fn bootstrapped_empty_stream_keeps_empty_current_and_list_compatible() {
    let root = tempfile::tempdir().unwrap();
    let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let project_id = metadata.project_id();
    drop(metadata);
    let stream = ReviewStreamId::from_u128(2);
    let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);
    provider.bootstrap_authoring(stream).unwrap();

    let current = core_read(root.path(), json!({}));
    assert_eq!(current["result"]["status"], "no_review_state");
    let list = core_operation(root.path(), "list", json!({}));
    assert_eq!(list["result"]["status"], "ok");
    assert_eq!(list["result"]["streams"], json!([]));
}

#[test]
fn production_selector_observes_the_same_pending_gate() {
    use viewer_domain::review::{ProductionId, ProductionScope};

    let root = tempfile::tempdir().unwrap();
    let metadata = PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
    let project_id = metadata.project_id();
    drop(metadata);
    let stream = ReviewStreamId::from_u128(2);
    let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);
    let production = ProductionScope {
        task_id: ProductionId::parse("task-7").unwrap(),
        batch_id: ProductionId::parse("batch-9").unwrap(),
    };
    let mut initial = image_request(&root);
    initial.production = Some(production.clone());
    initial.next.state.project_id = project_id;
    provider
        .continuous_writer()
        .unwrap()
        .commit(initial)
        .unwrap();
    provider.bootstrap_authoring(stream).unwrap();
    advance_authoring(&provider, stream);

    let value = core_read(
        root.path(),
        json!({"taskId":production.task_id.as_str(), "batchId":production.batch_id.as_str()}),
    );

    assert_eq!(value["error"]["code"], "publication_pending");
    assert!(value.get("result").is_none());
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
