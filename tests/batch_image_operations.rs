use std::{fs, path::Path, sync::Arc};
use viewer_application::{
    BatchImageIoPort, BatchImageService, ProjectAccess, ProjectProbeError, ProjectProbePort,
};
use viewer_desktop::error::{CommandError, ErrorCategory};
use viewer_desktop::state::{DesktopEventSink, DesktopRuntime, ScanEventDto};
use viewer_domain::batch_image::{
    BatchImageConflictStrategy, BatchImageItemOutcome, BatchImageOutput, BatchImagePlanDisposition,
    BatchImageQuality, BatchImageRejectReason, BatchImageResize, BatchImageSettings,
    BatchImageTargetFormat,
};
use viewer_infrastructure::scan::walker::ProjectWalker;

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

struct RecordingIo {
    cancelled_after: usize,
    processed: std::sync::Mutex<usize>,
}

#[async_trait::async_trait]
impl BatchImageIoPort for RecordingIo {
    async fn encode(
        &self,
        _source: &Path,
        _settings: &BatchImageSettings,
        temp: &Path,
    ) -> Result<(u32, u32, u64), viewer_application::ImageError> {
        let mut processed = self.processed.lock().unwrap();
        *processed += 1;
        if *processed > self.cancelled_after {
            return Err(viewer_application::ImageError::Corrupt);
        }
        fs::write(temp, b"encoded")
            .map_err(|error| viewer_application::ImageError::Io(format!("encode temp: {error}")))?;
        Ok((10, 8, 7))
    }
    async fn trash(&self, path: &Path) -> Result<(), String> {
        fs::remove_file(path).map_err(|error| error.to_string())
    }
    fn path_exists(&self, path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok()
    }
    fn rename(&self, from: &Path, to: &Path) -> Result<(), String> {
        fs::rename(from, to).map_err(|error| error.to_string())
    }
    fn create_directory(&self, path: &Path) -> Result<(), String> {
        fs::create_dir_all(path).map_err(|error| error.to_string())
    }
    fn available_formats(&self) -> Vec<BatchImageTargetFormat> {
        vec![
            BatchImageTargetFormat::Jpeg,
            BatchImageTargetFormat::Png,
            BatchImageTargetFormat::Heic,
            BatchImageTargetFormat::Webp,
        ]
    }
}

fn settings(conflict: BatchImageConflictStrategy, output: BatchImageOutput) -> BatchImageSettings {
    settings_format(BatchImageTargetFormat::Webp, conflict, output)
}

fn settings_format(
    format: BatchImageTargetFormat,
    conflict: BatchImageConflictStrategy,
    output: BatchImageOutput,
) -> BatchImageSettings {
    BatchImageSettings {
        format,
        quality: BatchImageQuality::DEFAULT,
        resize: BatchImageResize::None,
        output,
        conflict,
    }
}

fn write_tree(root: &Path) {
    fs::write(root.join("a.jpg"), b"jpeg-a").unwrap();
    fs::write(root.join("b.png"), b"png-b").unwrap();
    fs::write(root.join("c.txt"), b"text-c").unwrap();
}

async fn runtime_with(root: &Path, access: ProjectAccess) -> DesktopRuntime {
    let cache = tempfile::tempdir().unwrap();
    let runtime = DesktopRuntime::new_with_dependencies(
        cachekeep(cache.path()).to_path_buf(),
        Arc::new(FixedProbe(access)),
        Arc::new(ProjectWalker),
        Arc::new(NullEventSink),
    );
    std::mem::forget(cache);
    runtime.open_project(root).await.unwrap();
    runtime.wait_for_scan().await.unwrap();
    runtime
}

fn cachekeep(path: &Path) -> &Path {
    path
}

async fn entity_for(runtime: &DesktopRuntime, suffix: &str) -> String {
    let workspace = runtime.query_folder(None).await.unwrap();
    let viewer_desktop::dto::FolderWorkspaceDto::Content {
        images,
        videos,
        other_files,
        ..
    } = workspace
    else {
        panic!("expected content workspace");
    };
    let mut all = images.iter().chain(videos.iter()).chain(other_files.iter());
    all.find(|file| file.relative_path.ends_with(suffix))
        .unwrap_or_else(|| panic!("file {suffix} not indexed"))
        .entity_id
        .clone()
}

#[tokio::test]
async fn plan_reports_destinations_conflicts_and_rejections() {
    let project = tempfile::tempdir().unwrap();
    write_tree(project.path());
    let io = Arc::new(RecordingIo {
        cancelled_after: usize::MAX,
        processed: std::sync::Mutex::new(0),
    });
    let service = BatchImageService::new(io);

    let nodes = vec![
        node_for(project.path(), "a.jpg"),
        node_for(project.path(), "c.txt"),
    ];
    let plan = service.plan(
        project.path(),
        &nodes,
        &settings(
            BatchImageConflictStrategy::Skip,
            BatchImageOutput::Sibling {
                suffix: "-webp".into(),
            },
        ),
    );

    assert!(matches!(
        plan[0].disposition,
        BatchImagePlanDisposition::Convert { ref destination } if destination == "a-webp.webp"
    ));
    assert!(matches!(
        plan[1].disposition,
        BatchImagePlanDisposition::Rejected { .. }
    ));
}

