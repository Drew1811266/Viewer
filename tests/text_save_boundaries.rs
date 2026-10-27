use std::{fs, path::Path, sync::Arc};
use viewer_application::{
    ProjectAccess, ProjectProbeError, ProjectProbePort, TextEncoding, TextNewlineStyle,
    TextPreviewPort, TextSaveError, TextSavePort, TextSaveRequest,
};
use viewer_desktop::dto::FolderWorkspaceDto;
use viewer_desktop::error::{CommandError, ErrorCategory};
use viewer_desktop::state::{DesktopEventSink, DesktopRuntime, ScanEventDto};

async fn workspace_file(runtime: &DesktopRuntime, suffix: &str) -> String {
    let workspace = runtime.query_folder(None).await.unwrap();
    let FolderWorkspaceDto::Content {
        images,
        videos,
        other_files,
        ..
    } = workspace
    else {
        panic!("expected a content workspace at the project root");
    };
    let mut all = images.iter().chain(videos.iter()).chain(other_files.iter());
    let file = all
        .find(|file| file.relative_path.ends_with(suffix))
        .unwrap_or_else(|| panic!("indexed file ending with {suffix} not found"));
    file.entity_id.clone()
}
use viewer_infrastructure::scan::walker::ProjectWalker;
use viewer_infrastructure::text::writer::AtomicTextWriter;

struct FixedProbe(ProjectAccess);

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(self.0)
    }
}

struct NullEventSink;

impl DesktopEventSink for NullEventSink {
    fn emit_scan(&self, _event: ScanEventDto) {}
}

#[test]
fn writer_saves_utf8_content_and_returns_the_new_fingerprint() {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), b"# hello\n").unwrap();
    let writer = AtomicTextWriter;

    let loaded = viewer_infrastructure::text::preview::TextPreviewReader
        .read(file.path(), None)
        .unwrap();
    let outcome = writer
        .save(TextSaveRequest {
            source: file.path(),
            content: "# hello\n## edited\n",
            expected_fingerprint: &loaded.fingerprint,
            newline_style: TextNewlineStyle::Lf,
            acknowledge_encoding_conversion: false,
        })
        .unwrap();

    assert_eq!(fs::read(file.path()).unwrap(), b"# hello\n## edited\n");
    let reloaded = viewer_infrastructure::text::preview::TextPreviewReader
        .read(file.path(), None)
        .unwrap();
    assert_eq!(outcome.fingerprint, reloaded.fingerprint);
}

#[test]
fn writer_round_trips_crlf_newline_style() {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), b"one\r\ntwo\r\n").unwrap();
    let loaded = viewer_infrastructure::text::preview::TextPreviewReader
        .read(file.path(), None)
        .unwrap();
    assert!(loaded.uses_crlf);
    assert_eq!(loaded.text, "one\ntwo\n");

    AtomicTextWriter
        .save(TextSaveRequest {
            source: file.path(),
            content: "one\ntwo\nedited\n",
            expected_fingerprint: &loaded.fingerprint,
            newline_style: TextNewlineStyle::Crlf,
            acknowledge_encoding_conversion: false,
        })
        .unwrap();

    assert_eq!(fs::read(file.path()).unwrap(), b"one\r\ntwo\r\nedited\r\n");
}

#[test]
fn writer_refuses_blind_overwrites_of_externally_modified_files() {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), b"original\n").unwrap();
    let loaded = viewer_infrastructure::text::preview::TextPreviewReader
        .read(file.path(), None)
        .unwrap();
    fs::write(file.path(), b"external change\n").unwrap();

    let result = AtomicTextWriter.save(TextSaveRequest {
        source: file.path(),
        content: "edited\n",
        expected_fingerprint: &loaded.fingerprint,
        newline_style: TextNewlineStyle::Lf,
        acknowledge_encoding_conversion: false,
    });

    assert_eq!(result.unwrap_err(), TextSaveError::Conflict);
    assert_eq!(fs::read(file.path()).unwrap(), b"external change\n");
}

