use async_trait::async_trait;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use viewer_application::{
    BrowseIndexPort, ImageArtifact, ImageError, ImagePort, ImageRequest, ReviewAssetCatalogPort,
    ReviewAssetError, ReviewAssetValidation, ReviewProgressPort, ReviewScope,
    ReviewTaskCancellation, ReviewTaskProgress, review_assets::ContinuousReviewAssetPort,
};
use viewer_domain::file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata};
use viewer_domain::image::{ImageFormat, ImageProbe};
use viewer_domain::review::continuous::SourceCheckStatus;
use viewer_domain::review::{ReviewMedia, ReviewabilityFailure};
use viewer_domain::search::Generation;
use viewer_domain::video::{VideoFailureKind, VideoMetadata, VideoProbeStatus};
use viewer_domain::{EntityId, RelativePath};
use viewer_infrastructure::review::{IndexedReviewAssetCatalog, ReviewChangeLedger};
use viewer_infrastructure::search::index::SessionIndex;
use viewer_infrastructure::video_probe::{MediaFileIdentity, VideoMetadataProbe, VideoProbeError};

struct ProjectFixture {
    directory: TempDir,
    index: Arc<SessionIndex>,
    generation: Generation,
}

impl ProjectFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let index = Arc::new(SessionIndex::open(directory.path().join("session.sqlite")).unwrap());
        Self {
            directory,
            index,
            generation: Generation::new(1),
        }
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }

    fn write(&self, relative: &str, bytes: &[u8], kind: FileKind) -> FileNode {
        let path = self.root().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        node_for_path(self.root(), relative, kind)
    }

    fn directory(&self, relative: &str) -> FileNode {
        fs::create_dir_all(self.root().join(relative)).unwrap();
        node_for_path(self.root(), relative, FileKind::Directory)
    }

    fn publish(&self, nodes: &[FileNode]) {
        self.index.upsert_batch(nodes, self.generation).unwrap();
    }

    fn ready_image(&self, node: &FileNode, width: u32, height: u32) {
        self.index
            .replace_image_metadata(
                node.entity_id,
                &node.relative_path,
                Ok(ImageMetadata { width, height }),
            )
            .unwrap();
    }
}

fn node_for_path(root: &Path, relative: &str, kind: FileKind) -> FileNode {
    let relative_path = RelativePath::parse(relative).unwrap();
    let metadata = fs::symlink_metadata(root.join(relative)).unwrap();
    FileNode {
        entity_id: entity_id(&metadata),
        relative_path,
        kind,
        size: metadata.len(),
        modified_ns: modified_ns(&metadata),
    }
}

#[cfg(unix)]
fn entity_id(metadata: &fs::Metadata) -> EntityId {
    use std::os::unix::fs::MetadataExt;
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

#[cfg(unix)]
fn modified_ns(metadata: &fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

struct StaticImageProbe(Result<ImageProbe, ImageFailure>);

struct ReplacingImageProbe;

struct BlockingImageProbe {
    started: tokio::sync::Notify,
}

#[derive(Clone, Copy)]
enum ImageFailure {
    Unsupported,
    Corrupt,
    BudgetExceeded,
    Io,
}

impl StaticImageProbe {
    fn ready(width: u32, height: u32) -> Self {
        Self(Ok(ImageProbe {
            format: ImageFormat::Png,
            width,
            height,
            orientation: 1,
            has_alpha: true,
            icc_profile_name: None,
        }))
    }
}

#[async_trait]
impl ImagePort for StaticImageProbe {
    async fn probe(&self, _source: &Path) -> Result<ImageProbe, ImageError> {
        self.0.clone().map_err(|failure| match failure {
            ImageFailure::Unsupported => ImageError::Unsupported,
            ImageFailure::Corrupt => ImageError::Corrupt,
            ImageFailure::BudgetExceeded => ImageError::BudgetExceeded,
            ImageFailure::Io => ImageError::Io("transient".into()),
        })
    }

    async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        panic!("review preparation must never render an image")
    }

    async fn cancel_session(&self, _session_id: viewer_domain::SessionId) {}
}

#[async_trait]
impl ImagePort for ReplacingImageProbe {
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError> {
        let replacement = source.with_extension("replacement");
        fs::write(&replacement, b"replacement").unwrap();
        fs::rename(replacement, source).unwrap();
        Ok(ImageProbe {
            format: ImageFormat::Png,
            width: 10,
            height: 10,
            orientation: 1,
            has_alpha: true,
            icc_profile_name: None,
        })
    }

    async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        panic!("review preparation must never render an image")
    }

    async fn cancel_session(&self, _session_id: viewer_domain::SessionId) {}
}

