use serde::{Deserialize, Serialize};
use std::{fmt, path::{Component, Path}};
use thiserror::Error;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
        }

        impl Default for $name {
            fn default() -> Self { Self::new() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
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

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct RelativePath(String);

#[derive(Debug, Error, Eq, PartialEq)]
pub enum RelativePathError {
    #[error("path must be a non-empty project-relative path")]
    Invalid,
}

impl RelativePath {
    pub fn parse(value: &str) -> Result<Self, RelativePathError> {
        let path = Path::new(value);
        let valid = !value.is_empty()
            && !path.is_absolute()
            && path.components().all(|part| {
                matches!(part, Component::Normal(name) if name != ".viewer")
            });
        valid.then(|| Self(value.to_owned())).ok_or(RelativePathError::Invalid)
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_rejects_escape_and_absolute_paths() {
        assert!(RelativePath::parse("../outside").is_err());
        assert!(RelativePath::parse("/absolute").is_err());
        assert!(RelativePath::parse(".viewer/metadata.sqlite").is_err());
        assert!(RelativePath::parse("products/id-1/front.png").is_ok());
    }

    #[test]
    fn ids_are_distinct() {
        assert_ne!(ProjectId::new().to_string(), ProjectId::new().to_string());
    }
}
