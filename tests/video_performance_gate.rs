#[path = "support/video_acceptance_evidence.rs"]
mod evidence;

use evidence::{EvidenceStatus, PERFORMANCE_SAMPLE_IDS, load_development_evidence};

#[test]
#[ignore = "requires macOS native video acceptance"]
fn common_local_samples_meet_native_playback_budgets() {
    let evidence = load_development_evidence();
    assert!(!evidence.machine.model.is_empty());
    assert!(!evidence.machine.chip.is_empty());
    assert!(!evidence.machine.macos.is_empty());
    assert_eq!(evidence.thumbnail_concurrency, 1);

    assert_eq!(evidence.performance.len(), PERFORMANCE_SAMPLE_IDS.len());
    for expected_id in PERFORMANCE_SAMPLE_IDS {
        let sample = evidence
            .performance
            .iter()
            .find(|sample| sample.id == expected_id)
            .unwrap_or_else(|| panic!("missing performance sample {expected_id}"));
        assert!(sample.codec == "h264" || sample.codec == "hevc");
        match expected_id {
            "h264-1080p60" => {
                assert_eq!((sample.width, sample.height), (1920, 1080));
                assert!((sample.frames_per_second - 60.0).abs() < 0.01);
            }
            "hevc-4k30" => {
                assert_eq!((sample.width, sample.height), (3840, 2160));
                assert!((sample.frames_per_second - 30.0).abs() < 0.01);
            }
            _ => unreachable!(),
        }
        match sample.status {
            EvidenceStatus::Passed => println!(
                "PASS {} codec={} {}x{}@{} bit={} hwdec={} vo={} latency_ms={:.3} dropped_pct={:.3} source_run={}",
                sample.id,
                sample.codec,
                sample.width,
                sample.height,
                sample.frames_per_second,
                sample.bit_depth.expect("validated bit depth"),
                sample.hwdec.as_deref().expect("validated hardware decoder"),
                sample
                    .video_output
                    .as_deref()
                    .expect("validated video output"),
                sample
                    .average_command_latency_ms
                    .expect("validated command latency"),
                sample
                    .dropped_frame_percent()
                    .expect("validated dropped-frame window"),
                sample
                    .source_run_id
                    .as_deref()
                    .expect("validated source run"),
            ),
            EvidenceStatus::Unverified => println!(
                "UNVERIFIED {} reason={}",
                sample.id,
                sample
                    .reason
                    .as_deref()
                    .expect("validated unverified reason")
            ),
        }
    }
}
