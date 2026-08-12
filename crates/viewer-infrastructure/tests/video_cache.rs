use async_trait::async_trait;
use std::{fs, path::Path, sync::Arc, thread, time::Duration};

use tempfile::tempdir;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use viewer_application::scheduler::TaskCoordinator;
use viewer_domain::{SessionId, VideoThumbnailRequestId};
use viewer_infrastructure::image_cache::{
    ImageArtifactLookup, ImageArtifactRegistry, ImageArtifactRegistryError, register_video_png,
};
use viewer_infrastructure::video_cache::{
    CacheError, VIDEO_CACHE_BUDGET_BYTES, VideoCache, VideoCacheKey, VideoSourceIdentity,
};
use viewer_infrastructure::{
    video_probe::MediaFileIdentity,
    video_thumbnail::{
        FrameExtractionError, TimelineThumbnailRequest, VideoFrameExtractor, VideoFrameOutput,
        VideoThumbnailArtifact, VideoThumbnailContext, VideoThumbnailService,
    },
};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfixture";

fn source(path: &str, size: u64, modified_ns: i128) -> VideoSourceIdentity {
    VideoSourceIdentity::new(Path::new(path), size, modified_ns)
}

fn initialized_cache() -> (tempfile::TempDir, VideoCache) {
    let app_cache = tempdir().unwrap();
    let cache = VideoCache::initialize(app_cache.path()).unwrap();
    (app_cache, cache)
}

struct PngExtractor;

#[async_trait]
impl VideoFrameExtractor for PngExtractor {
    async fn extract_frame(
        &self,
        _canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        _time_us: u64,
        _output: VideoFrameOutput,
        _cancellation: CancellationToken,
    ) -> Result<Vec<u8>, FrameExtractionError> {
        Ok(PNG.to_vec())
    }
}

async fn generated_artifact(
    cache: Arc<VideoCache>,
) -> (
    tempfile::TempDir,
    Arc<TaskCoordinator>,
    VideoThumbnailArtifact,
    VideoThumbnailRequestId,
) {
    let source_directory = tempdir().unwrap();
    let path = source_directory.path().join("clip.mp4");
    fs::write(&path, b"fixture").unwrap();
    let path = path.canonicalize().unwrap();
    let metadata = fs::metadata(&path).unwrap();
    let source = VideoSourceIdentity::new(&path, metadata.len(), modified_ns(&metadata));
    let identity = MediaFileIdentity::from_metadata(&metadata);
    let coordinator = Arc::new(TaskCoordinator::default());
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let request_id = VideoThumbnailRequestId::new();
    let (_playback_tx, playback_rx) = watch::channel(false);
    let service = VideoThumbnailService::new(
        Arc::new(PngExtractor),
        cache,
        Arc::clone(&coordinator),
        playback_rx,
    );
    let artifact = service
        .timeline(TimelineThumbnailRequest::new(
            VideoThumbnailContext::new(
                session,
                generation,
                source,
                identity,
                CancellationToken::new(),
            ),
            request_id,
            1_000_000,
            5_000_000,
        ))
        .await
        .unwrap();
    (source_directory, coordinator, artifact, request_id)
}

