use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::Mutex;
use viewer_domain::EntityId;
use viewer_platform_macos::image_render::{
    AuthorizedImageSource, InputExclusionRect, SurfaceLayout,
};
use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, AnnotationStyle, AssetGeneration, CameraMode,
    CameraState, CommandId, InteractionMode, LogicalPoint, NormalizedPoint, NormalizedRect,
    RenderCommand, RenderEnvelope, RenderSessionId, RevisionDecision, RevisionGate, Rotation,
    ScenePatch, SceneRevision, SceneSnapshot,
};
use viewer_render_wgpu::{MagnifierConfig, MagnifierShape};

use crate::dto::{
    ImageRenderAckDto, ImageRenderAnnotationDto, ImageRenderAnnotationGeometryDto,
    ImageRenderBackendDto, ImageRenderCameraDto, ImageRenderCameraModeDto, ImageRenderCommandDto,
    ImageRenderCommandKindDto, ImageRenderDispositionDto, ImageRenderMagnifierDto,
    ImageRenderMagnifierShapeDto, ImageRenderPointDto, ImageRenderRectDto, ImageRenderRotationDto,
    ImageRenderSceneDto, ImageRenderScenePatchDto, ImageRenderSurfaceDto, ImageRenderToolDto,
};
use crate::image_render_events::ImageRenderEventPort;

#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
pub use native::NativeImageRenderDriver;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ImageRenderRuntimeError {
    #[error("image render command envelope is invalid")]
    InvalidEnvelope,
    #[error("image render entity id is invalid")]
    InvalidEntity,
    #[error("image entity is not authorized by the active project session")]
    UnauthorizedEntity,
    #[error("image render command payload is invalid")]
    InvalidCommand,
    #[error("native image render driver is unavailable")]
    DriverUnavailable,
    #[error("native image render driver failed")]
    DriverFailed,
}

#[async_trait]
pub trait ImageSourceAuthorizer: Send + Sync {
    async fn authorize_image(
        &self,
        entity_id: EntityId,
    ) -> Result<AuthorizedImageSource, ImageRenderRuntimeError>;
}

#[derive(Clone)]
pub enum AuthorizedImageRenderCommand {
    Open {
        session_id: RenderSessionId,
        generation: AssetGeneration,
        source: AuthorizedImageSource,
    },
    SetSurface {
        layout: SurfaceLayout,
    },
    SetInputExclusions {
        exclusions: Vec<InputExclusionRect>,
    },
    SetTool {
        tool: InteractionMode,
    },
    SetScene {
        scene: SceneSnapshot,
    },
    Camera {
        camera: CameraState,
    },
    SetMagnifier {
        magnifier: Option<MagnifierConfig>,
    },
}

pub trait ImageRenderDriver: Send + Sync {
    fn bind_window(&self, _window: tauri::WebviewWindow) -> Result<(), ImageRenderRuntimeError> {
        Ok(())
    }

    fn apply(&self, command: AuthorizedImageRenderCommand) -> Result<(), ImageRenderRuntimeError>;

    fn close(&self) -> Result<(), ImageRenderRuntimeError>;
}

struct ActiveSession {
    id: RenderSessionId,
    gate: RevisionGate,
    scene: SceneSnapshot,
}

#[derive(Clone, Copy)]
struct ClosedSession {
    id: RenderSessionId,
    generation: AssetGeneration,
    revision: SceneRevision,
    command_id: CommandId,
}

#[derive(Default)]
struct RuntimeState {
    active: Option<ActiveSession>,
    last_closed: Option<ClosedSession>,
}

pub struct ImageRenderRuntime {
    authorizer: Arc<dyn ImageSourceAuthorizer>,
    driver: Arc<dyn ImageRenderDriver>,
    events: Arc<dyn ImageRenderEventPort>,
    state: Mutex<RuntimeState>,
}

impl ImageRenderRuntime {
    pub fn with_ports(
        authorizer: Arc<dyn ImageSourceAuthorizer>,
        driver: Arc<dyn ImageRenderDriver>,
        events: Arc<dyn ImageRenderEventPort>,
    ) -> Self {
        Self {
            authorizer,
            driver,
            events,
            state: Mutex::new(RuntimeState::default()),
        }
    }

