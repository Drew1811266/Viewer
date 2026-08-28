use std::{fs, sync::Arc};
use tempfile::TempDir;
use viewer_application::review_workspace::*;
use viewer_application::{ProjectAccess, ReviewRepositoryProviderPort};
use viewer_domain::review::continuous::*;
use viewer_domain::review::continuous::{ContinuousReviewState, SnapshotRef};
use viewer_domain::review::{AssetEvidence, AssetVersion, ReviewMedia};
use viewer_domain::{
    AssetVersionId, FeedbackId, RelativePath, ReviewArchiveId, ReviewTargetId,
    ReviewTargetRevisionId, ReviewTextRevisionId,
};
use viewer_domain::{ProjectId, ReviewCommandId, ReviewSnapshotId, ReviewStreamId};
use viewer_infrastructure::review::ProjectReviewRepositoryProvider;

fn request(sequence: u128, expected: Option<SnapshotRef>) -> ReviewCommitRequest {
    ReviewCommitRequest {
        expected,
        production: None,
        next: PreparedContinuousSnapshot {
            state: ContinuousReviewState::empty(
                ProjectId::from_u128(1),
                ReviewStreamId::from_u128(2),
                ReviewSnapshotId::from_u128(sequence),
            ),
            command_id: ReviewCommandId::from_u128(sequence),
            payload_digest: [sequence as u8; 32],
            changes: vec![],
            evidence: vec![],
        },
        archives: vec![],
        adopted_usage: vec![],
        staged_evidence: vec![],
    }
}

fn setup() -> (TempDir, ProjectReviewRepositoryProvider) {
    let root = TempDir::new().unwrap();
    let provider = ProjectReviewRepositoryProvider::new(root.path(), ProjectId::from_u128(1));
    (root, provider)
}

#[test]
fn reader_is_side_effect_free_and_observes_first_atomic_commit() {
    let (root, provider) = setup();
    let reader = provider.continuous_reader().unwrap();
    assert!(
        reader
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .is_none()
    );
    assert!(!root.path().join(".viewer").exists());
    let writer = provider.continuous_writer().unwrap();
    let receipt = writer.commit(request(3, None)).unwrap();
    let current = reader
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    assert_eq!(current.reference, receipt.snapshot);
    assert_eq!(current.state.snapshot_id, ReviewSnapshotId::from_u128(3));
    assert!(current.state.feedback.is_empty());
}

#[test]
fn stale_cas_does_not_publish_a_state_or_overwrite_current() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    let index = fs::read(root.path().join(".viewer/reviews/index.json")).unwrap();
    assert_eq!(
        writer.commit(request(
            4,
            Some(SnapshotRef {
                snapshot_id: ReviewSnapshotId::from_u128(9),
                blake3: [9; 32]
            })
        )),
        Err(ReviewCommitError::StaleSnapshot)
    );
    assert_eq!(
        fs::read(root.path().join(".viewer/reviews/index.json")).unwrap(),
        index
    );
    assert_eq!(
        writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .reference,
        first.snapshot
    );
    assert!(
        !root
            .path()
            .join(format!(
                ".viewer/reviews/states/{}.json",
                ReviewSnapshotId::from_u128(4)
            ))
            .exists()
    );
}

#[test]
fn legacy_and_continuous_writers_share_one_lease_and_read_only_access_cannot_write() {
    let (root, provider) = setup();
    fs::create_dir(root.path().join(".viewer")).unwrap();
    let legacy = ReviewRepositoryProviderPort::open_writer(&provider).unwrap();
    assert!(matches!(
        provider.continuous_writer(),
        Err(ReviewCommitError::LeaseBusy)
    ));
    drop(legacy);
    // An existing legacy index requires explicit migration, never silent replacement.
    let before = fs::read(root.path().join(".viewer/reviews/index.json")).unwrap();
    assert!(provider.continuous_writer().is_err());
    assert_eq!(
        fs::read(root.path().join(".viewer/reviews/index.json")).unwrap(),
        before
    );
    let readonly = ProjectReviewRepositoryProvider::new_with_access(
        root.path(),
        ProjectId::from_u128(1),
        ProjectAccess::ReadOnly,
    );
    assert!(matches!(
        readonly.continuous_writer(),
        Err(ReviewCommitError::ReadOnly)
    ));
}

