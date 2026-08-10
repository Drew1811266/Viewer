use std::path::{Path, PathBuf};

use viewer_video_mpv::runtime_manifest::{RuntimeLayout, RuntimeLayoutError};

#[test]
fn bundle_root_resolves_only_viewer_video_runtime_resources() {
    let layout = RuntimeLayout::from_bundle_root(Path::new("/Viewer.app/Contents/Resources"))
        .expect("an absolute bundle root must resolve");

    assert_eq!(
        layout.libmpv,
        PathBuf::from("/Viewer.app/Contents/Resources/ViewerVideoRuntime/lib/libmpv.2.dylib")
    );
    assert_eq!(
        layout.ffmpeg,
        PathBuf::from("/Viewer.app/Contents/Resources/ViewerVideoRuntime/bin/ffmpeg")
    );
    assert_eq!(
        layout.ffprobe,
        PathBuf::from("/Viewer.app/Contents/Resources/ViewerVideoRuntime/bin/ffprobe")
    );
}

#[test]
fn bundle_root_rejects_relative_paths() {
    assert_eq!(
        RuntimeLayout::from_bundle_root(Path::new("Viewer.app/Contents/Resources")),
        Err(RuntimeLayoutError::BundleRootMustBeAbsolute)
    );
}
