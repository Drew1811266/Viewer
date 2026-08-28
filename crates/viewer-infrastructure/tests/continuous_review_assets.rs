use async_trait::async_trait;
use std::{fs, os::unix::fs::MetadataExt, path::Path, sync::Arc};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use viewer_application::{ImageArtifact, ImageError, ImagePort, ImageRequest, review_assets::*};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode, ImageMetadata},
    image::{ImageFormat, ImageProbe},
    review::continuous::SourceCheckStatus,
    search::Generation,
};
use viewer_infrastructure::{
    review::{IndexedReviewAssetCatalog, ReviewChangeLedger},
    search::index::SessionIndex,
    video_probe::{VideoMetadataProbe, VideoProbeError},
};

struct Fixture {
    root: TempDir,
    index: Arc<SessionIndex>,
    ledger: ReviewChangeLedger,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let index = Arc::new(SessionIndex::open(root.path().join("index.sqlite")).unwrap());
        Self {
            root,
            index,
            ledger: ReviewChangeLedger::default(),
        }
    }
    fn node(&self, path: &str) -> FileNode {
        let relative_path = RelativePath::parse(path).unwrap();
        let m = fs::metadata(self.root.path().join(path)).unwrap();
        FileNode {
            entity_id: EntityId::from_u128((u128::from(m.dev()) << 64) | u128::from(m.ino())),
            relative_path,
            kind: FileKind::Png,
            size: m.len(),
            modified_ns: i128::from(m.mtime()) * 1_000_000_000 + i128::from(m.mtime_nsec()),
        }
    }
    fn publish(&self, path: &str) -> FileNode {
        let node = self.node(path);
        self.index
            .upsert_batch(std::slice::from_ref(&node), Generation::new(1))
            .unwrap();
        self.index
            .replace_image_metadata(
                node.entity_id,
                &node.relative_path,
                Ok(ImageMetadata {
                    width: 10,
                    height: 10,
                }),
            )
            .unwrap();
        node
    }
    fn write(&self, path: &str, bytes: &[u8]) -> FileNode {
        let pathbuf = self.root.path().join(path);
        fs::create_dir_all(pathbuf.parent().unwrap()).unwrap();
        fs::write(pathbuf, bytes).unwrap();
        self.publish(path)
    }
    fn catalog(&self) -> IndexedReviewAssetCatalog {
        IndexedReviewAssetCatalog::new(
            self.root.path(),
            self.index.clone(),
            Arc::new(Probe),
            Arc::new(Probe),
            self.ledger.clone(),
        )
        .unwrap()
    }
}
struct Probe;
type ProbeHook = Arc<dyn Fn(&Path) -> ImageProbe + Send + Sync>;
struct InterceptProbe(ProbeHook);
#[async_trait]
impl ImagePort for InterceptProbe {
    async fn probe(&self, path: &Path) -> Result<ImageProbe, ImageError> {
        Ok((self.0)(path))
    }
    async fn render(&self, _: ImageRequest) -> Result<ImageArtifact, ImageError> {
        panic!("not a renderer")
    }
    async fn cancel_session(&self, _: SessionId) {}
}
#[async_trait]
impl ImagePort for Probe {
    async fn probe(&self, _: &Path) -> Result<ImageProbe, ImageError> {
        Ok(ImageProbe {
            format: ImageFormat::Png,
            width: 10,
            height: 10,
            orientation: 1,
            has_alpha: false,
            icc_profile_name: None,
        })
    }
    async fn render(&self, _: ImageRequest) -> Result<ImageArtifact, ImageError> {
        panic!("not an image renderer")
    }
    async fn cancel_session(&self, _: SessionId) {}
}
#[async_trait]
impl VideoMetadataProbe for Probe {
    async fn probe(
        &self,
        _: &Path,
        _: CancellationToken,
    ) -> Result<viewer_domain::video::VideoMetadata, VideoProbeError> {
        panic!("no video in this fixture")
    }
    async fn probe_identity_bound(
        &self,
        _: &Path,
        _: &viewer_infrastructure::video_probe::MediaFileIdentity,
        _: CancellationToken,
    ) -> Result<viewer_domain::video::VideoMetadata, VideoProbeError> {
        panic!("no video in this fixture")
    }
}