#[test]
fn cloned_writer_serializes_cas_and_does_not_lose_another_stream() {
    use viewer_domain::review::{ProductionId, ProductionScope};
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    let mut other = request(5, None);
    other.next.state.stream_id = ReviewStreamId::from_u128(20);
    other.production = Some(ProductionScope {
        task_id: ProductionId::parse("task").unwrap(),
        batch_id: ProductionId::parse("batch").unwrap(),
    });
    writer.commit(other).unwrap();
    let cloned = Arc::clone(&writer);
    let guard = first.snapshot;
    let left = std::thread::spawn(move || cloned.commit(request(6, Some(guard))));
    let right = writer.commit(request(7, Some(guard)));
    assert_eq!(
        [left.join().unwrap(), right]
            .iter()
            .filter(|v| v.is_ok())
            .count(),
        1
    );
    assert_eq!(
        writer
            .load_current(ReviewStreamId::from_u128(20))
            .unwrap()
            .unwrap()
            .state
            .snapshot_id,
        ReviewSnapshotId::from_u128(5)
    );
}

#[test]
fn history_requires_a_reachable_reference_with_the_exact_digest() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    writer.commit(request(4, Some(first.snapshot))).unwrap();
    assert_eq!(
        writer
            .load_snapshot(ReviewStreamId::from_u128(2), &first.snapshot)
            .unwrap()
            .state
            .snapshot_id,
        first.snapshot.snapshot_id
    );
    let forged = SnapshotRef {
        blake3: [0; 32],
        ..first.snapshot
    };
    assert!(
        writer
            .load_snapshot(ReviewStreamId::from_u128(2), &forged)
            .is_err()
    );
    let original = root.path().join(format!(
        ".viewer/reviews/states/{}.json",
        first.snapshot.snapshot_id
    ));
    let orphan_id = ReviewSnapshotId::from_u128(99);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(original).unwrap()).unwrap();
    value["snapshotId"] = serde_json::json!(orphan_id.to_string());
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(
        root.path()
            .join(format!(".viewer/reviews/states/{orphan_id}.json")),
        &bytes,
    )
    .unwrap();
    assert!(
        writer
            .load_snapshot(
                ReviewStreamId::from_u128(2),
                &SnapshotRef {
                    snapshot_id: orphan_id,
                    blake3: *blake3::hash(&bytes).as_bytes()
                }
            )
            .is_err()
    );
}