    pub fn events(&self) -> &Arc<dyn ImageRenderEventPort> {
        &self.events
    }

    pub fn bind_window(&self, window: tauri::WebviewWindow) -> Result<(), ImageRenderRuntimeError> {
        self.driver.bind_window(window)
    }

    pub async fn dispatch(
        &self,
        command: ImageRenderCommandDto,
    ) -> Result<ImageRenderAckDto, ImageRenderRuntimeError> {
        self.dispatch_inner(command, None).await
    }

    pub async fn dispatch_from_window(
        &self,
        command: ImageRenderCommandDto,
        window: tauri::WebviewWindow,
    ) -> Result<ImageRenderAckDto, ImageRenderRuntimeError> {
        self.dispatch_inner(command, Some(window)).await
    }

    async fn dispatch_inner(
        &self,
        command: ImageRenderCommandDto,
        window: Option<tauri::WebviewWindow>,
    ) -> Result<ImageRenderAckDto, ImageRenderRuntimeError> {
        let envelope = EnvelopeIds::try_from(&command)?;
        let mut state = self.state.lock().await;
        let Some(active) = state.active.as_mut() else {
            return self
                .dispatch_without_active(&mut state, envelope, command, window.as_ref())
                .await;
        };

        if active.id != envelope.session_id {
            return Ok(ack(
                ImageRenderDispositionDto::IgnoredStale,
                active.gate.scene_revision(),
            ));
        }

        let core_payload = core_payload(&command.command, envelope.scene_revision)?;
        let core_envelope = RenderEnvelope {
            session_id: envelope.session_id,
            asset_generation: envelope.generation,
            scene_revision: envelope.scene_revision,
            command_id: envelope.command_id,
            payload: core_payload,
        };
        let mut next_gate = active.gate.clone();
        match next_gate.classify(&core_envelope) {
            RevisionDecision::IgnoreDuplicate => Ok(ack(
                ImageRenderDispositionDto::IgnoredDuplicate,
                active.gate.scene_revision(),
            )),
            RevisionDecision::IgnoreStale => Ok(ack(
                ImageRenderDispositionDto::IgnoredStale,
                active.gate.scene_revision(),
            )),
            RevisionDecision::RequireSnapshot {
                expected_revision, ..
            } => Ok(ack(
                ImageRenderDispositionDto::RequireSnapshot,
                expected_revision,
            )),
            RevisionDecision::Apply => {
                if matches!(command.command, ImageRenderCommandKindDto::Close) {
                    self.bind_source_window(window.as_ref())?;
                    self.driver.close()?;
                    state.last_closed = Some(ClosedSession {
                        id: envelope.session_id,
                        generation: envelope.generation,
                        revision: active.gate.scene_revision(),
                        command_id: envelope.command_id,
                    });
                    state.active = None;
                    return Ok(ack(
                        ImageRenderDispositionDto::Applied,
                        envelope.scene_revision,
                    ));
                }

                let (authorized, next_scene) = self
                    .authorize_command(&command.command, envelope, &active.scene)
                    .await?;
                self.bind_source_window(window.as_ref())?;
                self.driver.apply(authorized)?;
                active.gate = next_gate;
                active.scene = next_scene;
                Ok(ack(
                    ImageRenderDispositionDto::Applied,
                    active.gate.scene_revision(),
                ))
            }
        }
    }

