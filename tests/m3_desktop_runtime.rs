use std::{
    fs,
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};
use viewer_application::{
    FileMutationPort, ProjectAccess, ProjectProbeError, ProjectProbePort,
    file_commands::{
        FileCommand, FileCommandAction, FileCommandItem, FileCommandKind, FileCommandPreflightState,
    },
    watcher::ReconcileSummary,
};
use viewer_desktop::{
    dto::{
        ExecuteFileCommandRequestDto, FileCommandActionRequestDto, FileCommandItemRequestDto,
        FolderWorkspaceDto, PreflightFileCommandRequestDto, PreviewRenameRequestDto,
    },
    state::{CloseRequestOutcome, DesktopEventSink, DesktopRuntime},
};
use viewer_domain::{
    EntityId, SessionId,
    file::ReviewState,
    operation::{
        BatchLifecycle, ConflictPolicy, OperationItemPlan, OperationKind, OperationPlan,
        OperationState, RenameRuleSet,
    },
    search::Generation,
};
use viewer_infrastructure::operation::{copy::LocalFileMutation, journal::OperationJournal};

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

#[derive(Default)]
struct RecordingEvents {
    project_changes: Mutex<Vec<ReconcileSummary>>,
    close_blocked: Mutex<Vec<viewer_domain::operation::BatchId>>,
}

impl DesktopEventSink for RecordingEvents {
    fn emit_scan(&self, _event: viewer_desktop::dto::ScanEventDto) {}

    fn emit_project_changed(
        &self,
        _session_id: SessionId,
        _generation: Generation,
        summary: ReconcileSummary,
    ) {
        self.project_changes.lock().unwrap().push(summary);
    }

    fn emit_close_blocked(
        &self,
        _session_id: SessionId,
        _generation: Generation,
        batch_id: viewer_domain::operation::BatchId,
        _target: viewer_desktop::state::CloseTarget,
    ) {
        self.close_blocked.lock().unwrap().push(batch_id);
    }
}

async fn persist_verified_replace(project: &Path, item: OperationItemPlan, placed_path: &Path) {
    let scratch = project.join(".viewer-recovery-hash.tmp");
    fs::write(&scratch, []).unwrap();
    let copied = LocalFileMutation
        .copy_and_hash(placed_path, &scratch)
        .await
        .unwrap();
    fs::remove_file(scratch).unwrap();
    let journal = OperationJournal::open(project.join(".viewer/metadata.sqlite")).unwrap();
    journal
        .begin_plan(
            &OperationPlan {
                batch_id: item.batch_id,
                kind: item.kind,
                items: vec![item.clone()],
            },
            1,
        )
        .unwrap();
    let temporary_name = match item.kind {
        OperationKind::Copy => format!(".viewer-copy-{}.part", item.operation_id),
        OperationKind::Rename | OperationKind::Move => {
            format!(".viewer-replace-{}.part", item.operation_id)
        }
        other => panic!("unsupported recovery fixture kind: {other:?}"),
    };
    let temporary = viewer_domain::RelativePath::parse(&temporary_name).unwrap();
    journal
        .record_prepared_evidence(item.operation_id, Some(&temporary), copied.0, copied.1, 2)
        .unwrap();
    journal
        .advance(
            item.operation_id,
            OperationState::Prepared,
            OperationState::Staged,
            3,
        )
        .unwrap();
    journal
        .record_fs_applied(
            item.operation_id,
            OperationState::Staged,
            copied.0,
            copied.1,
            4,
        )
        .unwrap();
    journal
        .advance(
            item.operation_id,
            OperationState::FsApplied,
            OperationState::Verified,
            5,
        )
        .unwrap();
}

