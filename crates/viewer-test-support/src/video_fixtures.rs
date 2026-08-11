use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoFixtureManifest {
    pub schema_version: u32,
    pub fixtures: Vec<VideoFixtureEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoFixtureEntry {
    pub id: String,
    pub file: String,
    pub sha256: String,
    pub redistribution: String,
}

#[derive(Clone, Copy, Debug, thiserror::Error, Eq, PartialEq)]
pub enum VideoFixtureError {
    #[error("video fixture manifest is invalid")]
    InvalidManifest,
    #[error("video fixture id is not approved")]
    UnknownFixture,
}

pub fn load_video_manifest() -> Result<VideoFixtureManifest, VideoFixtureError> {
    let manifest: VideoFixtureManifest =
        serde_json::from_str(include_str!("../../../tests/fixtures/videos/manifest.json"))
            .map_err(|_| VideoFixtureError::InvalidManifest)?;
    if manifest.schema_version != 1
        || manifest.fixtures.len() != 3
        || manifest.fixtures.iter().any(|fixture| {
            fixture.id.is_empty()
                || fixture.file.is_empty()
                || fixture.sha256.len() != 64
                || !fixture.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                || fixture.redistribution != "repository-generated"
        })
    {
        return Err(VideoFixtureError::InvalidManifest);
    }
    Ok(manifest)
}

pub fn video_fixture(id: &str) -> Result<PathBuf, VideoFixtureError> {
    let file = match id {
        "h264-1080p" => "h264-1080p.mp4",
        "hevc-portrait" => "hevc-portrait.mp4",
        "vfr-step" => "vfr-step.mp4",
        _ => return Err(VideoFixtureError::UnknownFixture),
    };
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/videos")
        .join(file))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::{load_video_manifest, sha256_hex, video_fixture};

    #[test]
    fn approved_manifest_resolves_the_unchanged_task_three_fixture_bytes() {
        let manifest = load_video_manifest().unwrap();
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(
            manifest
                .fixtures
                .iter()
                .map(|fixture| fixture.id.as_str())
                .collect::<Vec<_>>(),
            ["h264-1080p", "hevc-portrait", "vfr-step"]
        );

        for fixture in manifest.fixtures {
            let path = video_fixture(&fixture.id).unwrap();
            assert_eq!(path.file_name().unwrap().to_str().unwrap(), fixture.file);
            assert_eq!(
                sha256_hex(&std::fs::read(path).unwrap()),
                fixture.sha256,
                "approved fixture bytes changed for {}",
                fixture.id
            );
            assert_eq!(fixture.redistribution, "repository-generated");
        }
    }

    #[test]
    fn unknown_fixture_ids_never_become_paths() {
        assert!(video_fixture("../h264-1080p").is_err());
        assert!(video_fixture("unknown").is_err());
    }
}
