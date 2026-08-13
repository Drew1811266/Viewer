use serde::Deserialize;
use std::{fs, path::PathBuf};
use tokio_util::sync::CancellationToken;
use viewer_domain::video::{VideoFailureKind, VideoProbeStatus};
use viewer_infrastructure::video_probe::{MediaFileIdentity, VideoProbe, VideoProbeError};
use viewer_video_mpv::{BundledMediaTools, MediaFrameOutput, runtime_manifest::RuntimeLayout};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    id: String,
    file: String,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    rotation: Option<i16>,
    variable_frame_rate: Option<bool>,
    decode_expectation: Option<String>,
    expected_failure: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
enum DecodeOutcome {
    Ready,
    HardwareUnavailable,
}

#[tokio::test]
#[ignore = "requires bundled video runtime"]
async fn fixture_matrix_matches_manifest_expectations() {
    let resources = runtime_resources();
    let layout = RuntimeLayout::from_bundle_root(&resources).expect("absolute runtime resources");
    let probe = VideoProbe::from_bundle_root(&resources).expect("verified bundled ffprobe");
    let tools = BundledMediaTools::from_layout(&layout).expect("verified bundled ffmpeg");
    let manifest: Manifest = serde_json::from_str(include_str!("fixtures/videos/manifest.json"))
        .expect("valid fixture manifest");

    for fixture in manifest.fixtures {
        let path = fixture_path(&fixture.file);
        let outcome = probe.probe(&path, CancellationToken::new()).await;
        match fixture.expected_failure.as_deref() {
            Some("damaged") => assert_eq!(
                outcome,
                Err(VideoProbeError::Failed(VideoFailureKind::Damaged)),
                "{} must normalize as damaged",
                fixture.id
            ),
            Some("unsupported") => assert_eq!(
                outcome,
                Err(VideoProbeError::Failed(VideoFailureKind::Unsupported)),
                "{} must normalize as unsupported",
                fixture.id
            ),
            None => {
                let metadata = outcome.unwrap_or_else(|error| {
                    panic!("{} did not probe successfully: {error:?}", fixture.id)
                });
                assert_eq!(metadata.probe_status, VideoProbeStatus::Ready);
                if let Some(codec) = fixture.video_codec.as_deref() {
                    assert_eq!(
                        metadata.video_codec.as_deref(),
                        Some(codec),
                        "{}",
                        fixture.id
                    );
                }
                assert_eq!(
                    metadata.audio_codec.as_deref(),
                    fixture.audio_codec.as_deref(),
                    "{} audio codec",
                    fixture.id
                );
                if let Some(rotation) = fixture.rotation {
                    assert_eq!(metadata.rotation_degrees, rotation, "{}", fixture.id);
                }
                if fixture.variable_frame_rate == Some(true) {
                    assert_eq!(metadata.frame_rate_millihertz, Some(18_202));
                }

                let file_identity = MediaFileIdentity::from_metadata(
                    &fs::metadata(&path).expect("fixture metadata"),
                );
                let decoded = tools
                    .video_frame_identity_bound(
                        &path,
                        &file_identity,
                        0,
                        MediaFrameOutput::Png320,
                        CancellationToken::new(),
                    )
                    .await
                    .unwrap_or_else(|error| panic!("{} decode failed: {error:?}", fixture.id));
                if fixture.decode_expectation.as_deref() == Some("hardware-dependent") {
                    match normalized_decode_outcome(&decoded) {
                        DecodeOutcome::Ready => println!("PASS fixture-{}-decode", fixture.id),
                        DecodeOutcome::HardwareUnavailable => println!(
                            "UNVERIFIED fixture-{}-decode hardware-dependent; PASS probe",
                            fixture.id
                        ),
                    }
                } else {
                    assert_eq!(normalized_decode_outcome(&decoded), DecodeOutcome::Ready);
                }
            }
            Some(other) => panic!("unknown expected failure {other}"),
        }
        if fixture.decode_expectation.as_deref() != Some("hardware-dependent") {
            println!("PASS fixture-{}", fixture.id);
        }
    }
}

fn normalized_decode_outcome(output: &viewer_video_mpv::MediaToolOutput) -> DecodeOutcome {
    if output.status.success() && output.stdout.starts_with(b"\x89PNG\r\n\x1a\n") {
        return DecodeOutcome::Ready;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("platform doesn't support hardware accelerated AV1 decoding") {
        return DecodeOutcome::HardwareUnavailable;
    }
    panic!("decoder failed without a stable hardware-unavailable classification");
}

fn runtime_resources() -> PathBuf {
    let configured = PathBuf::from(
        std::env::var_os("VIEWER_VIDEO_RUNTIME_DIR")
            .expect("VIEWER_VIDEO_RUNTIME_DIR must name a reviewed runtime"),
    );
    let configured = configured
        .canonicalize()
        .expect("runtime directory must exist");
    if configured.file_name().and_then(|name| name.to_str()) == Some("ViewerVideoRuntime") {
        configured
            .parent()
            .expect("runtime has a resources parent")
            .to_path_buf()
    } else {
        configured
    }
}

fn fixture_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/videos")
        .join(file)
        .canonicalize()
        .expect("fixture file exists")
}
