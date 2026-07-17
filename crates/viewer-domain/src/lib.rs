pub mod file;
pub mod image;
pub mod operation;
pub mod search;

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
}
