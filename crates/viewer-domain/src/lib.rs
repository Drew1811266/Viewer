pub mod file;
pub mod image;
pub mod operation;
pub mod search;
pub mod video;

use serde::{Deserialize, Deserializer, Serialize};
use std::{
    fmt,
    path::{Component, Path},
};
use thiserror::Error;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub const fn from_u128(value: u128) -> Self {
                Self(Uuid::from_u128(value))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

id_type!(ProjectId);
id_type!(SessionId);
id_type!(EntityId);
id_type!(ImageRequestId);
id_type!(VideoSessionId);
id_type!(VideoThumbnailRequestId);
id_type!(TaskId);
id_type!(OperationId);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct RelativePath(String);

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RelativePathError {
    #[error("path must be a non-empty project-relative path")]
    Invalid,
}

impl RelativePath {
    pub fn parse(value: &str) -> Result<Self, RelativePathError> {
        let path = Path::new(value);
        let segments_are_canonical = !value.contains('\0')
            && value.split('/').all(|segment| {
                !matches!(segment, "" | "." | "..") && !segment.eq_ignore_ascii_case(".viewer")
            });
        let valid = segments_are_canonical
            && !path.is_absolute()
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_)));
        valid
            .then(|| Self(value.to_owned()))
            .ok_or(RelativePathError::Invalid)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{
        Deserialize,
        de::value::{Error as ValueError, StrDeserializer},
    };
    use std::str::FromStr;

    #[test]
    fn relative_path_accepts_a_canonical_project_path() {
        let path = RelativePath::parse("products/id-1/front.png").unwrap();
        assert_eq!(path.as_str(), "products/id-1/front.png");
    }

    #[test]
    fn relative_path_rejects_non_canonical_or_reserved_paths() {
        for invalid in [
            "",
            "../outside",
            "/absolute",
            "./front.png",
            "products/./front.png",
            "products/../front.png",
            ".viewer/metadata.sqlite",
            ".VIEWER/metadata.sqlite",
            ".Viewer/metadata.sqlite",
            "products/.viewer/metadata.sqlite",
            "products/.vIeWeR/metadata.sqlite",
            "products/id-1/front\0.png",
            "products//front.png",
            "products/front.png/",
        ] {
            assert!(RelativePath::parse(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn relative_path_deserialization_preserves_validation() {
        let invalid = StrDeserializer::<ValueError>::new("../outside");
        assert!(RelativePath::deserialize(invalid).is_err());

        for value in [".VIEWER/metadata.sqlite", "products/.Viewer/file", "a\0b"] {
            let invalid = StrDeserializer::<ValueError>::new(value);
            assert!(
                RelativePath::deserialize(invalid).is_err(),
                "accepted {value:?}"
            );
        }

        let valid = StrDeserializer::<ValueError>::new("products/id-1/front.png");
        assert_eq!(
            RelativePath::deserialize(valid).unwrap().as_str(),
            "products/id-1/front.png"
        );
    }

    #[test]
    fn id_display_and_parse_round_trip() {
        let id = ProjectId::new();
        assert_eq!(ProjectId::from_str(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn ids_are_distinct() {
        assert_ne!(ProjectId::new().to_string(), ProjectId::new().to_string());
    }

    #[test]
    fn video_identifiers_are_uuid_backed_and_round_trip() {
        let session = VideoSessionId::new();
        let request = VideoThumbnailRequestId::new();

        assert_eq!(
            VideoSessionId::from_str(&session.to_string()).unwrap(),
            session
        );
        assert_eq!(
            VideoThumbnailRequestId::from_str(&request.to_string()).unwrap(),
            request
        );
        assert_ne!(session.to_string(), request.to_string());
    }

    #[test]
    fn video_metadata_keeps_optional_properties_and_normalized_failures() {
        use crate::video::{VideoFailureKind, VideoMetadata, VideoProbeStatus};

        let metadata = VideoMetadata {
            duration_us: None,
            display_width: None,
            display_height: None,
            rotation_degrees: -90,
            frame_rate_millihertz: None,
            video_codec: Some("h264".to_owned()),
            audio_codec: None,
            probe_status: VideoProbeStatus::Failed(VideoFailureKind::Damaged),
        };

        assert_eq!(metadata.duration_us, None);
        assert_eq!(metadata.rotation_degrees, -90);
        assert_eq!(
            metadata.probe_status,
            VideoProbeStatus::Failed(VideoFailureKind::Damaged)
        );

        let all_failures = [
            VideoFailureKind::Unsupported,
            VideoFailureKind::Damaged,
            VideoFailureKind::Unreadable,
            VideoFailureKind::Missing,
            VideoFailureKind::EngineInitialization,
            VideoFailureKind::DecodeFallbackFailed,
            VideoFailureKind::RenderSurface,
            VideoFailureKind::ThumbnailUnavailable,
        ];
        assert_eq!(all_failures.len(), 8);
    }
}
