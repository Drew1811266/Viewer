#[path = "support/video_acceptance_evidence.rs"]
mod evidence;

use evidence::load_development_evidence;
use std::{fs, path::PathBuf, sync::Arc};
use tempfile::tempdir;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use viewer_application::scheduler::TaskCoordinator;
use viewer_domain::{SessionId, VideoThumbnailRequestId};
use viewer_infrastructure::{
    video_cache::{VideoCache, VideoSourceIdentity},
    video_probe::MediaFileIdentity,
    video_thumbnail::{TimelineThumbnailRequest, VideoThumbnailContext, VideoThumbnailService},
};
use viewer_video_mpv::{BundledMediaTools, runtime_manifest::RuntimeLayout};

#[test]
#[ignore = "requires macOS native video acceptance"]
fn thirty_open_close_navigation_cycles_return_to_baseline() {
    let evidence = load_development_evidence();
    assert_eq!(evidence.lifecycle.cycles, 30);
    assert_eq!(evidence.lifecycle.after, evidence.lifecycle.before);
    assert!(evidence.native.h264_first_frame);
    assert!(evidence.native.hevc_first_frame);
    assert!(evidence.native.frame_step_forward);
    assert!(evidence.native.frame_step_backward);
    assert!(evidence.native.timeline_preview);
    assert!(evidence.native.close_to_idle);

    println!(
        "PASS lifecycle cycles={} baseline={:?}",
        evidence.lifecycle.cycles, evidence.lifecycle.before
    );
}

#[tokio::test]
#[ignore = "requires bundled video runtime"]
async fn bundled_timeline_preview_produces_a_real_png() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/videos/h264-aac.mp4")
        .canonicalize()
        .expect("timeline fixture");
    let metadata = fs::metadata(&fixture).expect("timeline fixture metadata");
    let source = VideoSourceIdentity::new(&fixture, metadata.len(), modified_ns(&metadata));
    let identity = MediaFileIdentity::from_metadata(&metadata);
    let cache_root = tempdir().expect("timeline cache root");
    let cache = Arc::new(VideoCache::initialize(cache_root.path()).expect("timeline cache"));
    let coordinator = Arc::new(TaskCoordinator::default());
    let session = SessionId::new();
    let generation = coordinator.begin_session(session);
    let (_playback, playback_rx) = watch::channel(false);
    let layout = RuntimeLayout::from_bundle_root(&runtime_resources()).expect("reviewed runtime");
    let tools = BundledMediaTools::from_layout(&layout).expect("bundled media tools");
    let service = VideoThumbnailService::new(
        Arc::new(tools),
        Arc::clone(&cache),
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
            VideoThumbnailRequestId::new(),
            1_000_000,
            5_000_000,
        ))
        .await
        .expect("real bundled timeline thumbnail");
    assert!(
        fs::read(artifact.path())
            .expect("timeline PNG")
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    println!(
        "PASS bundled timeline preview {}",
        artifact.path().display()
    );
}

fn runtime_resources() -> PathBuf {
    let configured = PathBuf::from(
        std::env::var_os("VIEWER_VIDEO_RUNTIME_DIR")
            .expect("VIEWER_VIDEO_RUNTIME_DIR must name a reviewed runtime"),
    )
    .canonicalize()
    .expect("runtime directory must exist");
    if configured.file_name().and_then(|name| name.to_str()) == Some("ViewerVideoRuntime") {
        configured
            .parent()
            .expect("runtime has a resources parent")
            .to_path_buf()
    } else {
        configured
    }
}

fn modified_ns(metadata: &fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}