#[test]
fn m3_requests_are_exact_camel_case_and_never_accept_raw_paths() {
    let preview: PreviewRenameRequestDto = serde_json::from_value(serde_json::json!({
        "sessionId": "00000000-0000-0000-0000-000000000001",
        "generation": 4,
        "entityIds": ["00000000-0000-0000-0000-000000000002"],
        "rules": {
            "find": "front",
            "replacement": "hero",
            "prefix": "",
            "suffix": "-1",
            "sequence": { "start": 1, "digits": 3 }
        }
    }))
    .unwrap();
    assert_eq!(preview.generation, 4);
    assert_eq!(preview.rules.sequence.unwrap().digits, 3);

    let execute: ExecuteFileCommandRequestDto = serde_json::from_value(serde_json::json!({
        "sessionId": "00000000-0000-0000-0000-000000000001",
        "generation": 4,
        "kind": "move",
        "items": [{
            "entityId": "00000000-0000-0000-0000-000000000002",
            "action": {
                "kind": "move",
                "destinationFolderId": "00000000-0000-0000-0000-000000000003"
            }
        }],
        "conflicts": [{
            "entityId": "00000000-0000-0000-0000-000000000002",
            "policy": "keep_both",
            "applyToRemaining": true
        }]
    }))
    .unwrap();
    assert_eq!(execute.kind, FileCommandKind::Move);
    assert!(matches!(
        execute.items[0].action,
        FileCommandActionRequestDto::Move { .. }
    ));
    let preflight: PreflightFileCommandRequestDto = serde_json::from_value(serde_json::json!({
        "sessionId": "00000000-0000-0000-0000-000000000001",
        "generation": 4,
        "kind": "copy",
        "items": [{
            "entityId": "00000000-0000-0000-0000-000000000002",
            "action": {
                "kind": "copy",
                "destinationFolderId": "00000000-0000-0000-0000-000000000003"
            }
        }]
    }))
    .unwrap();
    assert_eq!(preflight.kind, FileCommandKind::Copy);

    for invalid in [
        serde_json::json!({
            "session_id": "bad-shape",
            "generation": 1,
            "kind": "trash",
            "items": [],
            "conflicts": []
        }),
        serde_json::json!({
            "sessionId": "00000000-0000-0000-0000-000000000001",
            "generation": 1,
            "kind": "trash",
            "items": [],
            "conflicts": [],
            "projectPath": "/Users/secret/project"
        }),
    ] {
        assert!(serde_json::from_value::<ExecuteFileCommandRequestDto>(invalid).is_err());
    }
}

#[tokio::test]
async fn runtime_rename_exposes_progress_results_and_a_safe_session_undo() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("front.png"), b"front bytes").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let workspace = runtime.query_folder(None).await.unwrap();
    let entity_id = match workspace {
        FolderWorkspaceDto::Content { images, .. } => {
            EntityId::from_str(&images[0].entity_id).unwrap()
        }
        other => panic!("unexpected workspace: {other:?}"),
    };
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);

    let preview = runtime
        .preview_rename(
            session_id,
            generation,
            &[entity_id],
            RenameRuleSet {
                prefix: "approved-".into(),
                ..RenameRuleSet::default()
            },
        )
        .await
        .unwrap();
    assert!(preview.executable);
    assert_eq!(preview.rows[0].proposed_name, "approved-front.png");
    let preflight = runtime
        .preflight_file_command(FileCommand {
            session_id,
            generation,
            kind: FileCommandKind::Rename,
            items: vec![FileCommandItem {
                entity_id,
                action: FileCommandAction::Rename {
                    proposed_name: "approved-front.png".into(),
                    edit_extension: true,
                },
            }],
        })
        .await
        .unwrap();
    assert!(preflight.is_executable());
    assert_eq!(preflight.rows()[0].state, FileCommandPreflightState::Ready);
    let started = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Rename,
            vec![FileCommandItem {
                entity_id,
                action: FileCommandAction::Rename {
                    proposed_name: "approved-front.png".into(),
                    edit_extension: true,
                },
            }],
            Vec::new(),
        )
        .await
        .unwrap();
    let results = runtime
        .operation_results(session_id, generation, started.batch_id, 0, 500)
        .await
        .unwrap();
    assert_eq!(results.total, 1);
    assert_eq!(results.items[0].code.as_str(), "renamed");
    let status = runtime
        .operation_status(session_id, generation, started.batch_id)
        .await
        .unwrap();
    assert_eq!(
        status.lifecycle,
        viewer_domain::operation::BatchLifecycle::Completed
    );
    assert_eq!((status.completed, status.failed), (1, 0));
    assert!(project.path().join("approved-front.png").is_file());
    assert!(!project.path().join("front.png").exists());

    let undone = runtime
        .undo_last_operation(session_id, generation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(undone.action_count, 1);
    assert!(project.path().join("front.png").is_file());
    assert!(!project.path().join("approved-front.png").exists());
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn desktop_cycle_rename_keeps_index_and_portable_markers_with_file_identity() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("a.txt"), b"content-a").unwrap();
    fs::write(project.path().join("b.txt"), b"content-b").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain text files")
    };
    let a = text_files.iter().find(|file| file.name == "a.txt").unwrap();
    let b = text_files.iter().find(|file| file.name == "b.txt").unwrap();
    let a_id = EntityId::from_str(&a.entity_id).unwrap();
    let b_id = EntityId::from_str(&b.entity_id).unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);
    runtime
        .set_review_state(session_id, generation, &[a_id], Some(ReviewState::Keep))
        .await
        .unwrap();

    let started = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Rename,
            vec![
                FileCommandItem {
                    entity_id: a_id,
                    action: FileCommandAction::Rename {
                        proposed_name: "b.txt".into(),
                        edit_extension: true,
                    },
                },
                FileCommandItem {
                    entity_id: b_id,
                    action: FileCommandAction::Rename {
                        proposed_name: "a.txt".into(),
                        edit_extension: true,
                    },
                },
            ],
            Vec::new(),
        )
        .await
        .unwrap();
    runtime.wait_for_operation(started.batch_id).await.unwrap();
    let results = runtime
        .operation_results(session_id, generation, started.batch_id, 0, 10)
        .await
        .unwrap();
    assert_eq!(
        results
            .items
            .iter()
            .filter(|item| item.status == viewer_domain::operation::BatchItemStatus::Completed)
            .count(),
        2
    );
    assert_eq!(
        fs::read(project.path().join("a.txt")).unwrap(),
        b"content-b"
    );
    assert_eq!(
        fs::read(project.path().join("b.txt")).unwrap(),
        b"content-a"
    );
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should remain a content folder")
    };
    let moved_a = text_files
        .iter()
        .find(|file| file.entity_id == a_id.to_string())
        .unwrap();
    assert_eq!(moved_a.name, "b.txt");
    assert_eq!(moved_a.marker.review_state, Some(ReviewState::Keep));
    runtime.close_project().await.unwrap();
}