    async fn dispatch_without_active(
        &self,
        state: &mut RuntimeState,
        envelope: EnvelopeIds,
        command: ImageRenderCommandDto,
        window: Option<&tauri::WebviewWindow>,
    ) -> Result<ImageRenderAckDto, ImageRenderRuntimeError> {
        if let Some(closed) = state.last_closed
            && closed.id == envelope.session_id
            && closed.generation == envelope.generation
            && closed.revision == envelope.scene_revision
            && closed.command_id == envelope.command_id
            && matches!(command.command, ImageRenderCommandKindDto::Close)
        {
            return Ok(ack(
                ImageRenderDispositionDto::IgnoredDuplicate,
                closed.revision,
            ));
        }
        if !matches!(command.command, ImageRenderCommandKindDto::Open { .. }) {
            return Ok(ack(
                ImageRenderDispositionDto::IgnoredStale,
                SceneRevision(0),
            ));
        }
        if envelope.generation.0 == 0 || envelope.scene_revision != SceneRevision(0) {
            return Err(ImageRenderRuntimeError::InvalidEnvelope);
        }

        let mut gate = RevisionGate::new(envelope.session_id, AssetGeneration(0));
        let core = RenderEnvelope {
            session_id: envelope.session_id,
            asset_generation: envelope.generation,
            scene_revision: envelope.scene_revision,
            command_id: envelope.command_id,
            payload: core_payload(&command.command, envelope.scene_revision)?,
        };
        if gate.classify(&core) != RevisionDecision::Apply {
            return Err(ImageRenderRuntimeError::InvalidEnvelope);
        }
        let empty = SceneSnapshot::empty(SceneRevision(0));
        let (authorized, scene) = self
            .authorize_command(&command.command, envelope, &empty)
            .await?;
        self.bind_source_window(window)?;
        self.driver.apply(authorized)?;
        state.active = Some(ActiveSession {
            id: envelope.session_id,
            gate,
            scene,
        });
        state.last_closed = None;
        Ok(ack(ImageRenderDispositionDto::Applied, SceneRevision(0)))
    }

    fn bind_source_window(
        &self,
        window: Option<&tauri::WebviewWindow>,
    ) -> Result<(), ImageRenderRuntimeError> {
        if let Some(window) = window {
            self.driver.bind_window(window.clone())?;
        }
        Ok(())
    }

    async fn authorize_command(
        &self,
        command: &ImageRenderCommandKindDto,
        envelope: EnvelopeIds,
        current_scene: &SceneSnapshot,
    ) -> Result<(AuthorizedImageRenderCommand, SceneSnapshot), ImageRenderRuntimeError> {
        let unchanged = || current_scene.clone();
        match command {
            ImageRenderCommandKindDto::Open { entity_id } => {
                let entity_id = EntityId::from_str(entity_id)
                    .map_err(|_| ImageRenderRuntimeError::InvalidEntity)?;
                let source = self.authorizer.authorize_image(entity_id).await?;
                Ok((
                    AuthorizedImageRenderCommand::Open {
                        session_id: envelope.session_id,
                        generation: envelope.generation,
                        source,
                    },
                    SceneSnapshot::empty(SceneRevision(0)),
                ))
            }
            ImageRenderCommandKindDto::Close => Err(ImageRenderRuntimeError::InvalidCommand),
            ImageRenderCommandKindDto::SetSurface { surface } => Ok((
                AuthorizedImageRenderCommand::SetSurface {
                    layout: surface_layout(*surface)?,
                },
                unchanged(),
            )),
            ImageRenderCommandKindDto::SetInputExclusions { exclusions } => Ok((
                AuthorizedImageRenderCommand::SetInputExclusions {
                    exclusions: exclusions
                        .iter()
                        .copied()
                        .map(input_exclusion)
                        .collect::<Result<Vec<_>, _>>()?,
                },
                unchanged(),
            )),
            ImageRenderCommandKindDto::SetTool { tool } => Ok((
                AuthorizedImageRenderCommand::SetTool {
                    tool: interaction_mode(*tool),
                },
                unchanged(),
            )),
            ImageRenderCommandKindDto::SetScene { scene } => {
                let scene = scene_snapshot(scene, envelope.scene_revision)?;
                Ok((
                    AuthorizedImageRenderCommand::SetScene {
                        scene: scene.clone(),
                    },
                    scene,
                ))
            }
            ImageRenderCommandKindDto::ApplyScenePatch { patch } => {
                let patch = scene_patch(patch, envelope.scene_revision)?;
                let mut scene = current_scene.clone();
                scene
                    .apply_patch(&patch)
                    .map_err(|_| ImageRenderRuntimeError::InvalidCommand)?;
                Ok((
                    AuthorizedImageRenderCommand::SetScene {
                        scene: scene.clone(),
                    },
                    scene,
                ))
            }
            ImageRenderCommandKindDto::Camera { camera } => Ok((
                AuthorizedImageRenderCommand::Camera {
                    camera: camera_state(*camera)?,
                },
                unchanged(),
            )),
            ImageRenderCommandKindDto::SetMagnifier { magnifier } => Ok((
                AuthorizedImageRenderCommand::SetMagnifier {
                    magnifier: magnifier.map(magnifier_config).transpose()?,
                },
                unchanged(),
            )),
        }
    }