#[async_trait]
impl ImagePort for BlockingImageProbe {
    async fn probe(&self, _source: &Path) -> Result<ImageProbe, ImageError> {
        self.started.notify_one();
        std::future::pending().await
    }

    async fn render(&self, _request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        panic!("review preparation must never render an image")
    }

    async fn cancel_session(&self, _session_id: viewer_domain::SessionId) {}
}

#[derive(Clone, Copy)]
enum VideoAnswer {
    Ready,
    Failed(VideoFailureKind),
    Pending,
}

struct StaticVideoProbe(VideoAnswer);

impl StaticVideoProbe {
    fn answer(&self) -> Result<VideoMetadata, VideoProbeError> {
        match self.0 {
            VideoAnswer::Ready => Ok(VideoMetadata {
                duration_us: Some(2_000_000),
                display_width: Some(1920),
                display_height: Some(1080),
                rotation_degrees: 0,
                frame_rate_millihertz: Some(24_000),
                video_codec: Some("h264".into()),
                audio_codec: None,
                probe_status: VideoProbeStatus::Ready,
            }),
            VideoAnswer::Failed(failure) => Err(VideoProbeError::Failed(failure)),
            VideoAnswer::Pending => Ok(VideoMetadata {
                duration_us: None,
                display_width: None,
                display_height: None,
                rotation_degrees: 0,
                frame_rate_millihertz: None,
                video_codec: None,
                audio_codec: None,
                probe_status: VideoProbeStatus::Pending,
            }),
        }
    }
}

#[async_trait]
impl VideoMetadataProbe for StaticVideoProbe {
    async fn probe(
        &self,
        _canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        if cancellation.is_cancelled() {
            Err(VideoProbeError::Cancelled)
        } else {
            self.answer()
        }
    }

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        self.probe(canonical_path, cancellation).await
    }
}

#[derive(Default)]
struct Progress(Mutex<Vec<ReviewTaskProgress>>);

impl ReviewProgressPort for Progress {
    fn report(&self, progress: ReviewTaskProgress) {
        self.0.lock().unwrap().push(progress);
    }
}

fn catalog(
    project: &ProjectFixture,
    image: Arc<dyn ImagePort>,
    video: Arc<dyn VideoMetadataProbe>,
    ledger: ReviewChangeLedger,
) -> IndexedReviewAssetCatalog {
    let index: Arc<dyn BrowseIndexPort> = project.index.clone();
    IndexedReviewAssetCatalog::new(project.root(), index, image, video, ledger).unwrap()
}

