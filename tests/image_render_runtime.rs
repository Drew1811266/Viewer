use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{path::Path, sync::Weak};

use async_trait::async_trait;
use viewer_application::{ProjectAccess, ProjectProbeError, ProjectProbePort};
use viewer_desktop::dto::{
    ImageRenderCameraDto, ImageRenderCameraModeDto, ImageRenderCommandDto,
    ImageRenderCommandKindDto, ImageRenderDispositionDto, ImageRenderEventDto, ImageRenderPointDto,
    ImageRenderRotationDto, ImageRenderSceneDto, ImageRenderSurfaceDto,
};
use viewer_desktop::image_render_events::{ImageRenderEventPort, RecordingImageRenderEvents};
use viewer_desktop::image_render_runtime::{
    AuthorizedImageRenderCommand, ImageRenderDriver, ImageRenderRuntime, ImageRenderRuntimeError,
    ImageSourceAuthorizer,
};
use viewer_desktop::state::{DesktopRuntime, ImageRenderClosePort};
use viewer_domain::EntityId;
use viewer_platform_macos::image_render::AuthorizedImageSource;
use viewer_test_support::image_fixtures::image_fixture;

#[derive(Default)]
struct FixtureAuthorizer;

struct FixedProjectProbe;

impl ProjectProbePort for FixedProjectProbe {
    fn probe(&self, _root: &Path) -> Result<ProjectAccess, ProjectProbeError> {
        Ok(ProjectAccess::ReadWrite)
    }
}

struct SessionInspectingLifecycle {
    desktop: Weak<DesktopRuntime>,
}

#[async_trait]
impl ImageRenderClosePort for SessionInspectingLifecycle {
    async fn close_image_renderer(&self) -> Result<(), viewer_desktop::state::RuntimeError> {
        if let Some(desktop) = self.desktop.upgrade() {
            let _ = desktop.snapshot().await;
        }
        Ok(())
    }
}

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

#[derive(Default)]
struct RecordingDriver {
    commands: Mutex<Vec<&'static str>>,
    annotation_permissions: Mutex<Vec<bool>>,
}

#[derive(Default)]
struct FailFirstCloseDriver {
    failed_once: AtomicBool,
    close_attempts: Mutex<u32>,
}

impl ImageRenderDriver for FailFirstCloseDriver {
    fn apply(&self, _command: AuthorizedImageRenderCommand) -> Result<(), ImageRenderRuntimeError> {
        Ok(())
    }

    fn close(&self) -> Result<(), ImageRenderRuntimeError> {
        *self.close_attempts.lock().unwrap() += 1;
        if !self.failed_once.swap(true, Ordering::AcqRel) {
            Err(ImageRenderRuntimeError::DriverFailed)
        } else {
            Ok(())
        }
    }
}

impl RecordingDriver {
    fn commands(&self) -> Vec<&'static str> {
        self.commands.lock().unwrap().clone()
    }
}

impl ImageRenderDriver for RecordingDriver {
    fn apply(&self, command: AuthorizedImageRenderCommand) -> Result<(), ImageRenderRuntimeError> {
        let name = match command {
            AuthorizedImageRenderCommand::Open { .. } => "open",
            AuthorizedImageRenderCommand::SetSurface { .. } => "surface",
            AuthorizedImageRenderCommand::SetInputExclusions { .. } => "exclusions",
            AuthorizedImageRenderCommand::SetTool { .. } => "tool",
            AuthorizedImageRenderCommand::SetScene { scene } => {
                self.annotation_permissions
                    .lock()
                    .unwrap()
                    .push(scene.annotations_editable());
                "scene"
            }
            AuthorizedImageRenderCommand::Camera { .. } => "camera",
            AuthorizedImageRenderCommand::SetMagnifier { .. } => "magnifier",
        };
        self.commands.lock().unwrap().push(name);
        Ok(())
    }

    fn close(&self) -> Result<(), ImageRenderRuntimeError> {
        self.commands.lock().unwrap().push("close");
        Ok(())
    }
}

fn envelope(
    command_id: u64,
    revision: u64,
    command: ImageRenderCommandKindDto,
) -> ImageRenderCommandDto {
    ImageRenderCommandDto {
        session_id: 41,
        asset_generation: 1,
        scene_revision: revision,
        command_id,
        command,
    }
}

fn open(command_id: u64) -> ImageRenderCommandDto {
    envelope(
        command_id,
        0,
        ImageRenderCommandKindDto::Open {
            entity_id: EntityId::from_u128(7).to_string(),
        },
    )
}

