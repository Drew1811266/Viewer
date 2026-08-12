use std::{fs, path::Path, sync::Arc};

use viewer_application::{ProjectAccess, ProjectProbeError, ProjectProbePort};
use viewer_desktop::{dto::FolderWorkspaceDto, state::DesktopRuntime};
use viewer_domain::EntityId;

struct FixedProbe;

impl ProjectProbePort for FixedProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadWrite)
    }
}

async fn scanned_runtime(project: &Path) -> (DesktopRuntime, Option<EntityId>, Option<EntityId>) {
    let cache = tempfile::tempdir().expect("create cache root");
    let cache = cache.keep();
    let runtime = DesktopRuntime::new(cache, Arc::new(FixedProbe));
    runtime.open_project(project).await.expect("open project");
    runtime.wait_for_scan().await.expect("finish project scan");
    let FolderWorkspaceDto::Content { images, videos, .. } =
        runtime.query_folder(None).await.expect("query root")
    else {
        panic!("root should contain previewable files")
    };
    let image = images
        .iter()
        .find(|file| file.name == "still.png")
        .map(|file| file.entity_id.parse().expect("parse image entity"));
    let video = videos
        .iter()
        .find(|file| file.name == "clip.mp4")
        .map(|file| file.entity_id.parse().expect("parse video entity"));
    (runtime, image, video)
}

#[tokio::test]
async fn open_rejects_non_video_stale_and_outside_project_entities() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(
        project.path().join("still.png"),
        b"not an image decoder fixture",
    )
    .expect("write image candidate");
    fs::write(project.path().join("clip.mp4"), b"video candidate").expect("write video candidate");
    let outside = tempfile::NamedTempFile::new().expect("create outside file");

    let (runtime, image, video) = scanned_runtime(project.path()).await;
    let image = image.expect("image should be indexed");
    let video = video.expect("video should be indexed");

    assert_eq!(
        runtime
            .resolve_video_entity(image)
            .await
            .unwrap_err()
            .code(),
        "not_video"
    );
    assert_eq!(
        runtime
            .resolve_video_entity(EntityId::from_u128(u128::MAX))
            .await
            .unwrap_err()
            .code(),
        "stale_session"
    );

    fs::remove_file(project.path().join("clip.mp4")).expect("remove indexed video");
    std::os::unix::fs::symlink(outside.path(), project.path().join("clip.mp4"))
        .expect("replace video with escaping symlink");
    assert_eq!(
        runtime
            .resolve_video_entity(video)
            .await
            .unwrap_err()
            .code(),
        "path_not_authorized"
    );
}

#[tokio::test]
async fn resolved_video_source_is_canonical_and_carries_indexed_metadata() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(project.path().join("clip.mp4"), b"video candidate").expect("write video candidate");
    let (runtime, _, video) = scanned_runtime(project.path()).await;
    let video = video.expect("video should be indexed");

    let source = runtime
        .resolve_video_entity(video)
        .await
        .expect("resolve indexed video");

    assert_eq!(source.entity_id, video);
    assert_eq!(
        source.canonical_path,
        project.path().join("clip.mp4").canonicalize().unwrap()
    );
    assert!(source.canonical_path.is_absolute());
    assert_eq!(
        source.session_id.to_string(),
        runtime.snapshot().await.unwrap().session_id
    );
}