#[test]
fn symlinked_repository_components_never_grant_outside_read_or_write_access() {
    use std::os::unix::fs::symlink;
    let (root, provider) = setup();
    let outside = TempDir::new().unwrap();
    symlink(outside.path(), root.path().join(".viewer")).unwrap();
    assert!(provider.continuous_reader().is_err());
    assert!(provider.continuous_writer().is_err());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn writer_rejects_directory_substitution_and_stream_scope_changes() {
    use std::os::unix::fs::symlink;
    use viewer_domain::review::{ProductionId, ProductionScope};
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    let mut wrong_scope = request(4, Some(first.snapshot));
    wrong_scope.production = Some(ProductionScope {
        task_id: ProductionId::parse("task").unwrap(),
        batch_id: ProductionId::parse("batch").unwrap(),
    });
    assert_eq!(
        writer.commit(wrong_scope),
        Err(ReviewCommitError::Integrity)
    );
    let outside = TempDir::new().unwrap();
    fs::rename(
        root.path().join(".viewer/reviews"),
        root.path().join("old-reviews"),
    )
    .unwrap();
    symlink(outside.path(), root.path().join(".viewer/reviews")).unwrap();
    assert!(writer.commit(request(5, Some(first.snapshot))).is_err());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn writer_never_overwrites_an_existing_snapshot_with_different_bytes() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    let path = root.path().join(format!(
        ".viewer/reviews/states/{}.json",
        ReviewSnapshotId::from_u128(4)
    ));
    fs::write(&path, b"other content").unwrap();
    assert_eq!(
        writer.commit(request(4, Some(first.snapshot))),
        Err(ReviewCommitError::Integrity)
    );
    assert_eq!(fs::read(&path).unwrap(), b"other content");
    assert_eq!(
        writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .reference,
        first.snapshot
    );
}

fn key(feedback: &VersionedFeedback, index: usize) -> TargetVersionKey {
    TargetVersionKey {
        feedback_id: feedback.id,
        text_revision_id: feedback.text_revision_id,
        target_id: feedback.targets[index].id,
        target_revision_id: feedback.targets[index].revision_id,
    }
}

fn feedback_request() -> ReviewCommitRequest {
    let mut value = request(3, None);
    value.next.state.assets = vec![AssetVersion {
        id: AssetVersionId::from_u128(10),
        source_entity_id: None,
        relative_path: RelativePath::parse("clip.mp4").unwrap(),
        evidence: AssetEvidence {
            size_bytes: 1,
            modified_ns: 1,
            blake3: Some([1; 32]),
        },
        media: ReviewMedia::Video {
            duration_us: Some(100),
            display_width: Some(640),
            display_height: Some(480),
        },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }];
    let feedback = VersionedFeedback {
        id: FeedbackId::from_u128(20),
        text_revision_id: ReviewTextRevisionId::from_u128(21),
        text: "保留原材质，收紧袖口。".into(),
        created_at_ms: 1,
        history_ref: None,
        targets: vec![
            VersionedTarget::asset(
                ReviewTargetId::from_u128(30),
                ReviewTargetRevisionId::from_u128(31),
                AssetVersionId::from_u128(10),
            ),
            VersionedTarget::asset(
                ReviewTargetId::from_u128(40),
                ReviewTargetRevisionId::from_u128(41),
                AssetVersionId::from_u128(10),
            ),
        ],
    };
    value.next.changes = (0..2)
        .map(|index| ReviewChange {
            target_id: feedback.targets[index].id,
            before: None,
            after: Some(key(&feedback, index)),
            kind: ReviewChangeKind::Added,
            archive_id: None,
            historical_key: None,
        })
        .collect();
    value.next.state.feedback = vec![feedback];
    value.next.evidence = vec![ReviewEvidenceBinding {
        asset_version_id: AssetVersionId::from_u128(10),
        capability: EvidenceCapability::NotImage,
    }];
    value
}

fn archive_request(
    current: &StoredContinuousSnapshot,
    basis: &StoredContinuousSnapshot,
    sequence: u128,
) -> ReviewCommitRequest {
    let selection = ArchiveSelection {
        expected_snapshot_id: current.state.snapshot_id,
        groups: vec![ArchiveGroup {
            basis: ArchiveBasis::Known {
                snapshot: basis.reference,
                source: ArchiveBasisSource::UserSelected,
            },
            targets: vec![key(&basis.state.feedback[0], 0)],
        }],
    };
    let (state, plan) = apply_archive(
        &current.state,
        std::slice::from_ref(&basis.state),
        &selection,
        &[],
        ReviewSnapshotId::from_u128(sequence),
    )
    .unwrap();
    let id = ReviewArchiveId::from_u128(sequence + 100);
    let checkpoint =
        ArchiveCheckpoint::from_plan(&current.state, current.reference, &plan, id, 2).unwrap();
    let mut value = request(sequence, Some(current.reference));
    value.next.state = state;
    value.next.changes = plan.changes(id);
    value.next.evidence = current.evidence.clone();
    value.archives = vec![checkpoint];
    value
}

#[test]
fn archive_publishes_one_exact_target_with_its_history_without_removing_shared_feedback() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let previous_bytes = fs::read(root.path().join(format!(
        ".viewer/reviews/states/{}.json",
        basis.reference.snapshot_id
    )))
    .unwrap();
    let archive = archive_request(&basis, &basis, 4);
    let checkpoint = archive.archives[0].clone();
    writer.commit(archive).unwrap();
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    assert_eq!(current.state.feedback.len(), 1);
    assert_eq!(current.state.feedback[0].targets.len(), 1);
    assert_eq!(
        current.state.feedback[0].targets[0].id,
        ReviewTargetId::from_u128(40)
    );
    assert_eq!(current.state.feedback[0].text, "保留原材质，收紧袖口。");
    assert_eq!(
        writer
            .load_archive(ReviewStreamId::from_u128(2), checkpoint.archive_id)
            .unwrap(),
        checkpoint
    );
    assert_eq!(
        fs::read(root.path().join(format!(
            ".viewer/reviews/states/{}.json",
            basis.reference.snapshot_id
        )))
        .unwrap(),
        previous_bytes
    );
}

