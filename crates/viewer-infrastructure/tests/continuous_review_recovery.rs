use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use viewer_application::review_workspace::*;
use viewer_domain::{ReviewCommandId, ReviewSnapshotId, ReviewStreamId};
use viewer_infrastructure::review::{ReviewCommitFaultInjector, ReviewCommitFaultPoint};
#[path = "support/continuous_review.rs"]
mod support;
use support::*;

struct FailOnce {
    point: ReviewCommitFaultPoint,
    pending: AtomicBool,
}
impl FailOnce {
    fn at(point: ReviewCommitFaultPoint) -> Arc<Self> {
        Arc::new(Self {
            point,
            pending: AtomicBool::new(true),
        })
    }
}
impl ReviewCommitFaultInjector for FailOnce {
    fn check(&self, point: ReviewCommitFaultPoint) -> Result<(), ReviewCommitError> {
        if point == self.point && self.pending.swap(false, Ordering::SeqCst) {
            Err(ReviewCommitError::Io)
        } else {
            Ok(())
        }
    }
}

#[test]
fn lost_receipt_retry_returns_original_commit_even_after_current_has_advanced() {
    let (root, provider) = setup();
    let writer = provider
        .continuous_writer_with_faults(FailOnce::at(ReviewCommitFaultPoint::AfterIndex))
        .unwrap();
    let first = request(3, None);
    assert_eq!(
        writer.commit(first.clone()),
        Err(ReviewCommitError::OutcomeUnknown)
    );
    let saved = writer
        .load_current_ref(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    drop(writer);
    let reopened = provider.continuous_writer().unwrap();
    let later = reopened.commit(request(4, Some(saved))).unwrap();
    let retried = reopened.commit(first.clone()).unwrap();
    assert_eq!(retried.snapshot, saved);
    assert_eq!(
        reopened
            .load_current_ref(ReviewStreamId::from_u128(2))
            .unwrap(),
        Some(later.snapshot)
    );
    assert_eq!(
        fs::read_dir(root.path().join(".viewer/reviews/states"))
            .unwrap()
            .count(),
        2
    );
    let mut conflict = first;
    conflict.next.payload_digest = [9; 32];
    assert_eq!(
        reopened.commit(conflict),
        Err(ReviewCommitError::CommandConflict)
    );
}

#[test]
fn every_pre_index_failure_keeps_the_previous_head_and_retry_reuses_immutable_files() {
    for point in [
        ReviewCommitFaultPoint::AfterEvidence,
        ReviewCommitFaultPoint::AfterState,
        ReviewCommitFaultPoint::AfterArchive,
        ReviewCommitFaultPoint::BeforeIndex,
    ] {
        let (root, provider) = setup();
        let writer = provider.continuous_writer().unwrap();
        writer.commit(image_request(&root)).unwrap();
        let before = writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap();
        let old_index = fs::read(root.path().join(".viewer/reviews/index.json")).unwrap();
        let value = archive_request(&before, &before, 4);
        drop(writer);
        let writer = provider
            .continuous_writer_with_faults(FailOnce::at(point))
            .unwrap();
        assert!(writer.commit(value.clone()).is_err(), "{point:?}");
        assert_eq!(
            fs::read(root.path().join(".viewer/reviews/index.json")).unwrap(),
            old_index
        );
        assert_eq!(
            writer
                .find_command(ReviewStreamId::from_u128(2), ReviewCommandId::from_u128(4))
                .unwrap(),
            CommandLookup::Absent
        );
        drop(writer);
        let reopened = provider.continuous_writer().unwrap();
        let receipt = reopened.commit(value).unwrap();
        assert_eq!(receipt.snapshot.snapshot_id, ReviewSnapshotId::from_u128(4));
        assert_eq!(
            reopened
                .load_current(ReviewStreamId::from_u128(2))
                .unwrap()
                .unwrap()
                .state
                .feedback[0]
                .targets
                .len(),
            1
        );
        assert_eq!(
            fs::read_dir(root.path().join(".viewer/reviews/evidence"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[test]
fn missing_history_is_unavailable_not_absent_and_cannot_authorize_a_retry() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    let current = writer.commit(request(4, Some(first.snapshot))).unwrap();
    fs::remove_file(root.path().join(format!(
        ".viewer/reviews/states/{}.json",
        first.snapshot.snapshot_id
    )))
    .unwrap();
    assert_eq!(
        writer
            .find_command(ReviewStreamId::from_u128(2), ReviewCommandId::from_u128(9))
            .unwrap(),
        CommandLookup::Unavailable
    );
    assert_eq!(
        writer.commit(request(5, Some(current.snapshot))),
        Err(ReviewCommitError::LookupUnavailable)
    );
    assert_eq!(
        writer
            .load_current_ref(ReviewStreamId::from_u128(2))
            .unwrap(),
        Some(current.snapshot)
    );
}

fn draft() -> RecoveryDraft {
    RecoveryDraft {
        stream_id: ReviewStreamId::from_u128(2),
        command_id: ReviewCommandId::from_u128(3),
        expected_snapshot_id: None,
        payload_digest: [3; 32],
        editor_input: RecoveryEditorInput {
            text: "未提交：请保留原来的颜色。".into(),
            feedback_id: None,
            targets: vec![],
            history_ref: None,
        },
        failure: ReviewRecoveryFailure::CommitUnknown,
    }
}

#[test]
fn recovery_input_survives_its_durable_fault_but_never_becomes_current_feedback() {
    let (root, provider) = setup();
    let reader = provider.continuous_reader().unwrap();
    assert!(reader.load_recovery().unwrap().is_empty());
    assert!(!root.path().join(".viewer").exists());
    let writer = provider
        .continuous_writer_with_faults(FailOnce::at(ReviewCommitFaultPoint::AfterRecovery))
        .unwrap();
    let recovery = draft();
    assert_eq!(writer.save_recovery(&recovery), Err(ReviewCommitError::Io));
    drop(writer);
    let reopened = provider.continuous_writer().unwrap();
    assert_eq!(reopened.load_recovery().unwrap(), vec![recovery.clone()]);
    assert!(
        reader
            .load_current_ref(ReviewStreamId::from_u128(2))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened.resolve_recovery(recovery.command_id).unwrap(),
        CommandLookup::Absent
    );
    let receipt = reopened.commit(request(3, None)).unwrap();
    assert_eq!(
        reopened.resolve_recovery(recovery.command_id).unwrap(),
        CommandLookup::Found(receipt)
    );
    assert_eq!(reopened.load_recovery().unwrap(), vec![recovery]);
    assert!(
        reader
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .state
            .feedback
            .is_empty()
    );
}

#[test]
fn recovery_rejects_identity_reuse_unsafe_files_and_excessive_input_without_overwriting() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let original = draft();
    writer.save_recovery(&original).unwrap();
    let path = root.path().join(format!(
        ".viewer/reviews/recovery/{}.json",
        original.command_id
    ));
    let bytes = fs::read(&path).unwrap();
    let mut conflict = original.clone();
    conflict.payload_digest = [4; 32];
    assert_eq!(
        writer.save_recovery(&conflict),
        Err(ReviewCommitError::CommandConflict)
    );
    let mut oversized = original.clone();
    oversized.editor_input.text = "a".repeat(65_537);
    assert_eq!(
        writer.save_recovery(&oversized),
        Err(ReviewCommitError::LimitExceeded)
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["editorInput"]["execute"] = serde_json::json!(true);
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert_eq!(writer.load_recovery(), Err(ReviewCommitError::Integrity));
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(root.path().join("outside.json"), &path).unwrap();
    assert!(writer.save_recovery(&original).is_err());
    assert!(!root.path().join("outside.json").exists());
}

#[test]
fn active_archive_coverage_cannot_be_recorded_twice() {
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    writer.commit(archive_request(&basis, &basis, 4)).unwrap();
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        writer.commit(archive_request(&current, &basis, 5)),
        Err(ReviewCommitError::Integrity)
    );
}

#[test]
fn explicitly_restored_coverage_can_be_archived_again_without_changing_old_history() {
    use viewer_domain::review::continuous::*;
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let first = archive_request(&basis, &basis, 4);
    let checkpoint = first.archives[0].clone();
    writer.commit(first).unwrap();
    let path = root.path().join(format!(
        ".viewer/reviews/archives/{}.json",
        checkpoint.archive_id
    ));
    let bytes = fs::read(&path).unwrap();
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let selected = key(&basis.state.feedback[0], 0);
    let (state, _) = apply_restore(
        &current.state,
        &checkpoint,
        std::slice::from_ref(&basis.state),
        &[RestoreDecision {
            historical_key: selected,
            choice: RestoreChoice::UseHistorical,
        }],
        ReviewSnapshotId::from_u128(5),
    )
    .unwrap();
    let mut restore = request(5, Some(current.reference));
    restore.next.state = state;
    restore.next.evidence = current.evidence.clone();
    restore.next.changes = vec![ReviewChange {
        target_id: selected.target_id,
        before: None,
        after: Some(selected),
        kind: ReviewChangeKind::Restored,
        archive_id: Some(checkpoint.archive_id),
        historical_key: Some(selected),
    }];
    writer.commit(restore).unwrap();
    let restored = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    writer
        .commit(archive_request(&restored, &basis, 6))
        .unwrap();
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(
        writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .state
            .feedback[0]
            .targets
            .len(),
        1
    );
    assert_eq!(
        fs::read_dir(root.path().join(".viewer/reviews/archives"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn history_lookup_is_bounded_at_ten_thousand_nodes_without_hiding_valid_current() {
    use viewer_domain::{ProjectId, review::continuous::SnapshotRef};
    use viewer_infrastructure::review::v3;
    let (root, provider) = setup();
    let directory = root.path().join(".viewer/reviews");
    fs::create_dir_all(directory.join("states")).unwrap();
    let mut previous = None;
    for sequence in 3..10_003_u128 {
        let mut value = request(sequence, previous);
        value.next.state.parent = previous;
        let record: v3::ReviewStateRecord = value.next.into();
        let bytes = v3::encode_state_v3(&record).unwrap();
        let reference = SnapshotRef {
            snapshot_id: record.state.snapshot_id,
            blake3: *blake3::hash(&bytes).as_bytes(),
        };
        fs::write(
            directory.join(format!("states/{}.json", reference.snapshot_id)),
            bytes,
        )
        .unwrap();
        previous = Some(reference);
    }
    let mut index = v3::ReviewIndexV3 {
        legacy_index: None,
        project_id: ProjectId::from_u128(1),
        streams: vec![v3::ReviewStreamV3 {
            review_stream_id: ReviewStreamId::from_u128(2),
            task_id: None,
            batch_id: None,
            current_ref: previous,
            archive_refs: vec![],
            legacy_refs: vec![],
            usage_refs: vec![],
        }],
    };
    fs::write(
        directory.join("index.json"),
        v3::encode_index_v3(&index).unwrap(),
    )
    .unwrap();
    let reader = provider.continuous_reader().unwrap();
    assert!(matches!(
        reader
            .find_command(ReviewStreamId::from_u128(2), ReviewCommandId::from_u128(3))
            .unwrap(),
        CommandLookup::Found(_)
    ));
    let mut value = request(10_003, previous);
    value.next.state.parent = previous;
    let bytes = v3::encode_state_v3(&value.next.into()).unwrap();
    let current = SnapshotRef {
        snapshot_id: ReviewSnapshotId::from_u128(10_003),
        blake3: *blake3::hash(&bytes).as_bytes(),
    };
    fs::write(
        directory.join(format!("states/{}.json", current.snapshot_id)),
        bytes,
    )
    .unwrap();
    index.streams[0].current_ref = Some(current);
    fs::write(
        directory.join("index.json"),
        v3::encode_index_v3(&index).unwrap(),
    )
    .unwrap();
    assert_eq!(
        reader
            .find_command(ReviewStreamId::from_u128(2), ReviewCommandId::from_u128(3))
            .unwrap(),
        CommandLookup::Unavailable
    );
    assert_eq!(
        reader
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .reference,
        current
    );
}

#[test]
fn second_cas_preserves_an_index_changed_after_the_immutable_files_were_written() {
    struct ReplaceIndex(std::path::PathBuf, Vec<u8>);
    impl ReviewCommitFaultInjector for ReplaceIndex {
        fn check(&self, point: ReviewCommitFaultPoint) -> Result<(), ReviewCommitError> {
            if point == ReviewCommitFaultPoint::BeforeIndex {
                fs::write(&self.0, &self.1).unwrap();
            }
            Ok(())
        }
    }
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    drop(writer);
    let path = root.path().join(".viewer/reviews/index.json");
    let mut changed = fs::read(&path).unwrap();
    changed.push(b'\n');
    let writer = provider
        .continuous_writer_with_faults(Arc::new(ReplaceIndex(path.clone(), changed.clone())))
        .unwrap();
    assert_eq!(
        writer.commit(request(4, Some(first.snapshot))),
        Err(ReviewCommitError::StaleSnapshot)
    );
    assert_eq!(fs::read(path).unwrap(), changed);
    assert_eq!(
        writer
            .load_current_ref(ReviewStreamId::from_u128(2))
            .unwrap(),
        Some(first.snapshot)
    );
}

#[test]
fn real_file_write_failure_keeps_the_head_and_recovery_input() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    let mut recovery = draft();
    recovery.command_id = ReviewCommandId::from_u128(4);
    recovery.expected_snapshot_id = Some(first.snapshot.snapshot_id);
    recovery.payload_digest = [4; 32];
    writer.save_recovery(&recovery).unwrap();
    let index = root.path().join(".viewer/reviews/index.json");
    let bytes = fs::read(&index).unwrap();
    fs::create_dir(root.path().join(format!(
        ".viewer/reviews/states/{}.json",
        ReviewSnapshotId::from_u128(4)
    )))
    .unwrap();
    assert!(writer.commit(request(4, Some(first.snapshot))).is_err());
    assert_eq!(fs::read(index).unwrap(), bytes);
    assert_eq!(writer.load_recovery().unwrap(), vec![recovery]);
}

#[test]
fn concurrent_retries_of_one_command_return_one_receipt() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let other = writer.clone();
    let start = Arc::new(std::sync::Barrier::new(2));
    let other_start = start.clone();
    let child = std::thread::spawn(move || {
        other_start.wait();
        other.commit(request(3, None))
    });
    start.wait();
    let receipt = writer.commit(request(3, None)).unwrap();
    assert_eq!(child.join().unwrap().unwrap(), receipt);
    assert_eq!(
        fs::read_dir(root.path().join(".viewer/reviews/states"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn oversized_documents_fail_closed_without_allocating_the_declared_size() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    let state = root.path().join(format!(
        ".viewer/reviews/states/{}.json",
        first.snapshot.snapshot_id
    ));
    fs::OpenOptions::new()
        .write(true)
        .open(&state)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        writer
            .find_command(ReviewStreamId::from_u128(2), first.command_id)
            .unwrap(),
        CommandLookup::Unavailable
    );
    assert_eq!(
        writer.load_current(ReviewStreamId::from_u128(2)),
        Err(ReviewCommitError::LimitExceeded)
    );
    let index = root.path().join(".viewer/reviews/index.json");
    fs::OpenOptions::new()
        .write(true)
        .open(&index)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert_eq!(
        writer.load_current_ref(ReviewStreamId::from_u128(2)),
        Err(ReviewCommitError::LimitExceeded)
    );
}

#[test]
fn recovery_is_read_only_for_readers_and_rejects_wrong_protocol_and_context() {
    let (root, provider) = setup();
    let reader = provider.continuous_reader().unwrap();
    assert_eq!(
        reader.save_recovery(&draft()),
        Err(ReviewCommitError::ReadOnly)
    );
    let writer = provider.continuous_writer().unwrap();
    let mut recovery = draft();
    let feedback = feedback_request().next.state.feedback.remove(0);
    recovery.editor_input.feedback_id = Some(feedback.id);
    recovery.editor_input.targets = feedback.targets;
    writer.save_recovery(&recovery).unwrap();
    assert_eq!(reader.load_recovery().unwrap(), vec![recovery.clone()]);
    let path = root.path().join(format!(
        ".viewer/reviews/recovery/{}.json",
        recovery.command_id
    ));
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for (key, value) in [
        ("protocol", "viewer.review.state/3".to_owned()),
        (
            "projectId",
            viewer_domain::ProjectId::from_u128(999).to_string(),
        ),
        ("commandId", ReviewCommandId::from_u128(999).to_string()),
    ] {
        let mut invalid = json.clone();
        invalid[key] = value.into();
        fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert_eq!(
            reader.load_recovery(),
            Err(ReviewCommitError::Integrity),
            "{key}"
        );
    }
}