fn modified_ns(metadata: &fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

#[test]
fn identity_changes_after_source_mtime_size_kind_bucket_or_algorithm_version() {
    let cover = VideoCacheKey::cover(source("/project/a.mp4", 12, 100), 1);
    assert_ne!(
        cover,
        VideoCacheKey::cover(source("/project/a.mp4", 12, 101), 1)
    );
    assert_ne!(
        cover,
        VideoCacheKey::cover(source("/project/a.mp4", 13, 100), 1)
    );
    assert_ne!(
        cover,
        VideoCacheKey::cover(source("/project/a.mp4", 12, 100), 2)
    );
    assert_ne!(
        cover,
        VideoCacheKey::timeline(source("/project/a.mp4", 12, 100), 0, 1)
    );
    assert_ne!(
        VideoCacheKey::timeline(source("/project/a.mp4", 12, 100), 0, 1),
        VideoCacheKey::timeline(source("/project/a.mp4", 12, 100), 500_000, 1)
    );
}

#[test]
fn clear_refuses_a_root_without_the_viewer_video_cache_marker() {
    let root = tempdir().unwrap();
    assert_eq!(
        VideoCache::open(root.path()).unwrap_err(),
        CacheError::UnverifiedRoot
    );
}

#[test]
fn initialization_creates_only_the_exact_video_child_and_reopens_it() {
    let app_cache = tempdir().unwrap();
    let cache = VideoCache::initialize(app_cache.path()).unwrap();

    assert_eq!(
        cache.root(),
        app_cache.path().canonicalize().unwrap().join("video")
    );
    assert!(cache.root().join(".viewer-video-cache-v1").is_file());
    assert_eq!(VideoCache::open(cache.root()).unwrap().root(), cache.root());
}

#[test]
fn atomic_put_get_and_stats_publish_only_the_final_png() {
    let (_app_cache, cache) = initialized_cache();
    let key = VideoCacheKey::cover(source("/project/a.mp4", 12, 100), 1);

    let path = cache.put_atomic(&key, PNG).unwrap();

    assert_eq!(cache.get(&key).unwrap(), Some(path.clone()));
    assert_eq!(fs::read(&path).unwrap(), PNG);
    assert!(
        fs::read_dir(path.parent().unwrap())
            .unwrap()
            .all(|entry| !entry.unwrap().file_name().to_string_lossy().contains("tmp"))
    );
    let stats = cache.stats().unwrap();
    assert_eq!(stats.entry_count, 1);
    assert_eq!(stats.bytes_used, PNG.len() as u64 + 1);
    assert_eq!(stats.budget_bytes, VIDEO_CACHE_BUDGET_BYTES);
}

#[test]
fn lru_hit_refreshes_access_and_evicts_the_older_entry() {
    let (_app_cache, cache) = initialized_cache();
    let first = VideoCacheKey::cover(source("/project/first.mp4", 1, 1), 1);
    let second = VideoCacheKey::cover(source("/project/second.mp4", 1, 1), 1);
    let trigger = VideoCacheKey::cover(source("/project/trigger.mp4", 1, 1), 1);
    let first_path = cache.put_atomic(&first, PNG).unwrap();
    thread::sleep(Duration::from_millis(5));
    let second_path = cache.put_atomic(&second, PNG).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&first_path)
        .unwrap()
        .set_len(VIDEO_CACHE_BUDGET_BYTES / 2 + 1)
        .unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&second_path)
        .unwrap()
        .set_len(VIDEO_CACHE_BUDGET_BYTES / 2 + 1)
        .unwrap();
    thread::sleep(Duration::from_millis(5));
    assert!(cache.get(&first).unwrap().is_some());
    thread::sleep(Duration::from_millis(5));

    cache.put_atomic(&trigger, PNG).unwrap();

    assert!(cache.get(&first).unwrap().is_some());
    assert!(cache.get(&second).unwrap().is_none());
    assert!(cache.get(&trigger).unwrap().is_some());
    assert!(cache.stats().unwrap().bytes_used <= VIDEO_CACHE_BUDGET_BYTES);
}

#[tokio::test]
async fn clear_removes_verified_entries_but_preserves_the_marker() {
    let (_app_cache, cache) = initialized_cache();
    let key = VideoCacheKey::cover(source("/project/a.mp4", 12, 100), 1);
    cache.put_atomic(&key, PNG).unwrap();

    let stats = cache.clear().await.unwrap();

    assert_eq!(stats.entry_count, 0);
    assert_eq!(stats.bytes_used, 0);
    assert!(cache.root().join(".viewer-video-cache-v1").is_file());
    assert!(cache.get(&key).unwrap().is_none());
}

#[test]
fn stats_accounts_for_temp_only_owned_entries() {
    let (_app_cache, cache) = initialized_cache();
    let key = VideoCacheKey::cover(source("/project/interrupted.mp4", 12, 100), 1);
    let entry = cache.root().join(key.to_hex());
    fs::create_dir(&entry).unwrap();
    fs::write(
        entry.join(".artifact.tmp-00000000000000000000000000000000"),
        vec![0_u8; 4096],
    )
    .unwrap();

    let stats = cache.stats().unwrap();

    assert_eq!(stats.entry_count, 1);
    assert_eq!(stats.bytes_used, 4096);
}

#[tokio::test]
async fn clear_removes_temp_only_owned_entries() {
    let (_app_cache, cache) = initialized_cache();
    let key = VideoCacheKey::cover(source("/project/interrupted.mp4", 12, 100), 1);
    let entry = cache.root().join(key.to_hex());
    fs::create_dir(&entry).unwrap();
    fs::write(
        entry.join(".artifact.tmp-00000000000000000000000000000000"),
        vec![0_u8; 4096],
    )
    .unwrap();

    let stats = cache.clear().await.unwrap();

    assert_eq!(stats.entry_count, 0);
    assert_eq!(stats.bytes_used, 0);
    assert!(!entry.exists());
}