#[test]
fn archive_of_old_basis_keeps_later_edits_and_rejects_unrelated_removals() {
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut edit = request(4, Some(basis.reference));
    edit.next.state = basis.state.clone();
    edit.next.state.snapshot_id = ReviewSnapshotId::from_u128(4);
    edit.next.state.parent = None;
    edit.next.state.feedback[0] = update_feedback_text(
        &basis.state.feedback[0],
        ReviewTextRevisionId::from_u128(22),
        "新的意见，请保留颜色。",
    )
    .unwrap();
    edit.next.evidence = basis.evidence.clone();
    writer.commit(edit).unwrap();
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let archive = archive_request(&current, &basis, 5);
    let mut malicious = archive.clone();
    malicious.next.state.feedback.clear();
    assert_eq!(writer.commit(malicious), Err(ReviewCommitError::Integrity));
    writer.commit(archive).unwrap();
    let after = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    assert_eq!(after.state.feedback[0].text, "新的意见，请保留颜色。");
    assert_eq!(after.state.feedback[0].targets.len(), 2);
}

#[test]
fn immutable_identity_is_checked_even_across_an_empty_snapshot() {
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut empty = request(4, Some(basis.reference));
    empty.next.changes = (0..2)
        .map(|index| ReviewChange {
            target_id: basis.state.feedback[0].targets[index].id,
            before: Some(key(&basis.state.feedback[0], index)),
            after: None,
            kind: ReviewChangeKind::Withdrawn,
            archive_id: None,
            historical_key: None,
        })
        .collect();
    let removed = writer.commit(empty).unwrap();
    let mut reused = feedback_request();
    reused.expected = Some(removed.snapshot);
    reused.next.state.snapshot_id = ReviewSnapshotId::from_u128(5);
    reused.next.command_id = ReviewCommandId::from_u128(5);
    reused.next.payload_digest = [5; 32];
    reused.next.state.feedback[0].text = "同一文字版本不可变成新指令".into();
    assert_eq!(writer.commit(reused), Err(ReviewCommitError::Integrity));
    assert_eq!(
        writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .reference,
        removed.snapshot
    );
}

fn image_request(root: &TempDir) -> ReviewCommitRequest {
    let mut value = feedback_request();
    let path = root.path().join("prepared.png");
    fs::copy(
        viewer_test_support::image_fixtures::image_fixture("alpha.png"),
        &path,
    )
    .unwrap();
    let bytes = fs::read(&path).unwrap();
    let digest = *blake3::hash(&bytes).as_bytes();
    let reference = EvidenceRef {
        blake3: digest,
        size_bytes: bytes.len() as u64,
        width: 640,
        height: 480,
    };
    value.next.state.assets[0].relative_path = RelativePath::parse("prepared.png").unwrap();
    value.next.state.assets[0].media = ReviewMedia::Image {
        width: Some(640),
        height: Some(480),
    };
    value.next.state.assets[0].evidence = AssetEvidence {
        size_bytes: bytes.len() as u64,
        modified_ns: 1,
        blake3: Some(digest),
    };
    value.next.evidence[0].capability = EvidenceCapability::Image {
        base: reference.clone(),
        annotated: None,
        annotations: vec![],
    };
    value.staged_evidence = vec![PreparedEvidenceFile { path, reference }];
    value
}

