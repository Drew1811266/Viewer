mod projection;
mod query;
mod schema;
mod writer;

use rusqlite::Connection;
use std::{str::FromStr, sync::Mutex};
use viewer_application::metadata::{IndexedNode, Marker};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode, ImageIndexStatus, ImageMetadata, ReviewState, TextIndexStatus},
    video::{VideoFailureKind, VideoMetadata, VideoProbeStatus},
};

#[derive(Debug, thiserror::Error)]
pub enum SessionIndexError {
    #[error("session index database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("parent {parent_path} must be indexed before {path}")]
    MissingParent { path: String, parent_path: String },
    #[error("file size cannot be represented in SQLite: {0}")]
    SizeOutOfRange(u64),
    #[error("text index node does not exist or its path changed: {entity_id} at {path}")]
    MissingTextNode { entity_id: EntityId, path: String },
    #[error("session index node does not exist: {0}")]
    MissingNode(EntityId),
    #[error("derived metadata does not match a current supported node: {0}")]
    InvalidDerivedMetadata(EntityId),
    #[error("search query is invalid")]
    InvalidSearchQuery,
    #[error("invalid persisted {field}: {value}")]
    InvalidPersistedValue { field: &'static str, value: String },
    #[error("reconcile snapshot is inconsistent with the requested project subtrees")]
    InvalidReconcile,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionReconcileSummary {
    pub added: u64,
    pub removed: u64,
    pub modified: u64,
    pub moved: u64,
}

pub struct SessionIndex {
    connection: Mutex<Connection>,
}

pub(super) fn encode_kind(kind: FileKind) -> i64 {
    kind.encode()
}

pub(super) fn encode_review_state(state: ReviewState) -> i64 {
    match state {
        ReviewState::Keep => 0,
        ReviewState::Pending => 1,
        ReviewState::Reject => 2,
    }
}

fn decode_review_state(value: i64) -> rusqlite::Result<ReviewState> {
    match value {
        0 => Ok(ReviewState::Keep),
        1 => Ok(ReviewState::Pending),
        2 => Ok(ReviewState::Reject),
        _ => Err(persisted_error("review_state", value)),
    }
}

fn decode_image_status(value: i64) -> rusqlite::Result<ImageIndexStatus> {
    match value {
        0 => Ok(ImageIndexStatus::Pending),
        1 => Ok(ImageIndexStatus::Ready),
        2 => Ok(ImageIndexStatus::Failed),
        _ => Err(persisted_error("image_status", value)),
    }
}

fn decode_text_status(value: i64) -> rusqlite::Result<TextIndexStatus> {
    match value {
        0 => Ok(TextIndexStatus::Pending),
        1 => Ok(TextIndexStatus::Ready),
        2 => Ok(TextIndexStatus::UnsupportedEncoding),
        3 => Ok(TextIndexStatus::TooLarge),
        4 => Ok(TextIndexStatus::Failed),
        _ => Err(persisted_error("text_status", value)),
    }
}

fn decode_video_failure(value: i64) -> rusqlite::Result<VideoFailureKind> {
    match value {
        0 => Ok(VideoFailureKind::Unsupported),
        1 => Ok(VideoFailureKind::Damaged),
        2 => Ok(VideoFailureKind::Unreadable),
        3 => Ok(VideoFailureKind::Missing),
        4 => Ok(VideoFailureKind::EngineInitialization),
        5 => Ok(VideoFailureKind::DecodeFallbackFailed),
        6 => Ok(VideoFailureKind::RenderSurface),
        7 => Ok(VideoFailureKind::ThumbnailUnavailable),
        _ => Err(persisted_error("video_failure_kind", value)),
    }
}

pub(super) fn decode_kind(value: i64) -> rusqlite::Result<FileKind> {
    match value {
        0 => Ok(FileKind::Directory),
        1 => Ok(FileKind::Jpeg),
        2 => Ok(FileKind::Png),
        3 => Ok(FileKind::Markdown),
        4 => Ok(FileKind::Text),
        5 => Ok(FileKind::UnsupportedImage),
        6 => Ok(FileKind::Other),
        7 => Ok(FileKind::Video),
        _ => Err(persisted_error("kind", value)),
    }
}

pub(super) fn read_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileNode> {
    let entity_id = parse_id(row.get(0)?, "entity_id")?;
    let relative_path = parse_path(row.get(1)?, "relative_path")?;
    let kind = decode_kind(row.get(2)?)?;
    let persisted_size = row.get::<_, i64>(3)?;
    let size =
        u64::try_from(persisted_size).map_err(|_| persisted_error("size", persisted_size))?;
    let persisted_modified = row.get::<_, String>(4)?;
    let modified_ns = persisted_modified
        .parse::<i128>()
        .map_err(|_| persisted_error("modified_ns", &persisted_modified))?;
    Ok(FileNode {
        entity_id,
        relative_path,
        kind,
        size,
        modified_ns,
    })
}

pub(super) fn read_indexed_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedNode> {
    let node = read_node(row)?;
    let review_state = row
        .get::<_, Option<i64>>(5)?
        .map(decode_review_state)
        .transpose()?;
    let width = row.get::<_, Option<i64>>(7)?;
    let height = row.get::<_, Option<i64>>(8)?;
    let image_metadata = match (width, height) {
        (Some(width), Some(height)) => Some(ImageMetadata {
            width: u32::try_from(width).map_err(|_| persisted_error("image_width", width))?,
            height: u32::try_from(height).map_err(|_| persisted_error("image_height", height))?,
        }),
        (None, None) => None,
        _ => return Err(persisted_error("image_dimensions", "partial")),
    };
    let video_metadata = read_video_metadata(row)?;
    Ok(IndexedNode {
        node,
        marker: Marker {
            review_state,
            favorite: row.get(6)?,
        },
        image_metadata,
        video_metadata,
        image_status: decode_image_status(row.get(9)?)?,
        text_status: decode_text_status(row.get(10)?)?,
    })
}

fn read_video_metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<VideoMetadata>> {
    let Some(probe_status) = row.get::<_, Option<i64>>(18)? else {
        return Ok(None);
    };
    let failure_kind = row.get::<_, Option<i64>>(19)?;
    let probe_status = match (probe_status, failure_kind) {
        (0, None) => VideoProbeStatus::Pending,
        (1, None) => VideoProbeStatus::Ready,
        (2, Some(failure)) => VideoProbeStatus::Failed(decode_video_failure(failure)?),
        (status, failure) => {
            return Err(persisted_error(
                "video_probe_status",
                format!("{status}/{failure:?}"),
            ));
        }
    };
    let duration_us = row
        .get::<_, Option<i64>>(11)?
        .map(|value| u64::try_from(value).map_err(|_| persisted_error("video_duration_us", value)))
        .transpose()?;
    let display_width = read_optional_u32(row, 12, "video_display_width")?;
    let display_height = read_optional_u32(row, 13, "video_display_height")?;
    let persisted_rotation = row.get::<_, i64>(14)?;
    let rotation_degrees = i16::try_from(persisted_rotation)
        .map_err(|_| persisted_error("video_rotation_degrees", persisted_rotation))?;
    let frame_rate_millihertz = read_optional_u32(row, 15, "video_frame_rate_millihertz")?;
    let persisted_generation = row.get::<_, i64>(20)?;
    u64::try_from(persisted_generation)
        .map_err(|_| persisted_error("video_updated_generation", persisted_generation))?;
    Ok(Some(VideoMetadata {
        duration_us,
        display_width,
        display_height,
        rotation_degrees,
        frame_rate_millihertz,
        video_codec: row.get(16)?,
        audio_codec: row.get(17)?,
        probe_status,
    }))
}

fn read_optional_u32(
    row: &rusqlite::Row<'_>,
    index: usize,
    field: &'static str,
) -> rusqlite::Result<Option<u32>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| u32::try_from(value).map_err(|_| persisted_error(field, value)))
        .transpose()
}

fn read_count(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, Option<i64>>(index)?.unwrap_or(0);
    u64::try_from(value).map_err(|_| persisted_error("progress_count", value))
}

fn parse_id<T>(value: String, field: &'static str) -> rusqlite::Result<T>
where
    T: FromStr,
{
    value
        .parse()
        .map_err(|_| persisted_error(field, value.as_str()))
}

fn parse_path(value: String, field: &'static str) -> rusqlite::Result<RelativePath> {
    RelativePath::parse(&value).map_err(|_| persisted_error(field, &value))
}

fn persisted_error(field: &'static str, value: impl ToString) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(SessionIndexError::InvalidPersistedValue {
            field,
            value: value.to_string(),
        }),
    )
}
