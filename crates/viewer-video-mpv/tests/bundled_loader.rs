use std::{env, ffi::OsString, fs, path::PathBuf};

use tokio_util::sync::CancellationToken;
use viewer_video_mpv::{
    BundledMediaTools, MediaFileIdentity, MediaFrameOutput, MpvClient, MpvLibrary,
    runtime_manifest::RuntimeLayout,
};

fn staged_layout() -> RuntimeLayout {
    let resources = PathBuf::from(
        env::var_os("VIEWER_VIDEO_TEST_RESOURCES")
            .expect("VIEWER_VIDEO_TEST_RESOURCES must identify the staged resources directory"),
    );
    RuntimeLayout::from_bundle_root(&resources).unwrap()
}

#[test]
#[ignore = "requires a staged ViewerVideoRuntime"]
fn staged_runtime_exports_the_reviewed_mpv_api() {
    let layout = staged_layout();
    let library = MpvLibrary::load(&layout).unwrap();
    assert_eq!(library.path(), layout.libmpv);
}

#[tokio::test]
#[ignore = "requires a staged ViewerVideoRuntime"]
async fn staged_media_tool_runs_only_from_the_reviewed_bundle() {
    let layout = staged_layout();
    let tools = BundledMediaTools::from_layout(&layout).unwrap();
    let output = tools.ffprobe(&[OsString::from("-version")]).await.unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("ffprobe version"));
}

#[tokio::test]
#[ignore = "requires a staged ViewerVideoRuntime"]
async fn staged_client_accepts_the_locked_down_local_media_options() {
    let layout = staged_layout();
    let tools = BundledMediaTools::from_layout(&layout).unwrap();
    let media = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/videos/h264-aac.mp4")
        .canonicalize()
        .unwrap();
    let identity = MediaFileIdentity::from_metadata(&fs::metadata(&media).unwrap());
    for (output, expected_width) in [
        (MediaFrameOutput::Png320, 320_u32),
        (MediaFrameOutput::Png640, 640_u32),
    ] {
        let frame = tools
            .video_frame_identity_bound(&media, &identity, 0, output, CancellationToken::new())
            .await
            .unwrap();
        assert!(
            frame.status.success(),
            "{}",
            String::from_utf8_lossy(&frame.stderr)
        );
        assert!(frame.stdout.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(
            u32::from_be_bytes(frame.stdout[16..20].try_into().unwrap()),
            expected_width
        );
    }

    let library = MpvLibrary::load(&layout).unwrap();
    let mut client = MpvClient::new(&library).unwrap();
    client.open_local_file(&media).unwrap();
    drop(client);
}