#[test]
fn plan_rejects_output_names_that_escape_the_project_root() {
    let project = tempfile::tempdir().unwrap();
    write_tree(project.path());
    let io = Arc::new(RecordingIo {
        cancelled_after: usize::MAX,
        processed: std::sync::Mutex::new(0),
    });
    let service = BatchImageService::new(io);
    let nodes = vec![node_for(project.path(), "a.jpg")];

    for output in [
        BatchImageOutput::Subfolder {
            name: "../outside".into(),
        },
        BatchImageOutput::Subfolder {
            name: "/tmp/outside".into(),
        },
        BatchImageOutput::Sibling {
            suffix: "/../../outside".into(),
        },
    ] {
        let plan = service.plan(
            project.path(),
            &nodes,
            &settings(BatchImageConflictStrategy::Overwrite, output),
        );
        assert!(
            matches!(
                plan[0].disposition,
                BatchImagePlanDisposition::Rejected {
                    reason: BatchImageRejectReason::UnwritableFormat
                }
            ),
            "unsafe output must be rejected: {:?}",
            plan[0].disposition
        );
    }
}

#[tokio::test]
async fn execute_writes_new_files_and_skips_conflicts() {
    let project = tempfile::tempdir().unwrap();
    write_tree(project.path());
    fs::write(project.path().join("a-webp.webp"), b"existing").unwrap();
    let io = Arc::new(RecordingIo {
        cancelled_after: usize::MAX,
        processed: std::sync::Mutex::new(0),
    });
    let service = BatchImageService::new(io);
    let cancellation = tokio_util::sync::CancellationToken::new();
    let progress_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let progress_sink = {
        let events = Arc::clone(&progress_events);
        move |progress: viewer_application::file_commands::BatchProgress| {
            events.lock().unwrap().push(progress.lifecycle);
        }
    };

    let nodes = vec![
        node_for(project.path(), "a.jpg"),
        node_for(project.path(), "b.png"),
    ];
    let execution = service
        .execute(
            project.path(),
            &nodes,
            &settings(
                BatchImageConflictStrategy::Skip,
                BatchImageOutput::Sibling {
                    suffix: "-webp".into(),
                },
            ),
            &cancellation,
            &progress_sink,
        )
        .await;

    assert_eq!(execution.outcomes.len(), 2);
    assert!(matches!(
        execution.outcomes[0].1,
        BatchImageItemOutcome::Skipped { .. }
    ));
    assert!(matches!(
        execution.outcomes[1].1,
        BatchImageItemOutcome::Converted { ref destination, .. } if destination == "b-webp.webp"
    ));
    assert!(project.path().join("b-webp.webp").is_file());
    assert_eq!(
        fs::read(project.path().join("a-webp.webp")).unwrap(),
        b"existing"
    );
    assert!(
        progress_events
            .lock()
            .unwrap()
            .iter()
            .any(|l| matches!(l, viewer_domain::operation::BatchLifecycle::Completed))
    );
}

#[tokio::test]
async fn execute_suffix_strategy_writes_the_next_free_name() {
    let project = tempfile::tempdir().unwrap();
    write_tree(project.path());
    fs::write(project.path().join("a-webp.webp"), b"existing").unwrap();
    fs::write(project.path().join("a-webp-2.webp"), b"existing2").unwrap();
    let io = Arc::new(RecordingIo {
        cancelled_after: usize::MAX,
        processed: std::sync::Mutex::new(0),
    });
    let service = BatchImageService::new(io);
    let cancellation = tokio_util::sync::CancellationToken::new();

    let nodes = vec![node_for(project.path(), "a.jpg")];
    let execution = service
        .execute(
            project.path(),
            &nodes,
            &settings(
                BatchImageConflictStrategy::Suffix,
                BatchImageOutput::Sibling {
                    suffix: "-webp".into(),
                },
            ),
            &cancellation,
            &|_| {},
        )
        .await;

    assert!(matches!(
        execution.outcomes[0].1,
        BatchImageItemOutcome::Converted { ref destination, .. } if destination == "a-webp-3.webp"
    ));
}

