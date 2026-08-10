use std::{env, ffi::OsString, fs, path::PathBuf, process};

use viewer_video_mpv::{BundledMediaTools, MpvClient, MpvLibrary, runtime_manifest::RuntimeLayout};

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
    let media = env::temp_dir().join(format!("viewer-video-mpv-options-{}.mp4", process::id()));
    let arguments = [
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=size=16x16:rate=1",
        "-t",
        "1",
        "-c:v",
        "mpeg4",
    ]
    .into_iter()
    .map(OsString::from)
    .chain([media.as_os_str().to_owned()])
    .collect::<Vec<_>>();
    let encode = tools.ffmpeg(&arguments).await.unwrap();
    assert!(
        encode.status.success(),
        "{}",
        String::from_utf8_lossy(&encode.stderr)
    );

    let library = MpvLibrary::load(&layout).unwrap();
    let mut client = MpvClient::new(&library).unwrap();
    client
        .open_local_file(&media.canonicalize().unwrap())
        .unwrap();
    drop(client);
    fs::remove_file(media).unwrap();
}