#[tokio::test]
async fn scopes_are_exact_deduplicated_sorted_and_count_exclusions() {
    let project = ProjectFixture::new();
    let image = project.write("b.png", b"image-b", FileKind::Png);
    let video = project.write("a.mp4", b"video-a", FileKind::Video);
    let text = project.write("notes.txt", b"notes", FileKind::Text);
    let folder = project.directory("nested");
    let nested = project.write("nested/c.jpg", b"image-c", FileKind::Jpeg);
    project.publish(&[
        image.clone(),
        video.clone(),
        text.clone(),
        folder,
        nested.clone(),
    ]);
    project.ready_image(&image, 20, 10);
    project.ready_image(&nested, 30, 15);
    let catalog = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(1, 1)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ReviewChangeLedger::default(),
    );

    let selection = catalog
        .resolve_scope(&ReviewScope::Selection {
            entity_ids: vec![
                image.entity_id,
                video.entity_id,
                image.entity_id,
                text.entity_id,
            ],
        })
        .await
        .unwrap();
    assert_eq!(
        selection.candidate_entity_ids,
        vec![video.entity_id, image.entity_id]
    );
    assert_eq!((selection.image_count, selection.video_count), (1, 1));
    assert_eq!(selection.excluded_count, 1);

    let direct = catalog
        .resolve_scope(&ReviewScope::Folder {
            folder_id: None,
            include_descendants: false,
        })
        .await
        .unwrap();
    assert_eq!(
        direct.candidate_entity_ids,
        vec![video.entity_id, image.entity_id]
    );
    assert_eq!(direct.excluded_count, 2);

    let aggregate = catalog
        .resolve_scope(&ReviewScope::Folder {
            folder_id: None,
            include_descendants: true,
        })
        .await
        .unwrap();
    assert_eq!(
        aggregate.candidate_entity_ids,
        vec![video.entity_id, image.entity_id, nested.entity_id]
    );
    assert_eq!((aggregate.image_count, aggregate.video_count), (2, 1));

    assert!(
        catalog
            .resolve_scope(&ReviewScope::Selection { entity_ids: vec![] })
            .await
            .unwrap()
            .candidate_entity_ids
            .is_empty()
    );
}

#[tokio::test]
async fn source_checks_rebind_session_identity_when_content_verifies_after_device_drift() {
    let project = ProjectFixture::new();
    let image_bytes = b"review-source";
    let image = project.write("drift.png", image_bytes, FileKind::Png);
    project.publish(std::slice::from_ref(&image));
    project.ready_image(&image, 8, 8);
    let catalog = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(8, 8)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ReviewChangeLedger::default(),
    );
    let prepared = catalog
        .prepare_assets(
            &[image.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await
        .unwrap();

    // A reboot reassigned the volume device id: the persisted session identity
    // no longer matches the file's current derived identity, but the content
    // (size + blake3) is unchanged.
    let drifted = EntityId::from_u128((u128::from(0x77_u32) << 64) | 0xBEEF);
    assert_ne!(drifted, image.entity_id);
    let mut assets = vec![prepared[0].asset.clone()];
    assets[0].source_entity_id = Some(drifted);

    let checks = catalog
        .check_sources(&mut assets, ReviewTaskCancellation::default())
        .await
        .unwrap();

    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].asset_version_id, assets[0].id);
    assert_eq!(checks[0].status, SourceCheckStatus::Match);
    assert_eq!(assets[0].source_entity_id, Some(image.entity_id));

    // A later load from the persisted state repeats the pattern and stays stable.
    let mut reloaded = vec![prepared[0].asset.clone()];
    reloaded[0].source_entity_id = Some(drifted);
    let repeated = catalog
        .check_sources(&mut reloaded, ReviewTaskCancellation::default())
        .await
        .unwrap();
    assert_eq!(repeated[0].status, SourceCheckStatus::Match);
    assert_eq!(reloaded[0].source_entity_id, Some(image.entity_id));
}

#[tokio::test]
async fn preparation_streams_content_and_reports_only_completed_members() {
    let project = ProjectFixture::new();
    let image_bytes = b"image-content";
    let video_bytes = b"video-content";
    let image = project.write("image.png", image_bytes, FileKind::Png);
    let video = project.write("video.mp4", video_bytes, FileKind::Video);
    project.publish(&[image.clone(), video.clone()]);
    project.ready_image(&image, 640, 480);
    let progress = Arc::new(Progress::default());
    let catalog = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(1, 1)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ReviewChangeLedger::default(),
    );

    let prepared = catalog
        .prepare_assets(
            &[image.entity_id, video.entity_id],
            ReviewTaskCancellation::default(),
            progress.clone(),
        )
        .await
        .unwrap();

    assert_eq!(prepared.len(), 2);
    assert_eq!(prepared[0].entity_id, image.entity_id);
    assert_eq!(
        prepared[0].asset.evidence.blake3,
        Some(*blake3::hash(image_bytes).as_bytes())
    );
    assert_eq!(prepared[0].asset.source_entity_id, Some(image.entity_id));
    assert_eq!(
        prepared[0].source_path,
        fs::canonicalize(project.root()).unwrap().join("image.png")
    );
    assert_eq!(
        prepared[0].asset.media,
        ReviewMedia::Image {
            width: Some(640),
            height: Some(480),
        }
    );
    assert_eq!(
        prepared[1].asset.evidence.blake3,
        Some(*blake3::hash(video_bytes).as_bytes())
    );
    assert_eq!(
        progress.0.lock().unwrap().last().copied(),
        Some(ReviewTaskProgress {
            completed: 2,
            total: 2,
        })
    );
}