#[tokio::test]
async fn fresh_image_probe_uses_exif_upright_dimensions_without_reusing_indexed_dimensions() {
    for orientation in 1..=8 {
        let f = Fixture::new();
        let node = f.write("oriented.png", b"source");
        let image = InterceptProbe(Arc::new(move |_| ImageProbe {
            format: ImageFormat::Png,
            width: 800,
            height: 600,
            orientation,
            has_alpha: false,
            icc_profile_name: None,
        }));
        let catalog = IndexedReviewAssetCatalog::new(
            f.root.path(),
            f.index.clone(),
            Arc::new(image),
            Arc::new(Probe),
            f.ledger.clone(),
        )
        .unwrap();
        let value = catalog
            .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
            .await
            .unwrap()
            .remove(0);
        let (width, height) = if orientation >= 5 {
            (600, 800)
        } else {
            (800, 600)
        };
        assert_eq!(
            value.asset.media,
            viewer_domain::review::ReviewMedia::Image {
                width: Some(width),
                height: Some(height),
            },
            "orientation {orientation}"
        );
        assert_eq!(value.failure, None);
    }
}

#[tokio::test]
async fn fresh_image_probe_does_not_make_an_invalid_orientation_reviewable() {
    for orientation in [0, 9] {
        let f = Fixture::new();
        let node = f.write("invalid.png", b"source");
        let image = InterceptProbe(Arc::new(move |_| ImageProbe {
            format: ImageFormat::Png,
            width: 800,
            height: 600,
            orientation,
            has_alpha: false,
            icc_profile_name: None,
        }));
        let catalog = IndexedReviewAssetCatalog::new(
            f.root.path(),
            f.index.clone(),
            Arc::new(image),
            Arc::new(Probe),
            f.ledger.clone(),
        )
        .unwrap();
        let value = catalog
            .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(
            value.failure,
            Some(viewer_domain::review::ReviewabilityFailure::Damaged)
        );
    }
}

#[tokio::test]
async fn fresh_video_probe_does_not_pair_new_content_with_cached_duration_or_dimensions() {
    use viewer_domain::video::{VideoMetadata, VideoProbeStatus};
    use viewer_infrastructure::video_probe::MediaFileIdentity;
    struct FreshProbe(MediaFileIdentity, VideoMetadata);
    #[async_trait]
    impl VideoMetadataProbe for FreshProbe {
        async fn probe(
            &self,
            _: &Path,
            _: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            panic!("continuous preparation must bind video probing to the captured identity")
        }
        async fn probe_identity_bound(
            &self,
            _: &Path,
            identity: &MediaFileIdentity,
            _: CancellationToken,
        ) -> Result<VideoMetadata, VideoProbeError> {
            assert_eq!(*identity, self.0);
            Ok(self.1.clone())
        }
    }
    let f = Fixture::new();
    let path = f.root.path().join("clip.mp4");
    fs::write(&path, b"old clip").unwrap();
    let before = fs::metadata(&path).unwrap();
    let mut node = f.node("clip.mp4");
    node.kind = FileKind::Video;
    f.index
        .upsert_batch(std::slice::from_ref(&node), Generation::new(1))
        .unwrap();
    let old_media = VideoMetadata {
        duration_us: Some(10),
        display_width: Some(320),
        display_height: Some(240),
        rotation_degrees: 0,
        frame_rate_millihertz: Some(30_000),
        video_codec: None,
        audio_codec: None,
        probe_status: VideoProbeStatus::Ready,
    };
    f.index
        .replace_video_metadata(node.entity_id, &old_media, Generation::new(1))
        .unwrap();
    fs::write(&path, b"new clip").unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    let fresh = VideoMetadata {
        duration_us: Some(50),
        display_width: Some(800),
        display_height: Some(600),
        ..old_media
    };
    let catalog = IndexedReviewAssetCatalog::new(
        f.root.path(),
        f.index.clone(),
        Arc::new(Probe),
        Arc::new(FreshProbe(
            MediaFileIdentity::from_metadata(&fs::metadata(&path).unwrap()),
            fresh,
        )),
        f.ledger.clone(),
    )
    .unwrap();
    let value = catalog
        .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        value.asset.evidence.blake3,
        Some(*blake3::hash(b"new clip").as_bytes())
    );
    assert_eq!(
        value.asset.media,
        viewer_domain::review::ReviewMedia::Video {
            duration_us: Some(50),
            display_width: Some(800),
            display_height: Some(600),
        }
    );
    assert_eq!(
        catalog
            .check_sources(&[value.asset], ReviewTaskCancellation::default())
            .await
            .unwrap()[0]
            .status,
        SourceCheckStatus::Match
    );
}