    pub async fn close(&self) -> Result<(), ImageRenderRuntimeError> {
        let mut state = self.state.lock().await;
        if state.active.is_some() {
            self.driver.close()?;
            state.active = None;
        }
        Ok(())
    }
}

#[async_trait]
impl ImageSourceAuthorizer for crate::state::DesktopRuntime {
    async fn authorize_image(
        &self,
        entity_id: EntityId,
    ) -> Result<AuthorizedImageSource, ImageRenderRuntimeError> {
        self.authorized_image_source(entity_id)
            .await
            .map_err(|_| ImageRenderRuntimeError::UnauthorizedEntity)
    }
}

#[async_trait]
impl crate::state::ImageRenderClosePort for ImageRenderRuntime {
    async fn close_image_renderer(&self) -> Result<(), crate::state::RuntimeError> {
        self.close()
            .await
            .map_err(|_| crate::state::RuntimeError::ImageRenderCloseFailed)
    }
}

#[derive(Clone, Copy)]
struct EnvelopeIds {
    session_id: RenderSessionId,
    generation: AssetGeneration,
    scene_revision: SceneRevision,
    command_id: CommandId,
}

impl TryFrom<&ImageRenderCommandDto> for EnvelopeIds {
    type Error = ImageRenderRuntimeError;

    fn try_from(value: &ImageRenderCommandDto) -> Result<Self, Self::Error> {
        if value.session_id == 0 || value.command_id == 0 {
            return Err(ImageRenderRuntimeError::InvalidEnvelope);
        }
        Ok(Self {
            session_id: RenderSessionId(value.session_id),
            generation: AssetGeneration(value.asset_generation),
            scene_revision: SceneRevision(value.scene_revision),
            command_id: CommandId(value.command_id),
        })
    }
}

fn ack(disposition: ImageRenderDispositionDto, revision: SceneRevision) -> ImageRenderAckDto {
    ImageRenderAckDto {
        disposition,
        accepted_revision: revision.0,
        backend: ImageRenderBackendDto::Native,
    }
}

fn core_payload(
    command: &ImageRenderCommandKindDto,
    revision: SceneRevision,
) -> Result<RenderCommand, ImageRenderRuntimeError> {
    match command {
        ImageRenderCommandKindDto::Open { entity_id } => Ok(RenderCommand::OpenAsset {
            entity_id: entity_id.clone(),
        }),
        ImageRenderCommandKindDto::SetScene { scene } => {
            Ok(RenderCommand::SetScene(scene_snapshot(scene, revision)?))
        }
        ImageRenderCommandKindDto::ApplyScenePatch { patch } => Ok(RenderCommand::ApplyScenePatch(
            scene_patch(patch, revision)?,
        )),
        ImageRenderCommandKindDto::Camera { camera } => {
            Ok(RenderCommand::SetCamera(camera_state(*camera)?))
        }
        ImageRenderCommandKindDto::Close
        | ImageRenderCommandKindDto::SetSurface { .. }
        | ImageRenderCommandKindDto::SetInputExclusions { .. }
        | ImageRenderCommandKindDto::SetTool { .. }
        | ImageRenderCommandKindDto::SetMagnifier { .. } => Ok(RenderCommand::Noop),
    }
}

