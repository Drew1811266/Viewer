use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderCommandDto {
    pub session_id: u64,
    pub asset_generation: u64,
    pub scene_revision: u64,
    pub command_id: u64,
    pub command: ImageRenderCommandKindDto,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ImageRenderCommandKindDto {
    Open {
        entity_id: String,
    },
    Close,
    SetSurface {
        surface: ImageRenderSurfaceDto,
    },
    SetInputExclusions {
        exclusions: Vec<ImageRenderRectDto>,
    },
    SetTool {
        tool: ImageRenderToolDto,
    },
    SetScene {
        scene: ImageRenderSceneDto,
    },
    ApplyScenePatch {
        patch: ImageRenderScenePatchDto,
    },
    Camera {
        camera: ImageRenderCameraDto,
    },
    SetMagnifier {
        magnifier: Option<ImageRenderMagnifierDto>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderSurfaceDto {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub scale_factor: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderRectDto {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderPointDto {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageRenderToolDto {
    Browse,
    Point,
    Arrow,
    Brush,
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageRenderCameraModeDto {
    Fit,
    Free,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageRenderRotationDto {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderCameraDto {
    pub mode: ImageRenderCameraModeDto,
    pub zoom: f64,
    pub rotation: ImageRenderRotationDto,
    pub offset: ImageRenderPointDto,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageRenderMagnifierShapeDto {
    Circle,
    RoundedRectangle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderMagnifierDto {
    pub width_px: f64,
    pub height_px: f64,
    pub magnification: f64,
    pub shape: ImageRenderMagnifierShapeDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderNormalizedRectDto {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ImageRenderAnnotationGeometryDto {
    Point {
        position: ImageRenderPointDto,
    },
    Arrow {
        tail: ImageRenderPointDto,
        head: ImageRenderPointDto,
    },
    Rectangle {
        rect: ImageRenderNormalizedRectDto,
    },
    Ellipse {
        rect: ImageRenderNormalizedRectDto,
    },
    Stroke {
        points: Vec<ImageRenderPointDto>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderAnnotationStyleDto {
    pub color: [f32; 4],
    pub line_width_px: f32,
    pub dashed: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderAnnotationDto {
    pub id: String,
    pub ordinal: u32,
    pub geometry: ImageRenderAnnotationGeometryDto,
    pub style: ImageRenderAnnotationStyleDto,
    pub selected: bool,
    pub draft: bool,
    pub visible: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRenderSceneDto {
    pub annotations: Vec<ImageRenderAnnotationDto>,
    pub draft: Option<ImageRenderAnnotationDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ImageRenderScenePatchDto {
    Upsert {
        base_revision: u64,
        node: ImageRenderAnnotationDto,
    },
    Remove {
        base_revision: u64,
        id: String,
    },
    ReplaceAll {
        base_revision: u64,
        annotations: Vec<ImageRenderAnnotationDto>,
        draft: Option<ImageRenderAnnotationDto>,
    },
    SetSelection {
        base_revision: u64,
        id: Option<String>,
    },
    SetDraft {
        base_revision: u64,
        node: Option<ImageRenderAnnotationDto>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageRenderDispositionDto {
    Applied,
    IgnoredDuplicate,
    IgnoredStale,
    RequireSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageRenderBackendDto {
    Native,
    Web,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRenderAckDto {
    pub disposition: ImageRenderDispositionDto,
    pub accepted_revision: u64,
    pub backend: ImageRenderBackendDto,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ImageRenderEventDto {
    Ready {
        session_id: u64,
        asset_generation: u64,
        width: u32,
        height: u32,
    },
    FramePresented {
        session_id: u64,
        asset_generation: u64,
        scene_revision: u64,
        frame_index: u64,
    },
    CameraChanged {
        session_id: u64,
        asset_generation: u64,
        camera: ImageRenderCameraDto,
    },
    DraftStarted {
        session_id: u64,
        asset_generation: u64,
        geometry: ImageRenderAnnotationGeometryDto,
    },
    DraftChanged {
        session_id: u64,
        asset_generation: u64,
        geometry: ImageRenderAnnotationGeometryDto,
    },
    DraftCompleted {
        session_id: u64,
        asset_generation: u64,
        geometry: ImageRenderAnnotationGeometryDto,
    },
    DraftCancelled {
        session_id: u64,
        asset_generation: u64,
    },
    SelectionChanged {
        session_id: u64,
        asset_generation: u64,
        annotation_id: Option<String>,
    },
    EditorPlacementChanged {
        session_id: u64,
        asset_generation: u64,
        position: ImageRenderPointDto,
    },
    Recovering {
        session_id: u64,
        asset_generation: u64,
        reason: String,
    },
    Failed {
        session_id: u64,
        asset_generation: u64,
        code: String,
        retryable: bool,
    },
}