#[test]
fn writer_requires_acknowledgement_before_converting_non_utf8_sources() {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), vec![0xd6, 0xd0, 0xce, 0xc4]).unwrap();
    let loaded = viewer_infrastructure::text::preview::TextPreviewReader
        .read(file.path(), None)
        .unwrap();
    assert_eq!(loaded.encoding, TextEncoding::Gb18030);

    let request = |acknowledge: bool| TextSaveRequest {
        source: file.path(),
        content: "中文",
        expected_fingerprint: &loaded.fingerprint,
        newline_style: TextNewlineStyle::Lf,
        acknowledge_encoding_conversion: acknowledge,
    };

    assert_eq!(
        AtomicTextWriter.save(request(false)).unwrap_err(),
        TextSaveError::EncodingAcknowledgementRequired
    );

    AtomicTextWriter.save(request(true)).unwrap();
    assert_eq!(fs::read(file.path()).unwrap(), "中文".as_bytes());
}

#[test]
fn writer_refuses_files_beyond_the_editable_size_limit() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let oversized = vec![b'a'; 10 * 1024 * 1024 + 1];
    fs::write(file.path(), &oversized).unwrap();
    let loaded = viewer_infrastructure::text::preview::TextPreviewReader
        .read(file.path(), None)
        .unwrap();
    assert!(loaded.truncated);

    let result = AtomicTextWriter.save(TextSaveRequest {
        source: file.path(),
        content: "small edit",
        expected_fingerprint: &loaded.fingerprint,
        newline_style: TextNewlineStyle::Lf,
        acknowledge_encoding_conversion: false,
    });

    assert_eq!(result.unwrap_err(), TextSaveError::TooLarge);
    assert_eq!(fs::read(file.path()).unwrap(), oversized);
}

#[test]
fn writer_leaves_no_temporary_files_behind() {
    let directory = tempfile::tempdir().unwrap();
    let file = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
    let directory = file.path().parent().unwrap().to_path_buf();
    fs::write(file.path(), b"one\n").unwrap();
    let loaded = viewer_infrastructure::text::preview::TextPreviewReader
        .read(file.path(), None)
        .unwrap();

    AtomicTextWriter
        .save(TextSaveRequest {
            source: file.path(),
            content: "two\n",
            expected_fingerprint: &loaded.fingerprint,
            newline_style: TextNewlineStyle::Lf,
            acknowledge_encoding_conversion: false,
        })
        .unwrap();

    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
}

#[tokio::test]
async fn save_text_command_round_trips_through_the_desktop_runtime() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.md"), b"# title\n").unwrap();

    let runtime = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(NullEventSink),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    let entity = workspace_file(&runtime, "notes.md").await;
    let preview = runtime
        .preview_text(entity_id(&entity), None)
        .await
        .unwrap();
    assert_eq!(preview.plain_text.as_deref(), Some("# title\n"));
    assert!(!preview.fingerprint.is_empty());

    let saved = runtime
        .save_text(
            entity_id(&entity),
            "# title\n\nedited\n".to_owned(),
            preview.fingerprint.clone(),
            if preview.uses_crlf {
                TextNewlineStyle::Crlf
            } else {
                TextNewlineStyle::Lf
            },
            false,
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        fs::read(project.path().join("notes.md")).unwrap(),
        b"# title\n\nedited\n"
    );

    // The atomic rename installs a fresh inode, so the result carries the
    // rekeyed entity id; the next save must address it instead of the stale
    // identity from the original load.
    assert_ne!(saved.entity_id, entity);
    runtime
        .save_text(
            entity_id(&saved.entity_id),
            "# title\n\nedited twice\n".to_owned(),
            saved.fingerprint,
            TextNewlineStyle::Lf,
            false,
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        fs::read(project.path().join("notes.md")).unwrap(),
        b"# title\n\nedited twice\n"
    );
}

