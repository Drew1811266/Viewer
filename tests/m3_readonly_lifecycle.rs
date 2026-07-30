use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use viewer_application::{
    ProjectAccess, ProjectProbeError, ProjectProbeOperation, ProjectProbePort,
    file_commands::{FileCommandAction, FileCommandItem, FileCommandKind},
};
use viewer_desktop::{
    dto::{CloseChoiceDto, CloseRequestOutcomeDto, CloseTargetDto, FolderWorkspaceDto},
    state::{CloseChoice, CloseCompletionAction, CloseRequestOutcome, CloseTarget, DesktopRuntime},
};
use viewer_domain::{EntityId, SessionId, file::ReviewState, search::Generation};
use viewer_infrastructure::{
    operation::journal::{BatchState, OperationJournal},
    portable::PortableProjectMetadata,
};

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

struct UnreadableProbe;

impl ProjectProbePort for UnreadableProbe {
    fn probe(&self, root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Err(ProjectProbeError::Io {
            operation: ProjectProbeOperation::ReadDirectory,
            path: root.to_path_buf(),
            message: "private operating-system detail".to_owned(),
        })
    }
}

#[test]
fn close_contract_is_exact_and_permission_opener_accepts_no_input() {
    assert_eq!(
        serde_json::from_str::<CloseChoiceDto>(r#""wait""#).unwrap(),
        CloseChoiceDto::Wait
    );
    assert_eq!(
        serde_json::from_str::<CloseChoiceDto>(r#""cancel_pending""#).unwrap(),
        CloseChoiceDto::CancelPending
    );
    assert_eq!(
        serde_json::from_str::<CloseChoiceDto>(r#""stay""#).unwrap(),
        CloseChoiceDto::Stay
    );
    assert!(serde_json::from_str::<CloseChoiceDto>(r#""force""#).is_err());
    assert_eq!(
        serde_json::from_str::<CloseTargetDto>(r#""application""#).unwrap(),
        CloseTargetDto::Application
    );
    assert_eq!(
        serde_json::to_string(&CloseRequestOutcomeDto::Closed).unwrap(),
        r#""closed""#
    );
    assert_eq!(
        serde_json::to_string(&CloseRequestOutcomeDto::Stayed).unwrap(),
        r#""stayed""#
    );

    let _fixed_opener: fn() -> std::io::Result<()> =
        viewer_platform_macos::settings::open_privacy_and_security;

    assert_eq!(
        CloseRequestOutcome::Stayed.completion_action(CloseTarget::Application),
        CloseCompletionAction::KeepOpen
    );
    assert_eq!(
        CloseRequestOutcome::Closed.completion_action(CloseTarget::Project),
        CloseCompletionAction::ShowEmptyProject
    );
    assert_eq!(
        CloseRequestOutcome::Closed.completion_action(CloseTarget::Window),
        CloseCompletionAction::HideWindow
    );
    assert_eq!(
        CloseRequestOutcome::Closed.completion_action(CloseTarget::Application),
        CloseCompletionAction::ExitApplication
    );
}

#[tokio::test]
async fn read_only_projects_browse_and_preview_without_creating_or_mutating_metadata() {
    for existing_metadata in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        fs::create_dir(project.path().join("source")).unwrap();
        fs::create_dir(project.path().join("destination")).unwrap();
        fs::write(project.path().join("source/notes.txt"), b"readonly notes").unwrap();
        if existing_metadata {
            drop(
                PortableProjectMetadata::open(project.path(), ProjectAccess::ReadWrite, 1).unwrap(),
            );
        }
        let before = tree_snapshot(project.path());
        let runtime = DesktopRuntime::new(
            cache.path().to_path_buf(),
            Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
        );
        let opened = runtime.open_project(project.path()).await.unwrap();
        runtime.wait_for_scan().await.unwrap();
        let folders = runtime.folder_tree().await.unwrap();
        let source_id = folder_id(&folders, "source");
        let destination_id = folder_id(&folders, "destination");
        let FolderWorkspaceDto::Content { other_files, .. } =
            runtime.query_folder(Some(source_id)).await.unwrap()
        else {
            panic!("read-only source should remain browsable");
        };
        let text_id = EntityId::from_str(&other_files[0].entity_id).unwrap();
        let preview = runtime.preview_text(text_id, None).await.unwrap();
        assert_eq!(preview.plain_text.as_deref(), Some("readonly notes"));

        let session_id = SessionId::from_str(&opened.session_id).unwrap();
        let generation = Generation::new(opened.generation);
        assert_eq!(
            runtime
                .set_review_state(session_id, generation, &[text_id], Some(ReviewState::Keep))
                .await
                .unwrap_err()
                .code,
            "project_read_only"
        );
        assert_eq!(
            runtime
                .execute_file_command(
                    session_id,
                    generation,
                    FileCommandKind::Copy,
                    vec![FileCommandItem {
                        entity_id: text_id,
                        action: FileCommandAction::Copy {
                            destination_folder: destination_id,
                        },
                    }],
                    Vec::new(),
                )
                .await
                .unwrap_err()
                .code,
            "project_read_only"
        );
        assert_eq!(
            runtime
                .undo_last_operation(session_id, generation)
                .await
                .unwrap_err()
                .code,
            "project_read_only"
        );
        runtime.close_project().await.unwrap();

        assert_eq!(tree_snapshot(project.path()), before);
        assert_eq!(project.path().join(".viewer").exists(), existing_metadata);
        assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
    }
}

#[tokio::test]
async fn unreadable_root_fails_before_activation_without_leaking_details() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new(cache.path().to_path_buf(), Arc::new(UnreadableProbe));

    let error = runtime.open_project(project.path()).await.unwrap_err();
    let serialized = serde_json::to_string(&error).unwrap();

    assert_eq!(error.code, "project_unreadable");
    assert!(!serialized.contains(project.path().to_str().unwrap()));
    assert!(!serialized.contains("private operating-system detail"));
    assert!(runtime.snapshot().await.is_none());
    assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn cache_cleanup_failure_is_reported_after_committing_the_closed_terminal_state() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("sentinel"), b"must survive").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    let session_cache = fs::read_dir(cache.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::remove_dir_all(&session_cache).unwrap();
    std::os::unix::fs::symlink(outside.path(), &session_cache).unwrap();

    let error = runtime.request_close(None).await.unwrap_err();
    assert_eq!(error.code, "project_closed_cache_cleanup_failed");
    assert!(runtime.snapshot().await.is_none());
    assert_eq!(
        fs::read(outside.path().join("sentinel")).unwrap(),
        b"must survive"
    );

    let next_project = tempfile::tempdir().unwrap();
    runtime.open_project(next_project.path()).await.unwrap();
    runtime.close_project().await.unwrap();
}

#[tokio::test]
async fn process_exit_fallback_closes_the_session_and_removes_its_cache() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.txt"), b"exit cleanup").unwrap();
    let runtime = DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 1);

    runtime.finalize_process_exit().await.unwrap();

    assert!(runtime.snapshot().await.is_none());
    assert_eq!(fs::read_dir(cache.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn wait_choice_finishes_the_batch_then_tears_down_the_session() {
    let fixture = ActiveCopyFixture::open().await;
    assert_eq!(
        fixture.runtime.request_close(None).await.unwrap(),
        CloseRequestOutcome::Stayed
    );
    assert_eq!(
        fixture
            .runtime
            .request_close(Some(CloseChoice::Stay))
            .await
            .unwrap(),
        CloseRequestOutcome::Stayed
    );
    assert!(fixture.runtime.snapshot().await.is_some());

    assert_eq!(
        fixture
            .runtime
            .request_close(Some(CloseChoice::Wait))
            .await
            .unwrap(),
        CloseRequestOutcome::Closed
    );

    assert!(fixture.runtime.snapshot().await.is_none());
    assert_eq!(fs::read_dir(fixture.cache.path()).unwrap().count(), 0);
    assert_eq!(
        fs::read_dir(fixture.project.path().join("destination"))
            .unwrap()
            .count(),
        ActiveCopyFixture::ITEMS
    );
    let batch = fixture.persisted_batch();
    assert_eq!(batch.state, BatchState::Completed);
    assert_eq!(batch.completed_count, ActiveCopyFixture::ITEMS as u32);
}

#[tokio::test]
async fn cancel_pending_choice_accounts_for_the_queue_then_tears_down_the_session() {
    let fixture = ActiveCopyFixture::open().await;
    assert_eq!(
        fixture.runtime.request_close(None).await.unwrap(),
        CloseRequestOutcome::Stayed
    );

    assert_eq!(
        fixture
            .runtime
            .request_close(Some(CloseChoice::CancelPending))
            .await
            .unwrap(),
        CloseRequestOutcome::Closed
    );

    assert!(fixture.runtime.snapshot().await.is_none());
    assert_eq!(fs::read_dir(fixture.cache.path()).unwrap().count(), 0);
    let copied = fs::read_dir(fixture.project.path().join("destination"))
        .unwrap()
        .count();
    assert!(copied < ActiveCopyFixture::ITEMS);
    let batch = fixture.persisted_batch();
    assert_eq!(batch.state, BatchState::Completed);
    assert_eq!(
        batch.completed_count + batch.failed_count + batch.skipped_count,
        ActiveCopyFixture::ITEMS as u32
    );
}

struct ActiveCopyFixture {
    cache: tempfile::TempDir,
    project: tempfile::TempDir,
    runtime: DesktopRuntime,
    batch_id: viewer_domain::operation::BatchId,
}

impl ActiveCopyFixture {
    const ITEMS: usize = 400;

    async fn open() -> Self {
        let cache = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        fs::create_dir(project.path().join("source")).unwrap();
        fs::create_dir(project.path().join("destination")).unwrap();
        for index in 0..Self::ITEMS {
            fs::write(
                project.path().join(format!("source/{index:04}.txt")),
                vec![b'x'; 128 * 1024],
            )
            .unwrap();
        }
        let runtime = DesktopRuntime::new(
            cache.path().to_path_buf(),
            Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        );
        let opened = runtime.open_project(project.path()).await.unwrap();
        runtime.wait_for_scan().await.unwrap();
        let folders = runtime.folder_tree().await.unwrap();
        let source_id = folder_id(&folders, "source");
        let destination_id = folder_id(&folders, "destination");
        let FolderWorkspaceDto::Content { other_files, .. } =
            runtime.query_folder(Some(source_id)).await.unwrap()
        else {
            panic!("copy source should be content");
        };
        let items = other_files
            .iter()
            .map(|file| FileCommandItem {
                entity_id: EntityId::from_str(&file.entity_id).unwrap(),
                action: FileCommandAction::Copy {
                    destination_folder: destination_id,
                },
            })
            .collect();
        let started = runtime
            .execute_file_command(
                SessionId::from_str(&opened.session_id).unwrap(),
                Generation::new(opened.generation),
                FileCommandKind::Copy,
                items,
                Vec::new(),
            )
            .await
            .unwrap();
        Self {
            cache,
            project,
            runtime,
            batch_id: started.batch_id,
        }
    }

    fn persisted_batch(&self) -> viewer_infrastructure::operation::journal::JournalBatch {
        OperationJournal::open(self.project.path().join(".viewer/metadata.sqlite"))
            .unwrap()
            .batch(self.batch_id)
            .unwrap()
            .unwrap()
    }
}

fn folder_id(folders: &[viewer_desktop::dto::FolderTreeItemDto], name: &str) -> EntityId {
    EntityId::from_str(
        &folders
            .iter()
            .find(|folder| folder.name == name)
            .unwrap_or_else(|| panic!("missing folder {name}"))
            .entity_id,
    )
    .unwrap()
}

fn tree_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, snapshot: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                visit(root, &entry, snapshot);
            } else {
                snapshot.insert(
                    entry.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry).unwrap(),
                );
            }
        }
    }
    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}