#[tokio::test]
async fn open_resize_scene_and_close_are_ordered_and_retry_safe() {
    let driver = Arc::new(RecordingDriver::default());
    let event_recorder = Arc::new(RecordingImageRenderEvents::default());
    let events: Arc<dyn ImageRenderEventPort> = event_recorder;
    let runtime =
        ImageRenderRuntime::with_ports(Arc::new(FixtureAuthorizer), driver.clone(), events);

    let opened = runtime.dispatch(open(1)).await.unwrap();
    assert_eq!(opened.disposition, ImageRenderDispositionDto::Applied);

    let surface = envelope(
        2,
        0,
        ImageRenderCommandKindDto::SetSurface {
            surface: ImageRenderSurfaceDto {
                left: 20.0,
                top: 80.0,
                width: 900.0,
                height: 600.0,
                scale_factor: 2.0,
            },
        },
    );
    runtime.dispatch(surface).await.unwrap();

    let scene = envelope(
        3,
        1,
        ImageRenderCommandKindDto::SetScene {
            scene: ImageRenderSceneDto {
                annotations: vec![],
                draft: None,
                annotations_editable: true,
            },
        },
    );
    let applied = runtime.dispatch(scene.clone()).await.unwrap();
    let duplicate = runtime.dispatch(scene).await.unwrap();
    assert_eq!(applied.accepted_revision, 1);
    assert_eq!(
        duplicate.disposition,
        ImageRenderDispositionDto::IgnoredDuplicate
    );

    let closed = runtime
        .dispatch(envelope(4, 1, ImageRenderCommandKindDto::Close))
        .await
        .unwrap();
    let repeated_close = runtime
        .dispatch(envelope(4, 1, ImageRenderCommandKindDto::Close))
        .await
        .unwrap();
    assert_eq!(closed.disposition, ImageRenderDispositionDto::Applied);
    assert_eq!(
        repeated_close.disposition,
        ImageRenderDispositionDto::IgnoredDuplicate
    );
    assert_eq!(driver.commands(), vec!["open", "surface", "scene", "close"]);
}

#[tokio::test]
async fn readonly_scene_permission_reaches_the_native_driver_and_legacy_snapshots_keep_their_default()
 {
    let driver = Arc::new(RecordingDriver::default());
    let runtime = ImageRenderRuntime::with_ports(
        Arc::new(FixtureAuthorizer),
        driver.clone(),
        Arc::new(RecordingImageRenderEvents::default()),
    );
    runtime.dispatch(open(1)).await.unwrap();
    for (index, value) in [
        serde_json::json!({ "annotations": [], "draft": null, "annotationsEditable": false }),
        serde_json::json!({ "annotations": [], "draft": null }),
    ]
    .into_iter()
    .enumerate()
    {
        let scene: ImageRenderSceneDto = serde_json::from_value(value).unwrap();
        runtime
            .dispatch(envelope(
                index as u64 + 2,
                index as u64 + 1,
                ImageRenderCommandKindDto::SetScene { scene },
            ))
            .await
            .unwrap();
    }
    assert_eq!(
        *driver.annotation_permissions.lock().unwrap(),
        vec![false, true]
    );
}

#[tokio::test]
async fn switching_assets_cancels_the_prior_generation_and_rejects_late_commands() {
    let driver = Arc::new(RecordingDriver::default());
    let runtime = ImageRenderRuntime::with_ports(
        Arc::new(FixtureAuthorizer),
        driver.clone(),
        Arc::new(RecordingImageRenderEvents::default()),
    );
    runtime.dispatch(open(1)).await.unwrap();

    let mut next = open(2);
    next.asset_generation = 2;
    next.command = ImageRenderCommandKindDto::Open {
        entity_id: EntityId::from_u128(8).to_string(),
    };
    runtime.dispatch(next).await.unwrap();

    let late = runtime
        .dispatch(envelope(
            3,
            0,
            ImageRenderCommandKindDto::Camera {
                camera: ImageRenderCameraDto {
                    mode: ImageRenderCameraModeDto::Fit,
                    zoom: 1.0,
                    rotation: ImageRenderRotationDto::Deg0,
                    offset: ImageRenderPointDto { x: 0.0, y: 0.0 },
                },
            },
        ))
        .await
        .unwrap();

    assert_eq!(late.disposition, ImageRenderDispositionDto::IgnoredStale);
    assert_eq!(driver.commands(), vec!["open", "open"]);
}

