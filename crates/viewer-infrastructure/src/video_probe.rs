use serde_json::Value;
use std::path::Path;
use tokio_util::sync::CancellationToken;
use viewer_domain::video::{VideoFailureKind, VideoMetadata, VideoProbeStatus};
use viewer_video_mpv::{BundledMediaTools, MediaToolError, runtime_manifest::RuntimeLayout};

pub use viewer_video_mpv::MediaFileIdentity;

const MAX_CODEC_BYTES: usize = 64;
const MAX_DIMENSION: u64 = 1_000_000;
const MAX_FRAME_RATE_MILLIHERTZ: u128 = 1_000_000_000;
const MAX_DURATION_US: u128 = 100 * 366 * 24 * 60 * 60 * 1_000_000;

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum VideoProbeError {
    #[error("video metadata probe failed")]
    Failed(VideoFailureKind),
    #[error("video metadata probe was cancelled")]
    Cancelled,
    #[error("video input changed after active-entity validation")]
    SourceChanged,
}

#[async_trait::async_trait]
pub trait VideoMetadataProbe: Send + Sync {
    async fn probe(
        &self,
        canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError>;

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError>;
}

#[derive(Clone, Debug)]
pub struct VideoProbe {
    tools: BundledMediaTools,
}

impl VideoProbe {
    pub fn new(tools: BundledMediaTools) -> Self {
        Self { tools }
    }

    pub fn from_bundle_root(bundle_resources: &Path) -> Result<Self, VideoProbeError> {
        let layout =
            RuntimeLayout::from_bundle_root(bundle_resources).map_err(|_| engine_init())?;
        let tools = BundledMediaTools::from_layout(&layout).map_err(|_| engine_init())?;
        Ok(Self::new(tools))
    }

    pub async fn probe(
        &self,
        canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        let output = self.tools.ffprobe_json(canonical_path, cancellation).await;
        normalize_output(output)
    }

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        let output = self
            .tools
            .ffprobe_json_identity_bound(canonical_path, expected_identity, cancellation)
            .await;
        normalize_output(output)
    }
}

fn normalize_output(
    output: Result<viewer_video_mpv::MediaToolOutput, MediaToolError>,
) -> Result<VideoMetadata, VideoProbeError> {
    let output = output.map_err(map_media_tool_error)?;
    if !output.status.success() {
        return Err(damaged());
    }
    let json = std::str::from_utf8(&output.stdout).map_err(|_| damaged())?;
    normalize_ffprobe(json)
}

#[async_trait::async_trait]
impl VideoMetadataProbe for VideoProbe {
    async fn probe(
        &self,
        canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        VideoProbe::probe(self, canonical_path, cancellation).await
    }

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        VideoProbe::probe_identity_bound(self, canonical_path, expected_identity, cancellation)
            .await
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableVideoProbe;

#[async_trait::async_trait]
impl VideoMetadataProbe for UnavailableVideoProbe {
    async fn probe(
        &self,
        _canonical_path: &Path,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        if cancellation.is_cancelled() {
            Err(VideoProbeError::Cancelled)
        } else {
            Err(engine_init())
        }
    }