#[tokio::test]
async fn execute_subfolder_output_creates_the_folder_once() {
    let project = tempfile::tempdir().unwrap();
    write_tree(project.path());
    let io = Arc::new(RecordingIo {
        cancelled_after: usize::MAX,
        processed: std::sync::Mutex::new(0),
    });
    let service = BatchImageService::new(io);
    let cancellation = tokio_util::sync::CancellationToken::new();

    let nodes = vec![node_for(project.path(), "b.png")];
    let execution = service
        .execute(
            project.path(),
            &nodes,
            &settings(
                BatchImageConflictStrategy::Skip,
                BatchImageOutput::Subfolder {
                    name: "converted".into(),
                },
            ),
            &cancellation,
            &|_| {},
        )
        .await;

    assert!(matches!(
        execution.outcomes[0].1,
        BatchImageItemOutcome::Converted { ref destination, .. } if destination == "converted/b.webp"
    ));
    assert!(project.path().join("converted/b.webp").is_file());
}

#[tokio::test]
async fn execute_reports_failures_without_stopping_the_batch() {
    let project = tempfile::tempdir().unwrap();
    write_tree(project.path());
    // Fail every encode.
    let io = Arc::new(RecordingIo {
        cancelled_after: 0,
        processed: std::sync::Mutex::new(0),
    });
    let service = BatchImageService::new(io);
    let cancellation = tokio_util::sync::CancellationToken::new();

    let nodes = vec![
        node_for(project.path(), "a.jpg"),
        node_for(project.path(), "b.png"),
    ];
    let execution = service
        .execute(
            project.path(),
            &nodes,
            &settings(
                BatchImageConflictStrategy::Skip,
                BatchImageOutput::Sibling {
                    suffix: "-webp".into(),
                },
            ),
            &cancellation,
            &|_| {},
        )
        .await;

    assert!(
        execution
            .outcomes
            .iter()
            .all(|(_, outcome)| matches!(outcome, BatchImageItemOutcome::Failed { .. }))
    );
    assert!(!project.path().join("a-webp.webp").exists());
}

#[tokio::test]
async fn desktop_commands_reject_read_only_projects() {
    let project = tempfile::tempdir().unwrap();
    write_tree(project.path());
    let runtime = runtime_with(project.path(), ProjectAccess::ReadOnly).await;

    let entity = entity_for(&runtime, "a.jpg").await;
    let error: CommandError = runtime
        .execute_batch_image(
            vec![entity.parse().unwrap()],
            settings(
                BatchImageConflictStrategy::Skip,
                BatchImageOutput::Sibling {
                    suffix: "-webp".into(),
                },
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "project_read_only");
    assert_eq!(error.category, ErrorCategory::Conflict);
}

#[tokio::test]
async fn desktop_round_trip_converts_a_real_jpeg_into_heic() {
    let project = tempfile::tempdir().unwrap();
    let fixture = viewer_test_support::image_fixtures::image_fixture("srgb.jpg");
    fs::copy(&fixture, project.path().join("photo.jpg")).unwrap();

    let runtime = runtime_with(project.path(), ProjectAccess::ReadWrite).await;
    let entity = entity_for(&runtime, "photo.jpg").await;

    let plan = runtime
        .preview_batch_image(
            vec![entity.parse().unwrap()],
            settings_format(
                BatchImageTargetFormat::Heic,
                BatchImageConflictStrategy::Skip,
                BatchImageOutput::Sibling {
                    suffix: "-converted".into(),
                },
            ),
        )
        .await
        .unwrap();
    assert!(matches!(
        plan[0].disposition,
        BatchImagePlanDisposition::Convert { ref destination } if destination == "photo-converted.heic"
    ));

    let execution = runtime
        .execute_batch_image(
            vec![entity.parse().unwrap()],
            settings_format(
                BatchImageTargetFormat::Heic,
                BatchImageConflictStrategy::Skip,
                BatchImageOutput::Sibling {
                    suffix: "-converted".into(),
                },
            ),
        )
        .await
        .unwrap();
    assert!(matches!(
        execution.outcomes[0].outcome,
        BatchImageItemOutcome::Converted { ref destination, width, height, .. }
            if destination == "photo-converted.heic" && width > 0 && height > 0
    ));

    // The converted output decodes through the same pipeline and reports the
    // target format.
    let backend = viewer_platform_macos::image::image_io::ImageIoBackend::default();
    let probe = backend
        .probe_sync(project.path().join("photo-converted.heic"))
        .unwrap();
    assert_eq!(probe.format, viewer_domain::image::ImageFormat::Heic);
}

fn node_for(root: &Path, name: &str) -> viewer_domain::file::FileNode {
    use std::os::unix::fs::MetadataExt;
    let path = root.join(name);
    let metadata = fs::metadata(&path).unwrap();
    viewer_domain::file::FileNode {
        entity_id: viewer_domain::EntityId::from_u128(
            (u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()),
        ),
        relative_path: viewer_domain::RelativePath::parse(name).unwrap(),
        kind: if name.ends_with(".txt") {
            viewer_domain::file::FileKind::Text
        } else if name.ends_with(".jpg") {
            viewer_domain::file::FileKind::Jpeg
        } else {
            viewer_domain::file::FileKind::Png
        },
        size: metadata.len(),
        modified_ns: i128::from(metadata.mtime()) * 1_000_000_000,
    }
}