#[tokio::test]
async fn clear_waits_for_an_atomic_write_started_by_another_cache_handle() {
    let (_app_cache, cache) = initialized_cache();
    let key = VideoCacheKey::cover(source("/project/a.mp4", 12, 100), 1);
    cache.put_atomic(&key, PNG).unwrap();
    let root = cache.root().to_path_buf();
    let second_handle = VideoCache::open(&root).unwrap();
    let mut large_png = vec![0_u8; 128 * 1024 * 1024];
    large_png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    let writer = thread::spawn(move || cache.put_atomic(&key, &large_png));
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let temporary_exists = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .flat_map(|entry| fs::read_dir(entry.path()).into_iter().flatten())
            .filter_map(Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".artifact.tmp-")
            });
        if temporary_exists {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "atomic writer did not publish its temporary file"
        );
        thread::yield_now();
    }

    let cleared = second_handle.clear().await;
    let written = writer.join().unwrap();

    assert!(written.is_ok(), "clear raced the writer: {written:?}");
    assert!(cleared.is_ok(), "write raced the clear: {cleared:?}");
    assert_eq!(second_handle.stats().unwrap().entry_count, 0);
}

#[cfg(unix)]
#[test]
fn cache_rejects_an_entry_directory_replaced_by_a_symlink() {
    let (_app_cache, cache) = initialized_cache();
    let key = VideoCacheKey::cover(source("/project/a.mp4", 12, 100), 1);
    let artifact = cache.put_atomic(&key, PNG).unwrap();
    let entry = artifact.parent().unwrap();
    let outside = tempdir().unwrap();
    fs::remove_dir_all(entry).unwrap();
    std::os::unix::fs::symlink(outside.path(), entry).unwrap();

    assert_eq!(cache.get(&key).unwrap_err(), CacheError::UnsafeEntry);
}

#[tokio::test]
async fn registered_video_png_is_session_scoped_and_preserves_artifact_identity() {
    let app_cache = tempdir().unwrap();
    let cache = Arc::new(VideoCache::initialize(app_cache.path()).unwrap());
    let (_source, _coordinator, artifact, request_id) =
        generated_artifact(Arc::clone(&cache)).await;
    let registry = ImageArtifactRegistry::default();
    let session = artifact.session_id();
    let generation = artifact.generation();
    let path = artifact.path().to_path_buf();

    let token = register_video_png(&registry, &cache, &artifact).unwrap();

    assert!(matches!(
        registry.lookup(session, token.as_str()),
        ImageArtifactLookup::Found(ref registered)
            if registered.cache_path() == path
                && registered.mime() == "image/png"
                && registered.generation() == Some(generation)
                && registered.video_request_id() == Some(request_id)
                && registered.immutable_bytes() == Some(PNG)
                && !registered.retains_cache_file()
    ));
    assert_eq!(
        registry.lookup(SessionId::new(), token.as_str()),
        ImageArtifactLookup::WrongSession
    );
}

#[tokio::test]
async fn clearing_a_registered_png_reclaims_the_cache_entry_without_losing_immutable_bytes() {
    let app_cache = tempdir().unwrap();
    let cache = Arc::new(VideoCache::initialize(app_cache.path()).unwrap());
    let (_source, _coordinator, artifact, _request_id) =
        generated_artifact(Arc::clone(&cache)).await;
    let registry = ImageArtifactRegistry::default();
    let session = artifact.session_id();
    let path = artifact.path().to_path_buf();
    let token = register_video_png(&registry, &cache, &artifact).unwrap();

    let stats = cache.clear().await.unwrap();

    assert_eq!(stats.bytes_used, 0);
    assert_eq!(stats.entry_count, 0);
    assert!(!path.exists());
    assert!(matches!(
        registry.lookup(session, token.as_str()),
        ImageArtifactLookup::Found(ref registered)
            if registered.immutable_bytes() == Some(PNG)
                && !registered.retains_cache_file()
    ));
}

#[tokio::test]
async fn video_png_registration_refuses_an_artifact_from_another_cache_capability() {
    let first_root = tempdir().unwrap();
    let first_cache = Arc::new(VideoCache::initialize(first_root.path()).unwrap());
    let (_source, _coordinator, artifact, _request_id) = generated_artifact(first_cache).await;
    let second_root = tempdir().unwrap();
    let second_cache = VideoCache::initialize(second_root.path()).unwrap();

    assert_eq!(
        register_video_png(&ImageArtifactRegistry::default(), &second_cache, &artifact)
            .unwrap_err(),
        ImageArtifactRegistryError::UnverifiedVideoCache
    );
}
