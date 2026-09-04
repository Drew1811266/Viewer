use std::{fs, path::Path, sync::Arc};

use async_trait::async_trait;
use viewer_application::{ProjectAccess, ProjectProbeError, ProjectProbePort};
use viewer_desktop::dto::{
    FolderWorkspaceDto, ImageRenderCommandDto, ImageRenderCommandKindDto,
    ImageRenderDispositionDto, ImageRenderSceneDto, ImageRenderScenePatchDto,
};
use viewer_desktop::image_render_events::RecordingImageRenderEvents;
use viewer_desktop::image_render_runtime::{
    AuthorizedImageRenderCommand, ImageRenderDriver, ImageRenderRuntime, ImageRenderRuntimeError,
    ImageSourceAuthorizer,
};
use viewer_desktop::state::DesktopRuntime;
use viewer_domain::EntityId;
use viewer_platform_macos::image_render::AuthorizedImageSource;
use viewer_test_support::image_fixtures::image_fixture;

struct RejectingAuthorizer;

struct FixedProjectProbe;

impl ProjectProbePort for FixedProjectProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadWrite)
    }
}

#[async_trait]
impl ImageSourceAuthorizer for RejectingAuthorizer {
    async fn authorize_image(
        &self,
        _entity_id: EntityId,
    ) -> Result<AuthorizedImageSource, ImageRenderRuntimeError> {
        Err(ImageRenderRuntimeError::UnauthorizedEntity)
    }
}

#[derive(Default)]
struct NoopDriver;

impl ImageRenderDriver for NoopDriver {
    fn apply(&self, _command: AuthorizedImageRenderCommand) -> Result<(), ImageRenderRuntimeError> {
        Ok(())
    }

    fn close(&self) -> Result<(), ImageRenderRuntimeError> {
        Ok(())
    }
}

fn command(
    command_id: u64,
    generation: u64,
    revision: u64,
    command: ImageRenderCommandKindDto,
) -> ImageRenderCommandDto {
    ImageRenderCommandDto {
        session_id: 9,
        asset_generation: generation,
        scene_revision: revision,
        command_id,
        command,
    }
}

#[test]
fn renderer_command_schema_contains_no_path_url_or_source_path_escape_hatch() {
    let value = serde_json::json!({
        "sessionId": 9,
        "assetGeneration": 1,
        "sceneRevision": 0,
        "commandId": 1,
        "command": {
            "type": "open",
            "entityId": EntityId::from_u128(12).to_string()
        }
    });
    let parsed: ImageRenderCommandDto = serde_json::from_value(value.clone()).unwrap();
    let schema = format!("{parsed:?}").to_ascii_lowercase();
    assert!(!schema.contains("path"));
    assert!(!schema.contains("url"));

    for forbidden in ["path", "sourcePath", "url"] {
        let mut invalid = value.clone();
        invalid["command"][forbidden] = serde_json::json!("/tmp/escape.jpg");
        assert!(serde_json::from_value::<ImageRenderCommandDto>(invalid).is_err());
    }
}

#[tokio::test]
async fn unauthorized_entities_are_rejected_before_the_driver_receives_open() {
    let runtime = ImageRenderRuntime::with_ports(
        Arc::new(RejectingAuthorizer),
        Arc::new(NoopDriver),
        Arc::new(RecordingImageRenderEvents::default()),
    );
    let result = runtime
        .dispatch(command(
            1,
            1,
            0,
            ImageRenderCommandKindDto::Open {
                entity_id: EntityId::from_u128(99).to_string(),
            },
        ))
        .await;

    assert_eq!(
        result.unwrap_err(),
        ImageRenderRuntimeError::UnauthorizedEntity
    );
}

#[tokio::test]
async fn desktop_authorizer_only_grants_current_indexed_image_files() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::copy(image_fixture("srgb.jpg"), project.path().join("still.jpg")).unwrap();
    fs::write(project.path().join("notes.txt"), b"not an image").unwrap();
    let desktop = Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProjectProbe),
    ));
    desktop.open_project(project.path()).await.unwrap();
    desktop.wait_for_scan().await.unwrap();
    let FolderWorkspaceDto::Content {
        images,
        other_files,
        ..
    } = desktop.query_folder(None).await.unwrap()
    else {
        panic!("root should contain the test files")
    };
    let image_id: EntityId = images
        .iter()
        .find(|file| file.name == "still.jpg")
        .unwrap()
        .entity_id
        .parse()
        .unwrap();
    let text_id: EntityId = other_files
        .iter()
        .find(|file| file.name == "notes.txt")
        .unwrap()
        .entity_id
        .parse()
        .unwrap();
    let authorizer: Arc<dyn ImageSourceAuthorizer> = desktop.clone();
    let runtime = ImageRenderRuntime::with_ports(
        authorizer,
        Arc::new(NoopDriver),
        Arc::new(RecordingImageRenderEvents::default()),
    );

    let rejected = runtime
        .dispatch(command(
            1,
            1,
            0,
            ImageRenderCommandKindDto::Open {
                entity_id: text_id.to_string(),
            },
        ))
        .await;
    assert_eq!(
        rejected.unwrap_err(),
        ImageRenderRuntimeError::UnauthorizedEntity
    );

    let opened = runtime
        .dispatch(command(
            2,
            1,
            0,
            ImageRenderCommandKindDto::Open {
                entity_id: image_id.to_string(),
            },
        ))
        .await
        .unwrap();
    assert_eq!(opened.disposition, ImageRenderDispositionDto::Applied);
    runtime.close().await.unwrap();
    desktop.close_project().await.unwrap();
}

#[tokio::test]
async fn stale_generation_is_ignored_and_revision_gaps_require_a_snapshot() {
    struct FixtureAuthorizer;
    #[async_trait]
    impl ImageSourceAuthorizer for FixtureAuthorizer {
        async fn authorize_image(
            &self,
            _entity_id: EntityId,
        ) -> Result<AuthorizedImageSource, ImageRenderRuntimeError> {
            AuthorizedImageSource::authorize_for_process(image_fixture("srgb.jpg"))
                .map_err(|_| ImageRenderRuntimeError::UnauthorizedEntity)
        }
    }
    let runtime = ImageRenderRuntime::with_ports(
        Arc::new(FixtureAuthorizer),
        Arc::new(NoopDriver),
        Arc::new(RecordingImageRenderEvents::default()),
    );
    runtime
        .dispatch(command(
            1,
            2,
            0,
            ImageRenderCommandKindDto::Open {
                entity_id: EntityId::from_u128(12).to_string(),
            },
        ))
        .await
        .expect("authorized fixture opens");

    let stale = runtime
        .dispatch(command(
            2,
            1,
            0,
            ImageRenderCommandKindDto::SetScene {
                scene: ImageRenderSceneDto {
                    annotations: vec![],
                    draft: None,
                },
            },
        ))
        .await
        .unwrap();
    let gap = runtime
        .dispatch(command(
            3,
            2,
            2,
            ImageRenderCommandKindDto::ApplyScenePatch {
                patch: ImageRenderScenePatchDto::SetSelection {
                    base_revision: 0,
                    id: None,
                },
            },
        ))
        .await
        .unwrap();

    assert_eq!(stale.disposition, ImageRenderDispositionDto::IgnoredStale);
    assert_eq!(gap.disposition, ImageRenderDispositionDto::RequireSnapshot);
    assert_eq!(gap.accepted_revision, 0);
}