#[tokio::test]
async fn lifecycle_close_is_idempotent() {
    let driver = Arc::new(RecordingDriver::default());
    let runtime = ImageRenderRuntime::with_ports(
        Arc::new(FixtureAuthorizer),
        driver.clone(),
        Arc::new(RecordingImageRenderEvents::default()),
    );
    runtime.dispatch(open(1)).await.unwrap();

    runtime.close().await.unwrap();
    runtime.close().await.unwrap();

    assert_eq!(driver.commands(), vec!["open", "close"]);
}

#[tokio::test]
async fn lifecycle_close_failure_keeps_the_session_retryable() {
    let driver = Arc::new(FailFirstCloseDriver::default());
    let runtime = ImageRenderRuntime::with_ports(
        Arc::new(FixtureAuthorizer),
        driver.clone(),
        Arc::new(RecordingImageRenderEvents::default()),
    );
    runtime.dispatch(open(1)).await.unwrap();

    assert_eq!(
        runtime.close().await,
        Err(ImageRenderRuntimeError::DriverFailed)
    );
    runtime.close().await.unwrap();

    assert_eq!(*driver.close_attempts.lock().unwrap(), 2);
}

#[tokio::test]
async fn project_close_stops_the_image_renderer_before_session_cleanup_finishes() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let desktop = Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProjectProbe),
    ));
    desktop.open_project(project.path()).await.unwrap();

    let driver = Arc::new(RecordingDriver::default());
    let image_runtime = Arc::new(ImageRenderRuntime::with_ports(
        Arc::new(FixtureAuthorizer),
        driver.clone(),
        Arc::new(RecordingImageRenderEvents::default()),
    ));
    image_runtime.dispatch(open(1)).await.unwrap();
    let lifecycle: Arc<dyn ImageRenderClosePort> = image_runtime.clone();
    let weak: Weak<dyn ImageRenderClosePort> = Arc::downgrade(&lifecycle);
    desktop.register_image_render_lifecycle(weak);

    desktop.close_project().await.unwrap();

    assert_eq!(driver.commands(), vec!["open", "close"]);
    assert_eq!(desktop.snapshot().await, None);
}

#[tokio::test]
async fn committed_project_close_retries_a_failed_renderer_cleanup_without_a_session() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let desktop = Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProjectProbe),
    ));
    desktop.open_project(project.path()).await.unwrap();

    let driver = Arc::new(FailFirstCloseDriver::default());
    let image_runtime = Arc::new(ImageRenderRuntime::with_ports(
        Arc::new(FixtureAuthorizer),
        driver.clone(),
        Arc::new(RecordingImageRenderEvents::default()),
    ));
    image_runtime.dispatch(open(1)).await.unwrap();
    let lifecycle: Arc<dyn ImageRenderClosePort> = image_runtime;
    desktop.register_image_render_lifecycle(Arc::downgrade(&lifecycle));

    assert_eq!(
        desktop.close_project().await.unwrap_err().code,
        "image_render_close_failed"
    );
    assert_eq!(desktop.snapshot().await, None);
    desktop.close_project().await.unwrap();

    assert_eq!(*driver.close_attempts.lock().unwrap(), 2);
}

#[tokio::test]
async fn project_open_never_holds_the_session_lock_across_renderer_cleanup() {
    let cache = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let desktop = Arc::new(DesktopRuntime::new(
        cache.path().to_path_buf(),
        Arc::new(FixedProjectProbe),
    ));
    let lifecycle: Arc<dyn ImageRenderClosePort> = Arc::new(SessionInspectingLifecycle {
        desktop: Arc::downgrade(&desktop),
    });
    desktop.register_image_render_lifecycle(Arc::downgrade(&lifecycle));

    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        desktop.open_project(project.path()),
    )
    .await
    .expect("project open must not deadlock")
    .unwrap();
    desktop.close_project().await.unwrap();
}

#[test]
fn renderer_events_are_semantic_camel_case_messages_without_raw_input() {
    let event = ImageRenderEventDto::CameraChanged {
        session_id: 41,
        asset_generation: 7,
        camera: ImageRenderCameraDto {
            mode: ImageRenderCameraModeDto::Free,
            zoom: 2.0,
            rotation: ImageRenderRotationDto::Deg90,
            offset: ImageRenderPointDto { x: 4.0, y: -3.0 },
        },
    };

    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["type"], "camera_changed");
    assert_eq!(value["sessionId"], 41);
    assert_eq!(value["assetGeneration"], 7);
    assert!(value.get("session_id").is_none());
    let encoded = value.to_string().to_ascii_lowercase();
    assert!(!encoded.contains("pointer"));
    assert!(!encoded.contains("scroll"));
    assert!(!encoded.contains("magnify"));
}