fn surface_layout(dto: ImageRenderSurfaceDto) -> Result<SurfaceLayout, ImageRenderRuntimeError> {
    if [dto.left, dto.top, dto.width, dto.height, dto.scale_factor]
        .iter()
        .any(|value| !value.is_finite())
        || dto.left < 0.0
        || dto.top < 0.0
        || dto.width <= 0.0
        || dto.height <= 0.0
        || dto.scale_factor <= 0.0
    {
        return Err(ImageRenderRuntimeError::InvalidCommand);
    }
    Ok(SurfaceLayout {
        left: dto.left,
        top: dto.top,
        width: dto.width,
        height: dto.height,
        scale_factor: dto.scale_factor,
    })
}

fn input_exclusion(dto: ImageRenderRectDto) -> Result<InputExclusionRect, ImageRenderRuntimeError> {
    InputExclusionRect::new(dto.left, dto.top, dto.width, dto.height)
        .map_err(|_| ImageRenderRuntimeError::InvalidCommand)
}

const fn interaction_mode(dto: ImageRenderToolDto) -> InteractionMode {
    match dto {
        ImageRenderToolDto::Browse => InteractionMode::Browse,
        ImageRenderToolDto::Point => InteractionMode::Point,
        ImageRenderToolDto::Arrow => InteractionMode::Arrow,
        ImageRenderToolDto::Brush => InteractionMode::Brush,
        ImageRenderToolDto::Rectangle => InteractionMode::Rectangle,
        ImageRenderToolDto::Ellipse => InteractionMode::Ellipse,
    }
}

fn camera_state(dto: ImageRenderCameraDto) -> Result<CameraState, ImageRenderRuntimeError> {
    if !dto.zoom.is_finite()
        || !(viewer_render_core::MIN_PREVIEW_ZOOM..=viewer_render_core::MAX_PREVIEW_ZOOM)
            .contains(&dto.zoom)
    {
        return Err(ImageRenderRuntimeError::InvalidCommand);
    }
    Ok(CameraState {
        mode: match dto.mode {
            ImageRenderCameraModeDto::Fit => CameraMode::Fit,
            ImageRenderCameraModeDto::Free => CameraMode::Free,
        },
        zoom: dto.zoom,
        rotation: rotation(dto.rotation),
        offset: logical_point(dto.offset)?,
    })
}

const fn rotation(dto: ImageRenderRotationDto) -> Rotation {
    match dto {
        ImageRenderRotationDto::Deg0 => Rotation::Deg0,
        ImageRenderRotationDto::Deg90 => Rotation::Deg90,
        ImageRenderRotationDto::Deg180 => Rotation::Deg180,
        ImageRenderRotationDto::Deg270 => Rotation::Deg270,
    }
}

fn magnifier_config(
    dto: ImageRenderMagnifierDto,
) -> Result<MagnifierConfig, ImageRenderRuntimeError> {
    MagnifierConfig::new(
        normalized_point(dto.focus)?,
        logical_point(dto.center)?,
        dto.diameter_px,
        dto.magnification,
        match dto.shape {
            ImageRenderMagnifierShapeDto::Circle => MagnifierShape::Circle,
            ImageRenderMagnifierShapeDto::RoundedRectangle => MagnifierShape::RoundedRectangle,
        },
    )
    .map_err(|_| ImageRenderRuntimeError::InvalidCommand)
}

fn scene_snapshot(
    dto: &ImageRenderSceneDto,
    revision: SceneRevision,
) -> Result<SceneSnapshot, ImageRenderRuntimeError> {
    SceneSnapshot::new(
        revision,
        dto.annotations
            .iter()
            .map(annotation_node)
            .collect::<Result<Vec<_>, _>>()?,
        dto.draft.as_ref().map(annotation_node).transpose()?,
    )
    .map_err(|_| ImageRenderRuntimeError::InvalidCommand)
}

