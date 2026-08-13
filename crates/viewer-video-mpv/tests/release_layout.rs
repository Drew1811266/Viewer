use std::path::{Path, PathBuf};

use viewer_video_mpv::runtime_manifest::RuntimeLayout;

#[test]
fn release_layout_never_uses_host_fallbacks() {
    let layout = RuntimeLayout::from_bundle_root(Path::new("/Viewer.app/Contents/Resources"))
        .expect("an absolute bundle resource root must resolve");

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