#[test]
fn image_evidence_is_installed_immutably_and_missing_or_corrupt_bytes_are_not_legacy_absence() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let value = image_request(&root);
    let mut missing = value.clone();
    missing.staged_evidence.clear();
    assert_eq!(writer.commit(missing), Err(ReviewCommitError::Integrity));
    let receipt = writer.commit(value.clone()).unwrap();
    fs::write(&value.staged_evidence[0].path, b"source replaced").unwrap();
    assert_eq!(
        writer
            .load_snapshot(ReviewStreamId::from_u128(2), &receipt.snapshot)
            .unwrap()
            .reference,
        receipt.snapshot
    );
    let path = root.path().join(format!(
        ".viewer/reviews/evidence/{}.png",
        blake3::Hash::from_bytes(value.staged_evidence[0].reference.blake3).to_hex()
    ));
    fs::write(path, b"corrupt repository evidence").unwrap();
    assert!(writer.load_current(ReviewStreamId::from_u128(2)).is_err());
    assert!(
        writer
            .load_snapshot(ReviewStreamId::from_u128(2), &receipt.snapshot)
            .is_err()
    );
}

#[test]
fn a_digest_matching_non_png_cannot_be_published_as_image_evidence() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let mut value = image_request(&root);
    let invalid = b"not a png";
    fs::write(&value.staged_evidence[0].path, invalid).unwrap();
    let reference = EvidenceRef {
        blake3: *blake3::hash(invalid).as_bytes(),
        size_bytes: invalid.len() as u64,
        width: 640,
        height: 480,
    };
    value.staged_evidence[0].reference = reference.clone();
    value.next.evidence[0].capability = EvidenceCapability::Image {
        base: reference,
        annotated: None,
        annotations: vec![],
    };
    assert_eq!(writer.commit(value), Err(ReviewCommitError::Integrity));
    assert!(
        writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .is_none()
    );
}

#[test]
fn adopted_usage_is_bound_to_the_exact_committed_basis_and_selected_targets() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let basis = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut value = request(4, Some(basis.reference));
    value.next.state = basis.state.clone();
    value.next.state.snapshot_id = ReviewSnapshotId::from_u128(4);
    value.next.state.parent = None;
    value.next.evidence = basis.evidence.clone();
    let id = viewer_domain::ReviewUsageId::from_u128(99);
    value.adopted_usage = vec![ReviewUsageDeclaration {
        id,
        project_id: ProjectId::from_u128(1),
        stream_id: ReviewStreamId::from_u128(2),
        basis: basis.reference,
        targets: vec![key(&basis.state.feedback[0], 0)],
        outputs: vec![],
    }];
    let mut bad = value.clone();
    bad.adopted_usage[0].basis.blake3 = [0; 32];
    assert_eq!(writer.commit(bad), Err(ReviewCommitError::Integrity));
    writer.commit(value).unwrap();
    let stored = fs::read(root.path().join(format!(".viewer/reviews/usage/{id}.json"))).unwrap();
    let usage = viewer_infrastructure::review::v3::decode_usage_v1(&stored).unwrap();
    assert_eq!(usage.basis, basis.reference);
    assert_eq!(usage.targets, vec![key(&basis.state.feedback[0], 0)]);
}

#[test]
fn an_archive_transition_cannot_remove_current_without_a_committed_archive_record() {
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut value = archive_request(&current, &current, 4);
    value.archives.clear();
    assert_eq!(writer.commit(value), Err(ReviewCommitError::Integrity));
}

#[test]
fn continued_feedback_requires_a_real_history_reference() {
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut value = request(4, Some(current.reference));
    value.next.state = current.state.clone();
    value.next.state.snapshot_id = ReviewSnapshotId::from_u128(4);
    value.next.state.parent = None;
    value.next.evidence = current.evidence.clone();
    let mut feedback = current.state.feedback[0].clone();
    feedback.id = FeedbackId::from_u128(120);
    feedback.text_revision_id = ReviewTextRevisionId::from_u128(121);
    feedback.targets.truncate(1);
    feedback.targets[0].id = ReviewTargetId::from_u128(130);
    feedback.targets[0].revision_id = ReviewTargetRevisionId::from_u128(131);
    feedback.history_ref = Some(HistoryRef {
        project_id: current.state.project_id,
        stream_id: current.state.stream_id,
        source: HistorySource::Snapshot {
            snapshot: SnapshotRef {
                snapshot_id: ReviewSnapshotId::from_u128(999),
                blake3: [0; 32],
            },
            keys: vec![key(&current.state.feedback[0], 0)],
        },
    });
    value.next.state.feedback.push(feedback);
    assert_eq!(
        writer.commit(value.clone()),
        Err(ReviewCommitError::Integrity)
    );
    if let HistorySource::Snapshot { snapshot, .. } = &mut value.next.state.feedback[1]
        .history_ref
        .as_mut()
        .unwrap()
        .source
    {
        *snapshot = current.reference;
    }
    writer.commit(value).unwrap();
}