#[tokio::test]
async fn additions_preserve_previous_tracking_and_always_hash_source_content() {
    let fixture = Fixture::new();
    let first = fixture.write("first.png", b"first");
    let second = fixture.write("second.png", b"second");
    let catalog = fixture.catalog();
    let first_asset = catalog
        .prepare_additions(&[first.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0);
    let second_asset = catalog
        .prepare_additions(&[second.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0);
    fixture.ledger.record(first.entity_id);
    assert!(fixture.ledger.revision(first.entity_id) > first_asset.change_revision);
    let path = fixture.root.path().join("first.png");
    let m = fs::metadata(&path).unwrap();
    fs::write(&path, b"FIRST").unwrap();
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(m.modified().unwrap()))
        .unwrap();
    let original = vec![first_asset.asset, second_asset.asset];
    let before = original.clone();
    let checks = catalog
        .check_sources(&original, ReviewTaskCancellation::default())
        .await
        .unwrap();
    assert_eq!(
        checks.iter().map(|v| v.status).collect::<Vec<_>>(),
        vec![SourceCheckStatus::Changed, SourceCheckStatus::Match]
    );
    assert_eq!(original, before);
}

fn relocation(node: &FileNode, previous: Option<ReviewSourceLocator>) -> SourceRelocationDecision {
    SourceRelocationDecision {
        previous,
        candidate: ReviewSourceLocator {
            entity_id: node.entity_id,
            relative_path: node.relative_path.clone(),
        },
        confirmation: SourceLocatorConfirmation::UserConfirmed,
    }
}

#[tokio::test]
async fn rename_and_move_require_explicit_locations_and_keep_captured_versions_immutable() {
    let f = Fixture::new();
    let original = f.write("original.png", b"pixels");
    let catalog = f.catalog();
    let prepared = catalog
        .prepare_additions(&[original.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0);
    let captured = prepared.asset.clone();
    let mut previous = None;
    let mut from = "original.png";
    for destination in ["renamed.png", "nested/moved.png"] {
        fs::create_dir_all(f.root.path().join(destination).parent().unwrap()).unwrap();
        if destination.contains('/') {
            let mut folder = f.node("nested");
            folder.kind = FileKind::Directory;
            f.index.upsert_batch(&[folder], Generation::new(1)).unwrap();
        }
        fs::rename(f.root.path().join(from), f.root.path().join(destination)).unwrap();
        let node = f.publish(destination);
        assert_eq!(
            catalog
                .check_sources(
                    std::slice::from_ref(&captured),
                    ReviewTaskCancellation::default()
                )
                .await
                .unwrap()[0]
                .status,
            if previous.is_none() {
                SourceCheckStatus::Unverified
            } else {
                SourceCheckStatus::Missing
            }
        );
        let decision = relocation(&node, previous.clone());
        assert_eq!(
            catalog
                .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
                .await,
            Err(ReviewAssetError::UnconfirmedLocation)
        );
        catalog
            .confirm_relocation(
                &captured,
                decision.clone(),
                ReviewTaskCancellation::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            catalog
                .check_sources(
                    std::slice::from_ref(&captured),
                    ReviewTaskCancellation::default()
                )
                .await
                .unwrap()[0]
                .status,
            SourceCheckStatus::Match
        );
        let again = catalog
            .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(again.asset, captured);
        assert!(again.source_path.ends_with(destination));
        assert_eq!(
            catalog
                .confirm_relocation(
                    &captured,
                    decision.clone(),
                    ReviewTaskCancellation::default()
                )
                .await,
            Err(ReviewAssetError::StaleLocator)
        );
        previous = Some(decision.candidate);
        from = destination;
    }
    assert_eq!(captured.relative_path.as_str(), "original.png");
}

#[tokio::test]
async fn duplicate_content_and_multiple_candidates_never_choose_a_business_identity() {
    let f = Fixture::new();
    let original = f.write("original.png", b"same bytes");
    let catalog = f.catalog();
    let captured = catalog
        .prepare_additions(&[original.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0)
        .asset;
    let left = f.write("left.png", b"same bytes");
    let right = f.write("right.png", b"same bytes");
    let candidates = catalog
        .prepare_additions(
            &[left.entity_id, right.entity_id],
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    assert_ne!(candidates[0].asset.id, captured.id);
    assert_ne!(candidates[0].asset.id, candidates[1].asset.id);
    fs::remove_file(f.root.path().join("original.png")).unwrap();
    assert_eq!(
        catalog
            .check_sources(
                std::slice::from_ref(&captured),
                ReviewTaskCancellation::default()
            )
            .await
            .unwrap()[0]
            .status,
        SourceCheckStatus::Missing
    );
    catalog
        .confirm_relocation(
            &captured,
            relocation(&right, None),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        catalog
            .check_sources(
                std::slice::from_ref(&captured),
                ReviewTaskCancellation::default()
            )
            .await
            .unwrap()[0]
            .status,
        SourceCheckStatus::Match
    );
    fs::write(f.root.path().join("right.png"), b"new pixels").unwrap();
    assert_eq!(
        catalog
            .check_sources(
                std::slice::from_ref(&captured),
                ReviewTaskCancellation::default()
            )
            .await
            .unwrap()[0]
            .status,
        SourceCheckStatus::Changed
    );
    // The other identical file does not become a fallback.
    assert_eq!(
        fs::read(f.root.path().join("left.png")).unwrap(),
        b"same bytes"
    );
}

#[tokio::test]
async fn bad_candidates_metadata_only_changes_and_cancellation_do_not_rewrite_facts() {
    let f = Fixture::new();
    let node = f.write("a.png", b"old pixels");
    let other = f.write("b.png", b"new pixels");
    let catalog = f.catalog();
    let captured = catalog
        .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0)
        .asset;
    assert_eq!(
        catalog
            .confirm_relocation(
                &captured,
                relocation(&other, None),
                ReviewTaskCancellation::default()
            )
            .await,
        Err(ReviewAssetError::SourceChanged)
    );
    let path = f.root.path().join("a.png");
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(
            fs::FileTimes::new()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(5)),
        )
        .unwrap();
    assert_eq!(
        catalog
            .check_sources(
                std::slice::from_ref(&captured),
                ReviewTaskCancellation::default()
            )
            .await
            .unwrap()[0]
            .status,
        SourceCheckStatus::Match
    );
    f.publish("a.png");
    assert_eq!(
        catalog
            .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
            .await
            .unwrap()[0]
            .asset,
        captured
    );
    let cancellation = ReviewTaskCancellation::default();
    cancellation.cancel();
    assert_eq!(
        catalog
            .prepare_additions(&[other.entity_id], cancellation.clone())
            .await,
        Err(ReviewAssetError::Cancelled)
    );
    assert_eq!(
        catalog
            .check_sources(std::slice::from_ref(&captured), cancellation)
            .await,
        Err(ReviewAssetError::Cancelled)
    );
    f.ledger.record(node.entity_id);
    assert!(f.ledger.revision(node.entity_id) > 0);
}

#[tokio::test]
async fn unreadable_symlinked_and_missing_sources_keep_old_versions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let f = Fixture::new();
    let node = f.write("a.png", b"pixels");
    let catalog = f.catalog();
    let captured = catalog
        .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0)
        .asset;
    let path = f.root.path().join("a.png");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let status = catalog
        .check_sources(
            std::slice::from_ref(&captured),
            ReviewTaskCancellation::default(),
        )
        .await
        .unwrap()[0]
        .status;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(status, SourceCheckStatus::Unreadable);
    fs::remove_file(&path).unwrap();
    symlink("outside.png", &path).unwrap();
    assert_eq!(
        catalog
            .check_sources(
                std::slice::from_ref(&captured),
                ReviewTaskCancellation::default()
            )
            .await
            .unwrap()[0]
            .status,
        SourceCheckStatus::Unreadable
    );
    assert_eq!(captured.relative_path.as_str(), "a.png");
}