#[test]
fn request_items_are_entity_only_types() {
    let item = FileCommandItemRequestDto {
        entity_id: "00000000-0000-0000-0000-000000000001".into(),
        action: FileCommandActionRequestDto::Trash,
    };
    let serialized = serde_json::to_string(&item).unwrap();
    assert!(!serialized.contains("path"));
    assert!(!serialized.contains("/Users"));
}

#[tokio::test]
async fn read_only_and_stale_sessions_reject_every_write_entry_point() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.txt"), b"read only").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should be a content folder")
    };
    let entity_id = EntityId::from_str(&text_files[0].entity_id).unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);

    let preview_error = runtime
        .preview_rename(
            session_id,
            generation,
            &[entity_id],
            RenameRuleSet::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(preview_error.code, "project_read_only");
    let execute_error = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Trash,
            vec![FileCommandItem {
                entity_id,
                action: FileCommandAction::Trash,
            }],
            Vec::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(execute_error.code, "project_read_only");

    let stale_error = runtime
        .preview_rename(
            SessionId::new(),
            generation,
            &[entity_id],
            RenameRuleSet::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(stale_error.code, "stale_project_session");
    runtime.close_project().await.unwrap();
    assert!(!project.path().join(".viewer").exists());
}

#[tokio::test]
async fn external_changes_reconcile_after_scan_and_watcher_stops_before_close_returns() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("first.txt"), b"first").unwrap();
    let events = Arc::new(RecordingEvents::default());
    let event_port: Arc<dyn DesktopEventSink> = events.clone();
    let runtime = DesktopRuntime::new_with_dependencies(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(viewer_infrastructure::scan::walker::ProjectWalker),
        event_port,
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    fs::write(project.path().join("external.txt"), b"external").unwrap();
    let mut reconciled = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let FolderWorkspaceDto::Content { text_files, .. } =
            runtime.query_folder(None).await.unwrap()
            && text_files.iter().any(|file| file.name == "external.txt")
        {
            reconciled = true;
            break;
        }
    }
    assert!(reconciled, "watcher did not publish the external file");
    assert!(!events.project_changes.lock().unwrap().is_empty());

    runtime.close_project().await.unwrap();
    let event_count = events.project_changes.lock().unwrap().len();
    fs::write(project.path().join("after-close.txt"), b"closed").unwrap();
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(events.project_changes.lock().unwrap().len(), event_count);
}