    async fn probe_identity_bound(
        &self,
        canonical_path: &Path,
        _expected_identity: &MediaFileIdentity,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, VideoProbeError> {
        self.probe(canonical_path, cancellation).await
    }
}

fn map_media_tool_error(error: MediaToolError) -> VideoProbeError {
    match error {
        MediaToolError::Cancelled => VideoProbeError::Cancelled,
        MediaToolError::InputMissing => VideoProbeError::Failed(VideoFailureKind::Missing),
        MediaToolError::InputUnreadable | MediaToolError::InputPathNotCanonical => {
            VideoProbeError::Failed(VideoFailureKind::Unreadable)
        }
        MediaToolError::InputChanged => VideoProbeError::SourceChanged,
        MediaToolError::TimedOut
        | MediaToolError::OutputTooLarge
        | MediaToolError::StdoutTooLarge
        | MediaToolError::StderrTooLarge => damaged(),
        MediaToolError::ExecutablePathMustBeAbsolute
        | MediaToolError::UnexpectedExecutablePath
        | MediaToolError::UnsafeExecutable
        | MediaToolError::ExecutableChanged
        | MediaToolError::RuntimeIntegrity
        | MediaToolError::GateClosed
        | MediaToolError::Io(_) => engine_init(),
    }
}

fn engine_init() -> VideoProbeError {
    VideoProbeError::Failed(VideoFailureKind::EngineInitialization)
}

fn normalize_ffprobe(json: &str) -> Result<VideoMetadata, VideoProbeError> {
    let document: Value = serde_json::from_str(json).map_err(|_| damaged())?;
    let streams = document
        .get("streams")
        .and_then(Value::as_array)
        .ok_or_else(damaged)?;
    let video = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
        .ok_or(VideoProbeError::Failed(VideoFailureKind::Unsupported))?;
    let audio = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));

    let width = parse_dimension(video.get("width"))?;
    let height = parse_dimension(video.get("height"))?;
    if width.is_some() != height.is_some() {
        return Err(damaged());
    }
    let rotation_degrees = parse_rotation(video)?;
    let (display_width, display_height) = if matches!(rotation_degrees, 90 | -90) {
        (height, width)
    } else {
        (width, height)
    };
    let average_rate = parse_frame_rate(video.get("avg_frame_rate"))?;
    let frame_rate_millihertz = match average_rate {
        Some(rate) => Some(rate),
        None => parse_frame_rate(video.get("r_frame_rate"))?,
    };
    let duration_us = document
        .get("format")
        .and_then(|format| format.get("duration"))
        .map(parse_duration)
        .transpose()?
        .flatten();
    let video_codec = parse_codec(video.get("codec_name"))?
        .ok_or(VideoProbeError::Failed(VideoFailureKind::Unsupported))?;

    Ok(VideoMetadata {
        duration_us,
        display_width,
        display_height,
        rotation_degrees,
        frame_rate_millihertz,
        video_codec: Some(video_codec),
        audio_codec: audio
            .map(|stream| parse_codec(stream.get("codec_name")))
            .transpose()?
            .flatten(),
        probe_status: VideoProbeStatus::Ready,
    })
}

fn damaged() -> VideoProbeError {
    VideoProbeError::Failed(VideoFailureKind::Damaged)
}

fn parse_codec(value: Option<&Value>) -> Result<Option<String>, VideoProbeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let codec = value.as_str().ok_or_else(damaged)?;
    if codec.is_empty() {
        return Ok(None);
    }
    if codec.len() > MAX_CODEC_BYTES || !codec.is_ascii() {
        return Err(damaged());
    }
    Ok(Some(codec.to_owned()))
}

fn parse_dimension(value: Option<&Value>) -> Result<Option<u32>, VideoProbeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let dimension = value.as_u64().ok_or_else(damaged)?;
    if dimension == 0 || dimension > MAX_DIMENSION {
        return Err(damaged());
    }
    u32::try_from(dimension).map(Some).map_err(|_| damaged())
}

fn parse_rotation(stream: &Value) -> Result<i16, VideoProbeError> {
    let side_data_rotation = stream
        .get("side_data_list")
        .map(|items| items.as_array().ok_or_else(damaged))
        .transpose()?
        .and_then(|items| items.iter().find_map(|item| item.get("rotation")));
    let tag_rotation = stream
        .get("tags")
        .map(|tags| tags.as_object().ok_or_else(damaged))
        .transpose()?
        .and_then(|tags| tags.get("rotate"));
    let Some(value) = side_data_rotation.or(tag_rotation) else {
        return Ok(0);
    };
    let raw = match value {
        Value::Number(number) => number.as_i64().ok_or_else(damaged)?,
        Value::String(value) => value.parse::<i64>().map_err(|_| damaged())?,
        _ => return Err(damaged()),
    };
    if raw % 90 != 0 {
        return Err(damaged());
    }
    match raw.rem_euclid(360) {
        0 => Ok(0),
        90 => Ok(90),
        180 => Ok(180),
        270 => Ok(-90),
        _ => unreachable!("multiples of 90 have four normalized values"),
    }
}