#[tokio::test]
async fn concurrent_relocation_confirmations_are_cas_guarded_and_input_is_bounded() {
    let f = Fixture::new();
    let node = f.write("a.png", b"same");
    let left = f.write("b.png", b"same");
    let right = f.write("c.png", b"same");
    let catalog = f.catalog();
    let captured = catalog
        .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0)
        .asset;
    let (a, b) = tokio::join!(
        catalog.confirm_relocation(
            &captured,
            relocation(&left, None),
            ReviewTaskCancellation::default()
        ),
        catalog.confirm_relocation(
            &captured,
            relocation(&right, None),
            ReviewTaskCancellation::default()
        )
    );
    assert!(
        (a == Ok(()) && b == Err(ReviewAssetError::StaleLocator))
            || (b == Ok(()) && a == Err(ReviewAssetError::StaleLocator))
    );
    assert_eq!(
        catalog
            .prepare_additions(
                &vec![node.entity_id; 50_001],
                ReviewTaskCancellation::default()
            )
            .await,
        Err(ReviewAssetError::LimitExceeded)
    );
    assert_eq!(
        catalog
            .check_sources(
                &[captured.clone(), captured.clone()],
                ReviewTaskCancellation::default()
            )
            .await,
        Err(ReviewAssetError::InvalidScope)
    );
    let mut changed = captured;
    changed.evidence.modified_ns += 1;
    assert_eq!(
        catalog
            .check_sources(&[changed], ReviewTaskCancellation::default())
            .await,
        Err(ReviewAssetError::InvalidScope)
    );
}