#[cfg(unix)]
#[tokio::test]
async fn preparation_rejects_symlinks_and_cancellation_releases_tracking() {
    use std::os::unix::fs::symlink;

    let project = ProjectFixture::new();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let link_path = project.root().join("linked.png");
    symlink(outside.path(), &link_path).unwrap();
    let link = node_for_path(project.root(), "linked.png", FileKind::Png);
    project.publish(std::slice::from_ref(&link));
    let ledger = ReviewChangeLedger::default();
    let catalog = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(10, 10)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ledger.clone(),
    );

    assert_eq!(
        catalog
            .prepare_assets(
                &[link.entity_id],
                ReviewTaskCancellation::default(),
                Arc::new(Progress::default()),
            )
            .await,
        Err(ReviewAssetError::UnsafeSource)
    );

    let cancellation = ReviewTaskCancellation::default();
    cancellation.cancel();
    assert_eq!(
        catalog
            .prepare_assets(
                &[link.entity_id],
                cancellation,
                Arc::new(Progress::default()),
            )
            .await,
        Err(ReviewAssetError::Cancelled)
    );
    ledger.record(link.entity_id);
    assert_eq!(ledger.revision(link.entity_id), 0);
}

#[tokio::test]
async fn source_replacement_during_media_confirmation_invalidates_the_whole_preparation() {
    let project = ProjectFixture::new();
    let image = project.write("replace.png", b"original", FileKind::Png);
    project.publish(std::slice::from_ref(&image));
    project
        .index
        .replace_image_metadata(
            image.entity_id,
            &image.relative_path,
            Err(ImageIndexStatus::Failed),
        )
        .unwrap();
    let catalog = catalog(
        &project,
        Arc::new(ReplacingImageProbe),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ReviewChangeLedger::default(),
    );

    assert_eq!(
        catalog
            .prepare_assets(
                &[image.entity_id],
                ReviewTaskCancellation::default(),
                Arc::new(Progress::default()),
            )
            .await,
        Err(ReviewAssetError::SourceChanged)
    );
}

