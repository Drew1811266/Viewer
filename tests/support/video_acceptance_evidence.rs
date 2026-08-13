#![allow(dead_code)]

use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};

pub const EVIDENCE_SCHEMA_VERSION: u64 = 3;
pub const PERFORMANCE_SAMPLE_IDS: [&str; 2] = ["h264-1080p60", "hevc-4k30"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevelopmentEvidence {
    pub schema_version: u64,
    pub mode: String,
    pub binding: EvidenceBinding,
    pub development_decision: StatusEvidence,
    pub machine: MachineEvidence,
    pub native: NativeEvidence,
    pub lifecycle: LifecycleEvidence,
    pub performance: Vec<PerformanceEvidence>,
    pub thumbnail_concurrency: u64,
    pub offline: OfflineEvidence,
    pub source_runs: SourceRuns,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceBinding {
    pub schema_version: u64,
    pub run_id: String,
    pub source_identity: serde_json::Value,
    pub machine: serde_json::Value,
    pub fixtures: serde_json::Value,
    pub app_runtime: serde_json::Value,
    pub bundle_identity: serde_json::Value,
    pub build_config: serde_json::Value,
    pub build_attestation_sha256: String,
    pub bundle_audit_sha256: String,
    pub network_launch_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceStatus {
    Passed,
    Unverified,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatusEvidence {
    pub status: EvidenceStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MachineEvidence {
    pub model: String,
    pub chip: String,
    pub macos: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeEvidence {
    pub h264_first_frame: bool,
    pub hevc_first_frame: bool,
    pub hevc4k_first_frame: bool,
    pub frame_step_forward: bool,
    pub frame_step_backward: bool,
    pub timeline_preview: bool,
    pub close_to_idle: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifecycleEvidence {
    pub cycles: u64,
    pub before: OwnedResources,
    pub after: OwnedResources,
    pub fixture_sequence: Vec<String>,
    pub measured_resources: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedResources {
    pub clients: u64,
    pub render_contexts: u64,
    pub surfaces: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerformanceEvidence {
    pub id: String,
    pub status: EvidenceStatus,
    pub reason: Option<String>,
    pub source_run_id: Option<String>,
    pub codec: String,
    pub width: u64,
    pub height: u64,
    pub frames_per_second: f64,
    pub bit_depth: Option<u64>,
    pub hwdec: Option<String>,
    pub video_output: Option<String>,
    pub average_command_latency_ms: Option<f64>,
    pub post_warmup_rendered_frames: Option<u64>,
    pub post_warmup_dropped_frames: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflineEvidence {
    pub status: EvidenceStatus,
    pub reason: Option<String>,
    pub network_policy: String,
    pub network_disabled: bool,
    pub bundled_runtime_only: bool,
    pub host_dependencies_absent: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRuns {
    pub base: String,
    pub rows: BTreeMap<String, Option<String>>,
    pub h264_performance: String,
    pub hevc4k_functional: String,
    pub hevc4k_performance: Option<String>,
}

impl PerformanceEvidence {
    pub fn dropped_frame_percent(&self) -> Option<f64> {
        let rendered = self.post_warmup_rendered_frames?;
        let dropped = self.post_warmup_dropped_frames?;
        let total = rendered + dropped;
        (total > 0).then(|| dropped as f64 * 100.0 / total as f64)
    }
}

pub fn parse_development_evidence(bytes: &[u8]) -> Result<DevelopmentEvidence, String> {
    let evidence: DevelopmentEvidence =
        serde_json::from_slice(bytes).map_err(|error| format!("invalid JSON: {error}"))?;
    validate_development_evidence(&evidence)?;
    Ok(evidence)
}

fn validate_development_evidence(evidence: &DevelopmentEvidence) -> Result<(), String> {
    if evidence.schema_version != EVIDENCE_SCHEMA_VERSION || evidence.mode != "development" {
        return Err("unsupported Task14 development evidence schema or mode".into());
    }
    if evidence.development_decision.status != EvidenceStatus::Passed {
        return Err("development decision did not pass".into());
    }
    if [
        evidence.native.h264_first_frame,
        evidence.native.hevc_first_frame,
        evidence.native.hevc4k_first_frame,
        evidence.native.frame_step_forward,
        evidence.native.frame_step_backward,
        evidence.native.timeline_preview,
        evidence.native.close_to_idle,
    ]
    .contains(&false)
    {
        return Err("required native evidence did not pass".into());
    }
    if evidence.binding.schema_version != 1
        || evidence.binding.run_id.is_empty()
        || !is_sha256(&evidence.binding.build_attestation_sha256)
        || !is_sha256(&evidence.binding.bundle_audit_sha256)
        || !is_sha256(&evidence.binding.network_launch_sha256)
        || evidence.lifecycle.cycles != 30
        || evidence.lifecycle.before != evidence.lifecycle.after
        || evidence.lifecycle.fixture_sequence.len() != 30
        || evidence
            .lifecycle
            .fixture_sequence
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        || evidence
            .lifecycle
            .fixture_sequence
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            < 2
        || evidence.lifecycle.measured_resources != ["clients", "renderContexts", "surfaces"]
    {
        return Err("required lifecycle evidence did not pass".into());
    }
    for row in [
        "first-frame-ready",
        "frame-step-forward",
        "frame-step-backward",
        "h264-videotoolbox",
        "hevc-videotoolbox",
        "30-mount-unmount-baseline",
        "timeline-preview",
        "h264-1080p60-performance",
    ] {
        if evidence
            .source_runs
            .rows
            .get(row)
            .and_then(|run_id| run_id.as_deref())
            != Some(evidence.binding.run_id.as_str())
        {
            return Err(format!(
                "required row {row} is not bound to the evidence run"
            ));
        }
    }
    if evidence.source_runs.base != evidence.binding.run_id
        || evidence.source_runs.h264_performance != evidence.binding.run_id
        || evidence.source_runs.hevc4k_functional != evidence.binding.run_id
        || evidence.offline.status != EvidenceStatus::Passed
        || evidence.offline.network_policy != "deny-all"
        || !evidence.offline.network_disabled
        || !evidence.offline.bundled_runtime_only
        || !evidence.offline.host_dependencies_absent
    {
        return Err("source-run or strict smoke binding did not pass".into());
    }
    if evidence.performance.len() != PERFORMANCE_SAMPLE_IDS.len() {
        return Err("performance evidence must contain exactly two samples".into());
    }
    for expected_id in PERFORMANCE_SAMPLE_IDS {
        let sample = evidence
            .performance
            .iter()
            .find(|sample| sample.id == expected_id)
            .ok_or_else(|| format!("missing performance sample {expected_id}"))?;
        match (expected_id, sample.status) {
            ("h264-1080p60", EvidenceStatus::Passed) => validate_measured_sample(sample)?,
            ("hevc-4k30", EvidenceStatus::Passed) => validate_measured_sample(sample)?,
            ("hevc-4k30", EvidenceStatus::Unverified) => {
                if sample.reason.as_deref().is_none_or(str::is_empty)
                    || sample.source_run_id.is_some()
                    || sample.bit_depth.is_some()
                    || sample.hwdec.is_some()
                    || sample.video_output.is_some()
                    || sample.average_command_latency_ms.is_some()
                    || sample.post_warmup_rendered_frames.is_some()
                    || sample.post_warmup_dropped_frames.is_some()
                    || evidence.source_runs.hevc4k_performance.is_some()
                {
                    return Err("unverified 4K performance contains asserted measurements".into());
                }
            }
            _ => return Err(format!("unverified is not allowed for {expected_id}")),
        }
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_measured_sample(sample: &PerformanceEvidence) -> Result<(), String> {
    let latency = sample
        .average_command_latency_ms
        .ok_or_else(|| format!("{} has no command latency", sample.id))?;
    let dropped = sample
        .dropped_frame_percent()
        .ok_or_else(|| format!("{} has no dropped-frame window", sample.id))?;
    if sample.source_run_id.as_deref().is_none_or(str::is_empty)
        || !matches!(sample.bit_depth, Some(8 | 10))
        || sample.hwdec.as_deref() != Some("videotoolbox")
        || sample.video_output.as_deref() != Some("libmpv")
        || latency >= 100.0
        || sample.post_warmup_rendered_frames == Some(0)
        || dropped >= 1.0
    {
        return Err(format!("{} measured performance did not pass", sample.id));
    }
    Ok(())
}

pub fn load_development_evidence() -> DevelopmentEvidence {
    let path = evidence_path();
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "Task14 development evidence is missing at {}: {error}; run scripts/video/run-clean-machine-smoke.sh --mode development <Viewer.app>",
            path.display()
        )
    });
    parse_development_evidence(&bytes)
        .unwrap_or_else(|error| panic!("invalid Task14 evidence {}: {error}", path.display()))
}

fn evidence_path() -> PathBuf {
    std::env::var_os("VIEWER_VIDEO_ACCEPTANCE_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../target/video-acceptance/development-evidence.json")
        })
}

#[cfg(test)]
mod tests {
    use super::parse_development_evidence;
    use serde_json::{Value, json};

    fn valid_evidence() -> Value {
        json!({
            "schemaVersion": 3,
            "mode": "development",
            "binding": {
                "schemaVersion": 1, "runId": "run-base",
                "sourceIdentity": {}, "machine": {}, "fixtures": [], "appRuntime": {},
                "bundleIdentity": {
                    "identifier": "com.viewer.desktop", "productName": "Viewer",
                    "executableRelativePath": "Contents/MacOS/viewer-desktop"
                },
                "buildConfig": { "profile": "debug" },
                "buildAttestationSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "bundleAuditSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "networkLaunchSha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
            },
            "developmentDecision": { "status": "passed", "reason": "core passed" },
            "machine": { "model": "Mac16,12", "chip": "Apple M4", "macos": "macOS 26" },
            "native": {
                "h264FirstFrame": true,
                "hevcFirstFrame": true,
                "hevc4kFirstFrame": true,
                "frameStepForward": true,
                "frameStepBackward": true,
                "timelinePreview": true,
                "closeToIdle": true
            },
            "lifecycle": {
                "cycles": 30,
                "before": { "clients": 0, "renderContexts": 0, "surfaces": 0 },
                "after": { "clients": 0, "renderContexts": 0, "surfaces": 0 },
                "fixtureSequence": [
                    "h264-1080p", "hevc-portrait", "h264-1080p", "hevc-portrait",
                    "h264-1080p", "hevc-portrait", "h264-1080p", "hevc-portrait",
                    "h264-1080p", "hevc-portrait", "h264-1080p", "hevc-portrait",
                    "h264-1080p", "hevc-portrait", "h264-1080p", "hevc-portrait",
                    "h264-1080p", "hevc-portrait", "h264-1080p", "hevc-portrait",
                    "h264-1080p", "hevc-portrait", "h264-1080p", "hevc-portrait",
                    "h264-1080p", "hevc-portrait", "h264-1080p", "hevc-portrait",
                    "h264-1080p", "hevc-portrait"
                ],
                "measuredResources": ["clients", "renderContexts", "surfaces"]
            },
            "performance": [
                {
                    "id": "h264-1080p60", "status": "passed", "reason": null,
                    "sourceRunId": "run-base", "codec": "h264", "width": 1920,
                    "height": 1080, "framesPerSecond": 60, "bitDepth": 8,
                    "hwdec": "videotoolbox", "videoOutput": "libmpv",
                    "averageCommandLatencyMs": 0.01965, "postWarmupRenderedFrames": 76,
                    "postWarmupDroppedFrames": 0
                },
                {
                    "id": "hevc-4k30", "status": "unverified",
                    "reason": "dropped-frame window was not collected", "sourceRunId": null,
                    "codec": "hevc", "width": 3840, "height": 2160,
                    "framesPerSecond": 30, "bitDepth": null, "hwdec": null,
                    "videoOutput": null, "averageCommandLatencyMs": null,
                    "postWarmupRenderedFrames": null, "postWarmupDroppedFrames": null
                }
            ],
            "thumbnailConcurrency": 1,
            "offline": {
                "status": "passed", "reason": null, "networkPolicy": "deny-all",
                "networkDisabled": true,
                "bundledRuntimeOnly": true, "hostDependenciesAbsent": true
            },
            "sourceRuns": {
                "base": "run-base",
                "rows": {
                    "first-frame-ready": "run-base",
                    "frame-step-forward": "run-base",
                    "frame-step-backward": "run-base",
                    "h264-videotoolbox": "run-base",
                    "hevc-videotoolbox": "run-base",
                    "30-mount-unmount-baseline": "run-base",
                    "timeline-preview": "run-base",
                    "h264-1080p60-performance": "run-base",
                    "hevc-4k30-performance": null
                },
                "h264Performance": "run-base", "hevc4kFunctional": "run-base",
                "hevc4kPerformance": null
            }
        })
    }

    #[test]
    fn only_the_allowlisted_4k_performance_sample_may_be_unverified() {
        let valid = serde_json::to_vec(&valid_evidence()).unwrap();
        assert!(parse_development_evidence(&valid).is_ok());

        let mut invalid = valid_evidence();
        invalid["performance"][0]["status"] = json!("unverified");
        invalid["performance"][0]["reason"] = json!("not collected");
        invalid["performance"][0]["sourceRunId"] = Value::Null;
        assert!(parse_development_evidence(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }

    #[test]
    fn false_or_missing_required_core_evidence_rejects_the_document() {
        let mut failed = valid_evidence();
        failed["native"]["frameStepBackward"] = json!(false);
        assert!(parse_development_evidence(&serde_json::to_vec(&failed).unwrap()).is_err());

        let mut missing = valid_evidence();
        missing["native"]
            .as_object_mut()
            .unwrap()
            .remove("timelinePreview");
        assert!(parse_development_evidence(&serde_json::to_vec(&missing).unwrap()).is_err());

        let mut missing_source = valid_evidence();
        missing_source["sourceRuns"]["rows"]
            .as_object_mut()
            .unwrap()
            .remove("frame-step-backward");
        assert!(parse_development_evidence(&serde_json::to_vec(&missing_source).unwrap()).is_err());
    }

    #[test]
    fn lifecycle_requires_the_measured_resource_allowlist_and_navigation_sequence() {
        let mut single_fixture = valid_evidence();
        single_fixture["lifecycle"]["fixtureSequence"] = json!(vec!["h264-1080p"; 30]);
        assert!(parse_development_evidence(&serde_json::to_vec(&single_fixture).unwrap()).is_err());

        let mut exaggerated_claim = valid_evidence();
        exaggerated_claim["lifecycle"]["measuredResources"] =
            json!(["clients", "renderContexts", "surfaces", "audio"]);
        assert!(
            parse_development_evidence(&serde_json::to_vec(&exaggerated_claim).unwrap()).is_err()
        );
    }

    #[test]
    fn build_and_audit_attestation_digests_are_required() {
        let mut missing = valid_evidence();
        missing["binding"]
            .as_object_mut()
            .unwrap()
            .remove("bundleAuditSha256");
        assert!(parse_development_evidence(&serde_json::to_vec(&missing).unwrap()).is_err());
    }

    #[test]
    fn deny_all_launch_digest_is_required_for_offline_evidence() {
        assert!(
            parse_development_evidence(&serde_json::to_vec(&valid_evidence()).unwrap()).is_ok()
        );

        let mut missing = valid_evidence();
        missing["binding"]
            .as_object_mut()
            .unwrap()
            .remove("networkLaunchSha256");
        assert!(parse_development_evidence(&serde_json::to_vec(&missing).unwrap()).is_err());

        let mut unrestricted = valid_evidence();
        unrestricted["offline"]["networkPolicy"] = json!("unrestricted");
        assert!(parse_development_evidence(&serde_json::to_vec(&unrestricted).unwrap()).is_err());
    }
}