fn parse_frame_rate(value: Option<&Value>) -> Result<Option<u32>, VideoProbeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let rate = value.as_str().ok_or_else(damaged)?;
    if matches!(rate, "" | "N/A" | "0/0") {
        return Ok(None);
    }
    let (numerator, denominator) = rate.split_once('/').ok_or_else(damaged)?;
    if denominator.contains('/') {
        return Err(damaged());
    }
    let numerator = numerator.parse::<u128>().map_err(|_| damaged())?;
    let denominator = denominator.parse::<u128>().map_err(|_| damaged())?;
    if denominator == 0 {
        return Err(damaged());
    }
    if numerator == 0 {
        return Ok(None);
    }
    let scaled = numerator.checked_mul(1_000).ok_or_else(damaged)?;
    let rounded = scaled.checked_add(denominator / 2).ok_or_else(damaged)? / denominator;
    if rounded == 0 || rounded > MAX_FRAME_RATE_MILLIHERTZ {
        return Err(damaged());
    }
    u32::try_from(rounded).map(Some).map_err(|_| damaged())
}

fn parse_duration(value: &Value) -> Result<Option<u64>, VideoProbeError> {
    let duration = value.as_str().ok_or_else(damaged)?;
    if matches!(duration, "" | "N/A") {
        return Ok(None);
    }
    let (whole, fraction) = duration.split_once('.').unwrap_or((duration, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(damaged());
    }
    let whole = whole.parse::<u128>().map_err(|_| damaged())?;
    let mut fraction_us = 0_u128;
    for index in 0..6 {
        fraction_us *= 10;
        if let Some(digit) = fraction.as_bytes().get(index) {
            fraction_us += u128::from(digit - b'0');
        }
    }
    let duration_us = whole
        .checked_mul(1_000_000)
        .and_then(|whole_us| whole_us.checked_add(fraction_us))
        .ok_or_else(damaged)?;
    if duration_us > MAX_DURATION_US {
        return Err(damaged());
    }
    u64::try_from(duration_us).map(Some).map_err(|_| damaged())
}

#[cfg(test)]
mod tests {
    use super::{VideoProbe, VideoProbeError, normalize_ffprobe};
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };
    use tokio_util::sync::CancellationToken;
    use viewer_domain::video::{VideoFailureKind, VideoProbeStatus};
    use viewer_test_support::video_fixtures::sha256_hex;
    use viewer_video_mpv::{BundledMediaTools, runtime_manifest::RuntimeLayout};

    #[test]
    fn normalizes_rotated_vfr_video_without_audio() {
        let metadata = normalize_ffprobe(include_str!(
            "../../../tests/fixtures/videos/rotated-vfr-no-audio.json"
        ))
        .unwrap();

        assert_eq!(metadata.duration_us, Some(2_966_667));
        assert_eq!(metadata.display_width, Some(540));
        assert_eq!(metadata.display_height, Some(960));
        assert_eq!(metadata.rotation_degrees, 90);
        assert_eq!(metadata.frame_rate_millihertz, Some(29_970));
        assert_eq!(metadata.video_codec.as_deref(), Some("h264"));
        assert_eq!(metadata.audio_codec, None);
        assert_eq!(metadata.probe_status, VideoProbeStatus::Ready);
    }

    #[test]
    fn missing_duration_and_frame_rate_remain_valid() {
        let metadata = normalize_ffprobe(include_str!(
            "../../../tests/fixtures/videos/no-duration.json"
        ))
        .unwrap();

        assert_eq!(metadata.duration_us, None);
        assert_eq!(metadata.frame_rate_millihertz, None);
        assert_eq!(metadata.probe_status, VideoProbeStatus::Ready);
    }

    #[test]
    fn side_data_rotation_precedes_the_legacy_tag_and_average_rate_precedes_nominal_rate() {
        let metadata = normalize_ffprobe(
            r#"{
              "streams": [{
                "codec_type": "video", "codec_name": "hevc",
                "width": 1080, "height": 1920,
                "avg_frame_rate": "24000/1001", "r_frame_rate": "60/1",
                "tags": { "rotate": "-90" },
                "side_data_list": [{ "rotation": 90 }]
              }],
              "format": { "duration": "2.000000" }
            }"#,
        )
        .unwrap();

        assert_eq!(metadata.rotation_degrees, 90);
        assert_eq!(metadata.frame_rate_millihertz, Some(23_976));
        assert_eq!(metadata.display_width, Some(1920));
        assert_eq!(metadata.display_height, Some(1080));
    }

    #[test]
    fn unknown_average_rate_falls_back_to_the_nominal_rate() {
        let metadata = normalize_ffprobe(
            r#"{
              "streams": [{
                "codec_type": "video", "codec_name": "vp9",
                "width": 640, "height": 360,
                "avg_frame_rate": "0/0", "r_frame_rate": "25/1"
              }],
              "format": { "duration": "N/A" }
            }"#,
        )
        .unwrap();

        assert_eq!(metadata.frame_rate_millihertz, Some(25_000));
        assert_eq!(metadata.duration_us, None);
    }

    #[test]
    fn invalid_json_and_unsupported_streams_have_stable_failure_kinds() {
        assert_eq!(
            normalize_ffprobe("not json").unwrap_err(),
            VideoProbeError::Failed(VideoFailureKind::Damaged)
        );
        assert_eq!(
            normalize_ffprobe(r#"{"streams":[{"codec_type":"audio","codec_name":"aac"}]}"#)
                .unwrap_err(),
            VideoProbeError::Failed(VideoFailureKind::Unsupported)
        );
    }

    #[test]
    fn corrupt_numeric_and_unbounded_metadata_are_rejected_without_wrapping() {
        for corrupt in [
            r#"{"streams":[{"codec_type":"video","width":4294967296,"height":1}]}"#,
            r#"{"streams":[{"codec_type":"video","width":1,"height":1,"avg_frame_rate":"1/0"}]}"#,
            r#"{"streams":[{"codec_type":"video","width":1,"height":1,"avg_frame_rate":"NaN"}]}"#,
            r#"{"streams":[{"codec_type":"video","width":1,"height":1,"codec_name":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}"#,
            r#"{"streams":[{"codec_type":"video","width":1,"height":1}],"format":{"duration":"18446744073709551615.0"}}"#,
        ] {
            assert_eq!(
                normalize_ffprobe(corrupt).unwrap_err(),
                VideoProbeError::Failed(VideoFailureKind::Damaged),
                "accepted corrupt ffprobe JSON: {corrupt}"
            );
        }
    }

    #[tokio::test]
    async fn probe_normalizes_success_and_maps_process_failures_without_details() {
        let ready = fake_probe(
            r#"printf '%s' '{"streams":[{"codec_type":"video","codec_name":"h264","width":16,"height":9,"avg_frame_rate":"30/1"}],"format":{"duration":"2.0"}}'"#,
        );
        let media = canonical_media(ready.path(), "ready.mp4");
        let metadata = ready
            .probe()
            .probe(&media, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(metadata.duration_us, Some(2_000_000));
        assert_eq!(metadata.probe_status, VideoProbeStatus::Ready);

        let invalid = fake_probe("printf 'not-json'");
        let media = canonical_media(invalid.path(), "invalid.mp4");
        assert_eq!(
            invalid
                .probe()
                .probe(&media, CancellationToken::new())
                .await
                .unwrap_err(),
            VideoProbeError::Failed(VideoFailureKind::Damaged)
        );

        let nonzero = fake_probe("printf 'private diagnostic' >&2; exit 2");
        let media = canonical_media(nonzero.path(), "nonzero.mp4");
        let error = nonzero
            .probe()
            .probe(&media, CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error, VideoProbeError::Failed(VideoFailureKind::Damaged));
        assert!(!format!("{error:?}").contains("private diagnostic"));
        assert!(!error.to_string().contains(media.to_str().unwrap()));
    }

    #[tokio::test]
    async fn probe_maps_missing_unreadable_engine_and_cancellation_stably() {
        let missing = fake_probe("printf '{}'");
        let media = canonical_media(missing.path(), "missing.mp4");
        fs::remove_file(&media).unwrap();
        assert_eq!(
            missing
                .probe()
                .probe(&media, CancellationToken::new())
                .await
                .unwrap_err(),
            VideoProbeError::Failed(VideoFailureKind::Missing)
        );

        let unreadable = fake_probe("printf '{}'");
        let media = canonical_media(unreadable.path(), "unreadable.mp4");
        fs::set_permissions(&media, fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(
            unreadable
                .probe()
                .probe(&media, CancellationToken::new())
                .await
                .unwrap_err(),
            VideoProbeError::Failed(VideoFailureKind::Unreadable)
        );

        let engine = fake_probe("printf '{}'");
        let engine_probe = engine.probe();
        fs::remove_file(engine.ffprobe_path()).unwrap();
        let media = canonical_media(engine.path(), "engine.mp4");
        assert_eq!(
            engine_probe
                .probe(&media, CancellationToken::new())
                .await
                .unwrap_err(),
            VideoProbeError::Failed(VideoFailureKind::EngineInitialization)
        );

        let cancelled = fake_probe("exit 0");
        let media = canonical_media(cancelled.path(), "cancelled.mp4");
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            cancelled
                .probe()
                .probe(&media, cancellation)
                .await
                .unwrap_err(),
            VideoProbeError::Cancelled
        );
    }

    struct FakeProbe {
        directory: tempfile::TempDir,
        layout: RuntimeLayout,
    }

    impl FakeProbe {
        fn path(&self) -> &Path {
            self.directory.path()
        }

        fn ffprobe_path(&self) -> &Path {
            &self.layout.ffprobe
        }

        fn probe(&self) -> VideoProbe {
            VideoProbe::new(BundledMediaTools::from_layout(&self.layout).unwrap())
        }
    }

    fn fake_probe(body: &str) -> FakeProbe {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("ViewerVideoRuntime");
        fs::create_dir_all(root.join("bin")).unwrap();
        let ffprobe = root.join("bin/ffprobe");
        let ffmpeg = root.join("bin/ffmpeg");
        fs::write(&ffprobe, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::write(&ffmpeg, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&ffprobe, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            root.join("runtime.lock.json"),
            r#"{"schemaVersion":1,"target":"universal-apple-darwin","mpv":{"tag":"v0.41.0","commit":"41f6a64","mesonOptions":{}},"ffmpeg":{"tag":"n8.0","configureOptions":["--disable-gpl","--disable-nonfree","--disable-network","--disable-ffplay","--enable-zlib","--enable-encoder=png"]},"components":[]}"#,
        )
        .unwrap();
        fs::write(
            root.join("runtime.inventory.sha256"),
            format!(
                "{}  bin/ffmpeg\n{}  bin/ffprobe\n",
                sha256_hex(&fs::read(&ffmpeg).unwrap()),
                sha256_hex(&fs::read(&ffprobe).unwrap())
            ),
        )
        .unwrap();
        let layout = RuntimeLayout {
            libmpv: root.join("lib/libmpv.2.dylib"),
            ffmpeg,
            ffprobe,
            manifest: root.join("runtime.lock.json"),
            licenses: root.join("licenses"),
            root,
        };
        FakeProbe { directory, layout }
    }

    fn canonical_media(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, b"fixture").unwrap();
        path.canonicalize().unwrap()
    }
}
