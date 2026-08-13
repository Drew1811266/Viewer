use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

pub const RUNTIME_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub target: String,
    pub mpv: MpvBuild,
    pub ffmpeg: FfmpegBuild,
    pub components: Vec<LockedComponent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MpvBuild {
    pub tag: String,
    pub commit: String,
    pub meson_options: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FfmpegBuild {
    pub tag: String,
    pub configure_options: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LockedComponent {
    pub name: String,
    pub version: String,
    pub source_url: String,
    pub sha256: String,
    pub license: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("video runtime manifest could not be read")]
    Read,
    #[error("video runtime manifest is not valid JSON")]
    InvalidJson,
    #[error("video runtime manifest schema is unsupported")]
    UnsupportedSchema,
    #[error("video runtime component metadata is incomplete")]
    InvalidComponent,
    #[error("video runtime contains a GPL component")]
    GplComponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLayout {
    pub root: PathBuf,
    pub libmpv: PathBuf,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub manifest: PathBuf,
    pub licenses: PathBuf,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeLayoutError {
    #[error("video runtime bundle root must be absolute")]
    BundleRootMustBeAbsolute,
}

impl RuntimeLayout {
    pub fn from_bundle_root(bundle_resources: &Path) -> Result<Self, RuntimeLayoutError> {
        if !bundle_resources.is_absolute() {
            return Err(RuntimeLayoutError::BundleRootMustBeAbsolute);
        }
        let root = bundle_resources.join("ViewerVideoRuntime");
        Ok(Self {
            libmpv: root.join("lib/libmpv.2.dylib"),
            ffmpeg: root.join("bin/ffmpeg"),
            ffprobe: root.join("bin/ffprobe"),
            manifest: root.join("runtime.lock.json"),
            licenses: root.join("licenses"),
            root,
        })
    }
}

impl RuntimeManifest {
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let json = fs::read_to_string(path).map_err(|_| ManifestError::Read)?;
        Self::from_json(&json)
    }

    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(json).map_err(|_| ManifestError::InvalidJson)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn component(&self, name: &str) -> Option<&LockedComponent> {
        self.components
            .iter()
            .find(|component| component.name == name)
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != RUNTIME_MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchema);
        }
        for component in &self.components {
            if component.name.is_empty()
                || component.version.is_empty()
                || component.source_url.is_empty()
                || !is_sha256(&component.sha256)
                || component.license.is_empty()
            {
                return Err(ManifestError::InvalidComponent);
            }
            if component
                .license
                .split(['-', ' '])
                .any(|part| part == "GPL")
            {
                return Err(ManifestError::GplComponent);
            }
        }
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