#[tokio::test]
async fn continuous_preparation_does_not_trust_cached_dimensions_after_an_undetected_overwrite() {
    let f = Fixture::new();
    let node = f.write("a.png", b"original");
    let image = InterceptProbe(Arc::new(|_| ImageProbe {
        format: ImageFormat::Png,
        width: 30,
        height: 20,
        orientation: 1,
        has_alpha: false,
        icc_profile_name: None,
    }));
    let catalog = IndexedReviewAssetCatalog::new(
        f.root.path(),
        f.index.clone(),
        Arc::new(image),
        Arc::new(Probe),
        f.ledger.clone(),
    )
    .unwrap();
    let prepared = catalog
        .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        prepared.asset.media,
        viewer_domain::review::ReviewMedia::Image {
            width: Some(30),
            height: Some(20)
        }
    );
}

#[tokio::test]
async fn preparation_rejects_same_inode_overwrite_during_media_probe_even_when_mtime_is_restored() {
    let f = Fixture::new();
    let node = f.write("a.png", b"original");
    let image = InterceptProbe(Arc::new(|path| {
        let time = fs::metadata(path).unwrap().modified().unwrap();
        fs::write(path, b"ORIGINAL").unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(time))
            .unwrap();
        ImageProbe {
            format: ImageFormat::Png,
            width: 10,
            height: 10,
            orientation: 1,
            has_alpha: false,
            icc_profile_name: None,
        }
    }));
    let catalog = IndexedReviewAssetCatalog::new(
        f.root.path(),
        f.index.clone(),
        Arc::new(image),
        Arc::new(Probe),
        f.ledger.clone(),
    )
    .unwrap();
    assert_eq!(
        catalog
            .prepare_additions(&[node.entity_id], ReviewTaskCancellation::default())
            .await,
        Err(ReviewAssetError::SourceChanged)
    );
}
