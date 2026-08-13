use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoFixtureManifest {
    pub schema_version: u32,
    pub license: String,
    pub provenance: String,
    pub fixtures: Vec<VideoFixtureEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoFixtureEntry {
    pub id: String,
    pub file: String,
    pub container: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub rotation: Option<i16>,
    pub variable_frame_rate: Option<bool>,
    pub decode_expectation: Option<String>,
    pub expected_failure: Option<String>,
    pub sha256: String,
    pub redistribution: String,
    pub redistributable: bool,
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
    if manifest.schema_version != 2
        || manifest.license != "CC0-1.0"
        || manifest.provenance.is_empty()
        || manifest.fixtures.len() != 9
        || manifest.fixtures.iter().any(|fixture| {
            fixture.id.is_empty()
                || fixture.file.is_empty()
                || fixture.container.is_empty()
                || fixture.sha256.len() != 64
                || !fixture.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !matches!(
                    fixture.redistribution.as_str(),
                    "reviewed-runtime-generated" | "repository-embedded-reviewed-seed"
                )
                || !fixture.redistributable
        })
    {
        return Err(VideoFixtureError::InvalidManifest);
    }
    Ok(manifest)
}

pub fn video_fixture(id: &str) -> Result<PathBuf, VideoFixtureError> {
    let file = match id {
        "h264-1080p" => "h264-1080p.mp4",
        "vfr-step" => "vfr-step.mp4",
        "h264-aac" => "h264-aac.mp4",
        "hevc-portrait" => "hevc-portrait.mov",
        "prores" => "prores.mov",
        "vp9-opus" => "vp9-opus.webm",
        "av1" => "av1.mkv",
        "vfr" => "vfr.mp4",
        "silent" => "silent.mp4",
        "truncated" => "truncated.mp4",
        "unsupported-codec" => "unsupported-codec.mkv",
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
    fn task_fourteen_manifest_resolves_the_nine_redistribution_safe_fixtures() {
        let manifest = load_video_manifest().unwrap();
        assert_eq!(manifest.schema_version, 2);
        assert_eq!(
            manifest
                .fixtures
                .iter()
                .map(|fixture| fixture.id.as_str())
                .collect::<Vec<_>>(),
            [
                "h264-aac",
                "hevc-portrait",
                "prores",
                "vp9-opus",
                "av1",
                "vfr",
                "silent",
                "truncated",
                "unsupported-codec",
            ]
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
            assert!(fixture.redistributable);
        }
    }

    #[test]
    fn task_three_native_fixture_bytes_remain_pinned_for_feasibility() {
        for (id, expected) in [
            (
                "h264-1080p",
                "972aff59c7183940dbdfae2d906421a26604a10b6bd24432ccf969a028674296",
            ),
            (
                "vfr-step",
                "ee9ede284fabb52570fdd51428bf5408fad35de9ef8a4aeaefbcbc577cfcdde9",
            ),
        ] {
            let path = video_fixture(id).unwrap();
            assert_eq!(sha256_hex(&std::fs::read(path).unwrap()), expected);
        }
    }

    #[test]
    fn unknown_fixture_ids_never_become_paths() {
        assert!(video_fixture("../h264-1080p").is_err());
        assert!(video_fixture("unknown").is_err());
    }
}