#[tokio::test]
async fn save_text_refuses_read_only_projects_and_non_text_targets() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.md"), b"# title\n").unwrap();
    fs::write(project.path().join("image.png"), b"png").unwrap();

    let runtime = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(NullEventSink),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    let notes = workspace_file(&runtime, "notes.md").await;
    let image = workspace_file(&runtime, "image.png").await;

    let error = runtime
        .save_text(
            entity_id(&image),
            "not text".to_owned(),
            "0".to_owned(),
            TextNewlineStyle::Lf,
            false,
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "text_not_found");

    runtime.close_project().await.unwrap();

    let read_only = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
        Arc::new(ProjectWalker),
        Arc::new(NullEventSink),
    );
    read_only.open_project(project.path()).await.unwrap();
    read_only.wait_for_scan().await.unwrap();
    let error = read_only
        .save_text(
            entity_id(&notes),
            "edit".to_owned(),
            "0".to_owned(),
            TextNewlineStyle::Lf,
            false,
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "project_read_only");
    assert_eq!(error.category, ErrorCategory::Conflict);
}

#[tokio::test]
async fn save_text_copy_writes_fresh_siblings_without_touching_the_original() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.md"), b"# title\n").unwrap();

    let runtime = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(NullEventSink),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    let entity = workspace_file(&runtime, "notes.md").await;
    let preview = runtime
        .preview_text(entity_id(&entity), None)
        .await
        .unwrap();
    let saved = runtime
        .save_text(
            entity_id(&entity),
            "# title\n\ncopy one\n".to_owned(),
            preview.fingerprint,
            TextNewlineStyle::Lf,
            false,
            true,
        )
        .await
        .unwrap();
    assert_eq!(saved.created_path.as_deref(), Some("notes-copy.md"));
    assert_eq!(
        fs::read(project.path().join("notes.md")).unwrap(),
        b"# title\n"
    );
    assert_eq!(
        fs::read(project.path().join("notes-copy.md")).unwrap(),
        b"# title\n\ncopy one\n"
    );

    let preview = runtime
        .preview_text(entity_id(&entity), None)
        .await
        .unwrap();
    let second = runtime
        .save_text(
            entity_id(&entity),
            "# title\n\ncopy two\n".to_owned(),
            preview.fingerprint,
            TextNewlineStyle::Lf,
            false,
            true,
        )
        .await
        .unwrap();
    assert_eq!(second.created_path.as_deref(), Some("notes-copy-2.md"));
    assert_eq!(
        fs::read(project.path().join("notes-copy-2.md")).unwrap(),
        b"# title\n\ncopy two\n"
    );
    assert!(
        workspace_file(&runtime, "notes-copy-2.md")
            .await
            .ends_with(&second.entity_id)
    );
}

#[tokio::test]
async fn save_text_copy_refuses_read_only_projects() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.md"), b"# title\n").unwrap();

    let runtime = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadOnly)),
        Arc::new(ProjectWalker),
        Arc::new(NullEventSink),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    let entity = workspace_file(&runtime, "notes.md").await;
    let error = runtime
        .save_text(
            entity_id(&entity),
            "# copy\n".to_owned(),
            "0".to_owned(),
            TextNewlineStyle::Lf,
            false,
            true,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "project_read_only");
    assert!(!project.path().join("notes-copy.md").exists());
}

#[tokio::test]
async fn save_text_reports_a_conflict_when_the_file_changes_behind_the_preview() {
    let cache_base = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("notes.md"), b"# original\n").unwrap();

    let runtime = DesktopRuntime::new_with_dependencies(
        cache_base.path().to_path_buf(),
        Arc::new(FixedProbe(ProjectAccess::ReadWrite)),
        Arc::new(ProjectWalker),
        Arc::new(NullEventSink),
    );
    runtime.open_project(project.path()).await.unwrap();
    runtime.wait_for_scan().await.unwrap();

    let notes = workspace_file(&runtime, "notes.md").await;
    let preview = runtime.preview_text(entity_id(&notes), None).await.unwrap();

    fs::write(project.path().join("notes.md"), b"# external\n").unwrap();

    let error: CommandError = runtime
        .save_text(
            entity_id(&notes),
            "# edited\n".to_owned(),
            preview.fingerprint,
            TextNewlineStyle::Lf,
            false,
            false,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "text_save_conflict");
    assert_eq!(error.category, ErrorCategory::Conflict);
    assert!(!error.retryable);
    assert_eq!(
        fs::read(project.path().join("notes.md")).unwrap(),
        b"# external\n"
    );
}

fn entity_id(value: &str) -> viewer_domain::EntityId {
    value.parse().unwrap()
}