#[test]
fn agent_declared_archive_cannot_claim_an_unadopted_usage_id() {
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    writer.commit(feedback_request()).unwrap();
    let current = writer
        .load_current(ReviewStreamId::from_u128(2))
        .unwrap()
        .unwrap();
    let mut value = archive_request(&current, &current, 4);
    let id = viewer_domain::ReviewUsageId::from_u128(99);
    if let ArchiveBasis::Known { source, .. } = &mut value.archives[0].groups[0].basis {
        *source = ArchiveBasisSource::AgentDeclared { usage_id: id };
    }
    assert_eq!(
        writer.commit(value.clone()),
        Err(ReviewCommitError::Integrity)
    );
    value.adopted_usage.push(ReviewUsageDeclaration {
        id,
        project_id: current.state.project_id,
        stream_id: current.state.stream_id,
        basis: current.reference,
        targets: vec![key(&current.state.feedback[0], 0)],
        outputs: vec![],
    });
    writer.commit(value).unwrap();
}

#[test]
fn a_replaced_write_lock_invalidates_the_old_writer() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let first = writer.commit(request(3, None)).unwrap();
    fs::rename(
        root.path().join(".viewer/reviews/write.lock"),
        root.path().join("old.lock"),
    )
    .unwrap();
    fs::write(root.path().join(".viewer/reviews/write.lock"), b"").unwrap();
    assert_eq!(
        writer.commit(request(4, Some(first.snapshot))),
        Err(ReviewCommitError::Integrity)
    );
}

#[test]
fn an_asset_version_cannot_acquire_a_different_base_image_on_a_later_snapshot() {
    let (root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let value = image_request(&root);
    writer.commit(value.clone()).unwrap();
    let mut next = value;
    next.expected = Some(
        writer
            .load_current(ReviewStreamId::from_u128(2))
            .unwrap()
            .unwrap()
            .reference,
    );
    next.next.state.snapshot_id = ReviewSnapshotId::from_u128(4);
    next.next.command_id = ReviewCommandId::from_u128(4);
    next.next.payload_digest = [4; 32];
    next.next.changes.clear();
    let mut bytes = fs::read(&next.staged_evidence[0].path).unwrap();
    bytes.extend_from_slice(b"changed base bytes");
    fs::write(&next.staged_evidence[0].path, &bytes).unwrap();
    next.staged_evidence[0].reference.blake3 = *blake3::hash(&bytes).as_bytes();
    next.staged_evidence[0].reference.size_bytes = bytes.len() as u64;
    if let EvidenceCapability::Image { base, .. } = &mut next.next.evidence[0].capability {
        *base = next.staged_evidence[0].reference.clone();
    }
    assert_eq!(writer.commit(next), Err(ReviewCommitError::Integrity));
}

#[test]
fn the_first_snapshot_cannot_claim_an_edit_without_a_predecessor() {
    let (_root, provider) = setup();
    let writer = provider.continuous_writer().unwrap();
    let mut value = feedback_request();
    let after = value.next.changes[0].after.unwrap();
    value.next.changes[0].before = Some(TargetVersionKey {
        text_revision_id: ReviewTextRevisionId::from_u128(99),
        ..after
    });
    value.next.changes[0].kind = ReviewChangeKind::Edited;
    assert_eq!(writer.commit(value), Err(ReviewCommitError::Integrity));
}