#[tokio::test]
async fn open_recovers_incomplete_journal_before_publishing_the_session() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("destination")).unwrap();
    fs::write(project.path().join("stuck.txt"), b"recover me").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.close_project().await.unwrap();

    let batch_id = viewer_domain::OperationId::new();
    let operation_id = viewer_domain::OperationId::new();
    let source = viewer_domain::RelativePath::parse("stuck.txt").unwrap();
    let destination = viewer_domain::RelativePath::parse("destination/stuck.txt").unwrap();
    let item = OperationItemPlan {
        batch_id,
        operation_id,
        entity_id: EntityId::new(),
        kind: OperationKind::Copy,
        source,
        destination: Some(destination.clone()),
        conflict_policy: ConflictPolicy::Skip,
    };
    let journal = OperationJournal::open(project.path().join(".viewer/metadata.sqlite")).unwrap();
    journal
        .begin_plan(
            &OperationPlan {
                batch_id,
                kind: OperationKind::Copy,
                items: vec![item],
            },
            1,
        )
        .unwrap();
    journal
        .register_temporary(
            operation_id,
            OperationState::Prepared,
            &viewer_domain::RelativePath::parse(&format!(
                "destination/.viewer-copy-{operation_id}.part"
            ))
            .unwrap(),
            2,
        )
        .unwrap();
    drop(journal);

    let reopened = runtime.open_project(project.path()).await.unwrap();
    let report = reopened
        .recovery_report
        .expect("incomplete operation should be reported");
    assert_eq!((report.recovered, report.needs_user_review), (1, 0));
    let serialized = serde_json::to_string(&reopened).unwrap();
    assert!(!serialized.contains(project.path().to_str().unwrap()));
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn reopen_recovery_clears_a_replaced_destination_marker_with_an_empty_session_index() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("source.txt"), b"source bytes").unwrap();
    fs::write(project.path().join("destination.txt"), b"old destination").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain text files")
    };
    let source_id = EntityId::from_str(
        &text_files
            .iter()
            .find(|file| file.name == "source.txt")
            .unwrap()
            .entity_id,
    )
    .unwrap();
    let destination_id = EntityId::from_str(
        &text_files
            .iter()
            .find(|file| file.name == "destination.txt")
            .unwrap()
            .entity_id,
    )
    .unwrap();
    runtime
        .set_review_state(
            SessionId::from_str(&opened.session_id).unwrap(),
            Generation::new(opened.generation),
            &[destination_id],
            Some(ReviewState::Reject),
        )
        .await
        .unwrap();
    runtime.close_project().await.unwrap();

    fs::write(project.path().join("destination.txt"), b"source bytes").unwrap();
    let batch_id = viewer_domain::OperationId::new();
    let item = OperationItemPlan {
        batch_id,
        operation_id: viewer_domain::OperationId::new(),
        entity_id: source_id,
        kind: OperationKind::Copy,
        source: viewer_domain::RelativePath::parse("source.txt").unwrap(),
        destination: Some(viewer_domain::RelativePath::parse("destination.txt").unwrap()),
        conflict_policy: ConflictPolicy::Replace,
    };
    persist_verified_replace(
        project.path(),
        item,
        &project.path().join("destination.txt"),
    )
    .await;

    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain recovered text files")
    };
    let destination = text_files
        .iter()
        .find(|file| file.name == "destination.txt")
        .unwrap();
    assert_eq!(destination.marker.review_state, None);
    assert!(!destination.marker.favorite);
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn reopen_recovery_replaces_the_old_marker_before_moving_the_source_marker() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("source.txt"), b"source bytes").unwrap();
    fs::write(project.path().join("destination.txt"), b"old destination").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain text files")
    };
    let source_id = EntityId::from_str(
        &text_files
            .iter()
            .find(|file| file.name == "source.txt")
            .unwrap()
            .entity_id,
    )
    .unwrap();
    let destination_id = EntityId::from_str(
        &text_files
            .iter()
            .find(|file| file.name == "destination.txt")
            .unwrap()
            .entity_id,
    )
    .unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);
    runtime
        .set_review_state(
            session_id,
            generation,
            &[source_id],
            Some(ReviewState::Keep),
        )
        .await
        .unwrap();
    runtime
        .set_review_state(
            session_id,
            generation,
            &[destination_id],
            Some(ReviewState::Reject),
        )
        .await
        .unwrap();
    runtime.close_project().await.unwrap();

    fs::remove_file(project.path().join("destination.txt")).unwrap();
    fs::rename(
        project.path().join("source.txt"),
        project.path().join("destination.txt"),
    )
    .unwrap();
    let batch_id = viewer_domain::OperationId::new();
    let item = OperationItemPlan {
        batch_id,
        operation_id: viewer_domain::OperationId::new(),
        entity_id: source_id,
        kind: OperationKind::Rename,
        source: viewer_domain::RelativePath::parse("source.txt").unwrap(),
        destination: Some(viewer_domain::RelativePath::parse("destination.txt").unwrap()),
        conflict_policy: ConflictPolicy::Replace,
    };
    persist_verified_replace(
        project.path(),
        item,
        &project.path().join("destination.txt"),
    )
    .await;

    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the recovered rename")
    };
    assert_eq!(text_files.len(), 1);
    assert_eq!(text_files[0].name, "destination.txt");
    assert_eq!(text_files[0].marker.review_state, Some(ReviewState::Keep));
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn operation_runtime_rejects_invalid_target_sets_actions_and_destination_ids() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("a.txt"), b"a").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } = runtime.query_folder(None).await.unwrap()
    else {
        panic!("root should contain text")
    };
    let entity_id = EntityId::from_str(&text_files[0].entity_id).unwrap();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);

    let empty = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Trash,
            Vec::new(),
            Vec::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(empty.code, "invalid_operation_targets");
    let duplicate = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Trash,
            vec![
                FileCommandItem {
                    entity_id,
                    action: FileCommandAction::Trash,
                },
                FileCommandItem {
                    entity_id,
                    action: FileCommandAction::Trash,
                },
            ],
            Vec::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(duplicate.code, "invalid_operation_targets");
    let mismatched = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Move,
            vec![FileCommandItem {
                entity_id,
                action: FileCommandAction::Trash,
            }],
            Vec::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(mismatched.code, "invalid_operation_targets");
    let unknown_destination = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Move,
            vec![FileCommandItem {
                entity_id,
                action: FileCommandAction::Move {
                    destination_folder: EntityId::new(),
                },
            }],
            Vec::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(unknown_destination.code, "operation_conflict_unresolved");
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn one_batch_is_active_results_are_bounded_and_close_requires_a_choice() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("source")).unwrap();
    fs::create_dir(project.path().join("destination")).unwrap();
    for index in 0..400 {
        fs::write(
            project.path().join(format!("source/{index:04}.txt")),
            vec![b'x'; 128 * 1024],
        )
        .unwrap();
    }
    let events = Arc::new(RecordingEvents::default());
    let event_port: Arc<dyn DesktopEventSink> = events.clone();
    let runtime = DesktopRuntime::new_with_dependencies(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(viewer_infrastructure::scan::walker::ProjectWalker),
        event_port,
    );
    let opened = runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let folders = runtime.folder_tree().await.unwrap();
    let source_id = EntityId::from_str(
        &folders
            .iter()
            .find(|folder| folder.name == "source")
            .unwrap()
            .entity_id,
    )
    .unwrap();
    let destination_id = EntityId::from_str(
        &folders
            .iter()
            .find(|folder| folder.name == "destination")
            .unwrap()
            .entity_id,
    )
    .unwrap();
    let FolderWorkspaceDto::Content { text_files, .. } =
        runtime.query_folder(Some(source_id)).await.unwrap()
    else {
        panic!("source should be a content folder")
    };
    let items = text_files
        .iter()
        .map(|file| FileCommandItem {
            entity_id: EntityId::from_str(&file.entity_id).unwrap(),
            action: FileCommandAction::Copy {
                destination_folder: destination_id,
            },
        })
        .collect::<Vec<_>>();
    let session_id = SessionId::from_str(&opened.session_id).unwrap();
    let generation = Generation::new(opened.generation);
    let started = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Copy,
            items.clone(),
            Vec::new(),
        )
        .await
        .unwrap();
    let concurrent = runtime
        .execute_file_command(
            session_id,
            generation,
            FileCommandKind::Copy,
            vec![items[0].clone()],
            Vec::new(),
        )
        .await
        .unwrap_err();
    assert_eq!(concurrent.code, "operation_batch_active");
    assert_eq!(
        runtime.request_close(None).await.unwrap(),
        CloseRequestOutcome::Stayed
    );
    assert_eq!(
        events.close_blocked.lock().unwrap().as_slice(),
        &[started.batch_id]
    );
    assert!(runtime.snapshot().await.is_some());

    assert!(
        runtime
            .cancel_operation(session_id, generation, started.batch_id)
            .await
            .unwrap()
    );
    runtime.wait_for_operation(started.batch_id).await.unwrap();
    let status = runtime
        .operation_status(session_id, generation, started.batch_id)
        .await
        .unwrap();
    assert_eq!(status.lifecycle, BatchLifecycle::Completed);
    assert_eq!(status.processed(), status.requested);
    assert!(status.cancelled > 0);
    let results = runtime
        .operation_results(session_id, generation, started.batch_id, 0, 500)
        .await
        .unwrap();
    assert_eq!(results.total, 400);
    assert_eq!(results.items.len(), 200);
    for (result, requested) in results.items.iter().take(10).zip(&items) {
        let requested_path = text_files
            .iter()
            .find(|file| file.entity_id == requested.entity_id.to_string())
            .unwrap()
            .relative_path
            .as_str();
        assert_eq!(result.relative_path.as_str(), requested_path);
    }
    runtime.close_project().await.unwrap();
}