fn scene_patch(
    dto: &ImageRenderScenePatchDto,
    revision: SceneRevision,
) -> Result<ScenePatch, ImageRenderRuntimeError> {
    Ok(match dto {
        ImageRenderScenePatchDto::Upsert {
            base_revision,
            node,
        } => ScenePatch::Upsert {
            base_revision: SceneRevision(*base_revision),
            revision,
            node: annotation_node(node)?,
        },
        ImageRenderScenePatchDto::Remove { base_revision, id } => ScenePatch::Remove {
            base_revision: SceneRevision(*base_revision),
            revision,
            id: annotation_id(id)?,
        },
        ImageRenderScenePatchDto::ReplaceAll {
            base_revision,
            annotations,
            draft,
        } => ScenePatch::ReplaceAll {
            base_revision: SceneRevision(*base_revision),
            revision,
            annotations: annotations
                .iter()
                .map(annotation_node)
                .collect::<Result<Vec<_>, _>>()?,
            draft: draft.as_ref().map(annotation_node).transpose()?,
        },
        ImageRenderScenePatchDto::SetSelection { base_revision, id } => ScenePatch::SetSelection {
            base_revision: SceneRevision(*base_revision),
            revision,
            id: id.as_deref().map(annotation_id).transpose()?,
        },
        ImageRenderScenePatchDto::SetDraft {
            base_revision,
            node,
        } => ScenePatch::SetDraft {
            base_revision: SceneRevision(*base_revision),
            revision,
            node: node.as_ref().map(annotation_node).transpose()?,
        },
    })
}

fn annotation_node(
    dto: &ImageRenderAnnotationDto,
) -> Result<AnnotationNode, ImageRenderRuntimeError> {
    if dto.style.color.iter().any(|value| !value.is_finite())
        || !dto.style.line_width_px.is_finite()
        || dto.style.line_width_px <= 0.0
    {
        return Err(ImageRenderRuntimeError::InvalidCommand);
    }
    let mut node = AnnotationNode::new(
        annotation_id(&dto.id)?,
        dto.ordinal,
        annotation_geometry(&dto.geometry)?,
    )
    .map_err(|_| ImageRenderRuntimeError::InvalidCommand)?;
    node.style = AnnotationStyle {
        color: dto.style.color,
        line_width_px: dto.style.line_width_px,
        dashed: dto.style.dashed,
    };
    node.selected = dto.selected;
    node.draft = dto.draft;
    node.visible = dto.visible;
    Ok(node)
}

fn annotation_geometry(
    dto: &ImageRenderAnnotationGeometryDto,
) -> Result<AnnotationGeometry, ImageRenderRuntimeError> {
    Ok(match dto {
        ImageRenderAnnotationGeometryDto::Point { position } => AnnotationGeometry::Point {
            position: normalized_point(*position)?,
        },
        ImageRenderAnnotationGeometryDto::Arrow { tail, head } => AnnotationGeometry::Arrow {
            tail: normalized_point(*tail)?,
            head: normalized_point(*head)?,
        },
        ImageRenderAnnotationGeometryDto::Rectangle { rect } => AnnotationGeometry::Rectangle {
            rect: NormalizedRect::new(rect.x, rect.y, rect.width, rect.height)
                .map_err(|_| ImageRenderRuntimeError::InvalidCommand)?,
        },
        ImageRenderAnnotationGeometryDto::Ellipse { rect } => AnnotationGeometry::Ellipse {
            rect: NormalizedRect::new(rect.x, rect.y, rect.width, rect.height)
                .map_err(|_| ImageRenderRuntimeError::InvalidCommand)?,
        },
        ImageRenderAnnotationGeometryDto::Stroke { points } => AnnotationGeometry::Stroke {
            points: points
                .iter()
                .copied()
                .map(normalized_point)
                .collect::<Result<Vec<_>, _>>()?,
        },
    })
}

fn annotation_id(value: &str) -> Result<AnnotationId, ImageRenderRuntimeError> {
    AnnotationId::new(value).map_err(|_| ImageRenderRuntimeError::InvalidCommand)
}

fn normalized_point(dto: ImageRenderPointDto) -> Result<NormalizedPoint, ImageRenderRuntimeError> {
    NormalizedPoint::new(dto.x, dto.y).map_err(|_| ImageRenderRuntimeError::InvalidCommand)
}

fn logical_point(dto: ImageRenderPointDto) -> Result<LogicalPoint, ImageRenderRuntimeError> {
    LogicalPoint::new(dto.x, dto.y).map_err(|_| ImageRenderRuntimeError::InvalidCommand)
}