#[tokio::test]
async fn cancellation_interrupts_inflight_media_confirmation_and_releases_tracking() {
    let project = ProjectFixture::new();
    let image = project.write("blocked.png", b"blocked", FileKind::Png);
    project.publish(std::slice::from_ref(&image));
    project
        .index
        .replace_image_metadata(
            image.entity_id,
            &image.relative_path,
            Err(ImageIndexStatus::Failed),
        )
        .unwrap();
    let image_probe = Arc::new(BlockingImageProbe {
        started: tokio::sync::Notify::new(),
    });
    let ledger = ReviewChangeLedger::default();
    let catalog = catalog(
        &project,
        image_probe.clone(),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ledger.clone(),
    );
    let cancellation = ReviewTaskCancellation::default();
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        catalog
            .prepare_assets(
                &[image.entity_id],
                task_cancellation,
                Arc::new(Progress::default()),
            )
            .await
    });
    image_probe.started.notified().await;

    cancellation.cancel();

    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .expect("cancellation must interrupt a pending media confirmation")
            .unwrap(),
        Err(ReviewAssetError::Cancelled)
    );
    ledger.record(image.entity_id);
    assert_eq!(ledger.revision(image.entity_id), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn proven_permission_failure_is_explicitly_unreviewable_without_a_digest() {
    use std::os::unix::fs::PermissionsExt;

    let project = ProjectFixture::new();
    let image = project.write("private.png", b"private", FileKind::Png);
    project.publish(std::slice::from_ref(&image));
    let source = project.root().join("private.png");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o000)).unwrap();
    let catalog = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(10, 10)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ReviewChangeLedger::default(),
    );

    let result = catalog
        .prepare_assets(
            &[image.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await;
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();

    let prepared = result.unwrap();
    assert_eq!(prepared[0].asset.evidence.blake3, None);
    assert_eq!(
        prepared[0].failure,
        Some(ReviewabilityFailure::PermissionDenied)
    );
    assert_eq!(
        prepared[0].asset.media,
        ReviewMedia::Image {
            width: None,
            height: None,
        }
    );
}

#[tokio::test]
async fn media_confirmation_distinguishes_reviewable_stable_and_transient_failures() {
    let project = ProjectFixture::new();
    let image = project.write("grid.png", b"grid-image", FileKind::Png);
    let unsupported = project.write("source.tiff", b"unsupported", FileKind::UnsupportedImage);
    let video = project.write("clip.mp4", b"video", FileKind::Video);
    project.publish(&[image.clone(), unsupported.clone(), video.clone()]);
    project
        .index
        .replace_image_metadata(
            image.entity_id,
            &image.relative_path,
            Err(ImageIndexStatus::Failed),
        )
        .unwrap();

    let ready = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(800, 600)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ReviewChangeLedger::default(),
    );
    let prepared = ready
        .prepare_assets(
            &[image.entity_id, unsupported.entity_id, video.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await
        .unwrap();
    assert_eq!(
        prepared
            .iter()
            .find(|prepared| prepared.entity_id == image.entity_id)
            .unwrap()
            .failure,
        None
    );
    assert_eq!(
        prepared
            .iter()
            .find(|prepared| prepared.entity_id == unsupported.entity_id)
            .unwrap()
            .failure,
        Some(ReviewabilityFailure::Unsupported)
    );
    assert_eq!(
        prepared
            .iter()
            .find(|prepared| prepared.entity_id == video.entity_id)
            .unwrap()
            .failure,
        None
    );

    let transient = catalog(
        &project,
        Arc::new(StaticImageProbe(Err(ImageFailure::BudgetExceeded))),
        Arc::new(StaticVideoProbe(VideoAnswer::Failed(
            VideoFailureKind::EngineInitialization,
        ))),
        ReviewChangeLedger::default(),
    );
    assert_eq!(
        transient
            .prepare_assets(
                &[video.entity_id],
                ReviewTaskCancellation::default(),
                Arc::new(Progress::default()),
            )
            .await,
        Err(ReviewAssetError::Pending)
    );
    assert_eq!(
        transient
            .prepare_assets(
                &[image.entity_id],
                ReviewTaskCancellation::default(),
                Arc::new(Progress::default()),
            )
            .await,
        Err(ReviewAssetError::Pending)
    );

    for (image_failure, expected) in [
        (ImageFailure::Unsupported, ReviewabilityFailure::Unsupported),
        (ImageFailure::Corrupt, ReviewabilityFailure::Damaged),
    ] {
        let stable_image = catalog(
            &project,
            Arc::new(StaticImageProbe(Err(image_failure))),
            Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
            ReviewChangeLedger::default(),
        )
        .prepare_assets(
            &[image.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await
        .unwrap();
        assert_eq!(stable_image[0].failure, Some(expected));
    }

    for transient_image in [ImageFailure::Io] {
        let pending_image = catalog(
            &project,
            Arc::new(StaticImageProbe(Err(transient_image))),
            Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
            ReviewChangeLedger::default(),
        )
        .prepare_assets(
            &[image.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await;
        assert_eq!(pending_image, Err(ReviewAssetError::Pending));
    }

    let pending_video = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(1, 1)),
        Arc::new(StaticVideoProbe(VideoAnswer::Pending)),
        ReviewChangeLedger::default(),
    )
    .prepare_assets(
        &[video.entity_id],
        ReviewTaskCancellation::default(),
        Arc::new(Progress::default()),
    )
    .await;
    assert_eq!(pending_video, Err(ReviewAssetError::Pending));

    let stable = catalog(
        &project,
        Arc::new(StaticImageProbe(Err(ImageFailure::Corrupt))),
        Arc::new(StaticVideoProbe(VideoAnswer::Failed(
            VideoFailureKind::Damaged,
        ))),
        ReviewChangeLedger::default(),
    );
    let stable_video = stable
        .prepare_assets(
            &[video.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await
        .unwrap();
    assert_eq!(stable_video[0].failure, Some(ReviewabilityFailure::Damaged));
    assert_eq!(
        ready
            .revalidate_assets(
                &stable_video,
                ReviewTaskCancellation::default(),
                Arc::new(Progress::default()),
            )
            .await
            .unwrap(),
        vec![ReviewAssetValidation::Conflict {
            asset_version_id: stable_video[0].asset.id,
            relative_path: stable_video[0].asset.relative_path.clone(),
            kind: viewer_application::ReviewAssetConflictKind::MediaChanged,
        }]
    );
    assert_eq!(
        transient
            .revalidate_assets(
                &stable_video,
                ReviewTaskCancellation::default(),
                Arc::new(Progress::default()),
            )
            .await
            .unwrap(),
        vec![ReviewAssetValidation::Pending {
            asset_version_id: stable_video[0].asset.id,
            relative_path: stable_video[0].asset.relative_path.clone(),
        }]
    );
}

#[tokio::test]
async fn watcher_ledger_forces_rehash_even_when_projection_evidence_looks_unchanged() {
    let project = ProjectFixture::new();
    let image = project.write("image.png", b"version-one", FileKind::Png);
    project.publish(std::slice::from_ref(&image));
    project.ready_image(&image, 10, 10);
    let ledger = ReviewChangeLedger::default();
    let catalog = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(10, 10)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ledger.clone(),
    );
    let mut prepared = catalog
        .prepare_assets(
            &[image.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await
        .unwrap()
        .remove(0);

    fs::write(project.root().join("image.png"), b"version-two").unwrap();
    let current = node_for_path(project.root(), "image.png", FileKind::Png);
    project.publish(std::slice::from_ref(&current));
    project.ready_image(&current, 10, 10);
    prepared.asset.evidence.modified_ns = current.modified_ns;
    ledger.record(image.entity_id);

    assert_eq!(
        catalog
            .revalidate_assets(
                &[prepared.clone()],
                ReviewTaskCancellation::default(),
                Arc::new(Progress::default()),
            )
            .await
            .unwrap(),
        vec![ReviewAssetValidation::Conflict {
            asset_version_id: prepared.asset.id,
            relative_path: image.relative_path,
            kind: viewer_application::ReviewAssetConflictKind::ContentChanged,
        }]
    );
}

#[tokio::test]
async fn changed_mtime_rehashes_filesystem_truth_even_if_the_index_is_stale() {
    let project = ProjectFixture::new();
    let image = project.write("image.png", b"same-content", FileKind::Png);
    project.publish(std::slice::from_ref(&image));
    project.ready_image(&image, 10, 10);
    let catalog = catalog(
        &project,
        Arc::new(StaticImageProbe::ready(10, 10)),
        Arc::new(StaticVideoProbe(VideoAnswer::Ready)),
        ReviewChangeLedger::default(),
    );
    let prepared = catalog
        .prepare_assets(
            &[image.entity_id],
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await
        .unwrap()
        .remove(0);
    std::thread::sleep(std::time::Duration::from_millis(2));
    fs::write(project.root().join("image.png"), b"same-content").unwrap();
    let actual_modified = modified_ns(&fs::metadata(project.root().join("image.png")).unwrap());

    let validation = catalog
        .revalidate_assets(
            std::slice::from_ref(&prepared),
            ReviewTaskCancellation::default(),
            Arc::new(Progress::default()),
        )
        .await
        .unwrap();

    let ReviewAssetValidation::Current(current) = &validation[0] else {
        panic!("identical bytes must remain current after conservative rehash")
    };
    assert_eq!(
        current.asset.evidence.blake3,
        prepared.asset.evidence.blake3
    );
    assert_eq!(current.asset.evidence.modified_ns, actual_modified);
    assert_eq!(
        current.source_path,
        fs::canonicalize(project.root()).unwrap().join("image.png")
    );
}
