use std::{error::Error, fs, path::PathBuf, time::Instant};

use serde::Serialize;
use viewer_application::review_workspace::{ReviewWorkspaceCommand, TargetEdit};
use viewer_domain::{
    ReviewCommandId,
    review::{FeedbackAnchor, NormalizedRect},
};

#[path = "support/continuous_review_fixture.rs"]
mod continuous_review_fixture;
use continuous_review_fixture::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LatencySummary {
    scenario: &'static str,
    samples: usize,
    p50_us: u64,
    p95_us: u64,
    p99_us: u64,
}

#[derive(Clone, Copy)]
enum SaveScenario {
    Asset,
    Region,
}

impl SaveScenario {
    const fn name(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Region => "region",
        }
    }

    fn anchor(self) -> Result<FeedbackAnchor, Box<dyn Error>> {
        Ok(match self {
            Self::Asset => FeedbackAnchor::Asset,
            Self::Region => FeedbackAnchor::ImageRect(NormalizedRect::new(0.25, 0.2, 0.4, 0.35)?),
        })
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("review save latency harness failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let samples = parse_samples(std::env::args().skip(1))?;
    for scenario in [SaveScenario::Asset, SaveScenario::Region] {
        let summary = measure(scenario, samples).await?;
        println!("{}", serde_json::to_string(&summary)?);
    }
    Ok(())
}

async fn measure(scenario: SaveScenario, samples: usize) -> Result<LatencySummary, Box<dyn Error>> {
    let project = tempfile::tempdir()?;
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/images/rotated-6.jpg");
    fs::copy(source, project.path().join("source-1.jpg"))?;

    // Project indexing, image-port setup, and initial asset preparation are deliberately outside
    // the timed section. Each sample measures the existing save acknowledgement path only.
    let composition = build_composition(project.path())?;
    let asset = composition
        .prepare_named_assets(&["source-1.jpg"])
        .await?
        .remove(0);
    let mut expected_snapshot_id = None;
    let mut elapsed_us = Vec::with_capacity(samples);

    for sample in 0..samples {
        let started_at = Instant::now();
        let envelope = composition
            .service
            .prepare(
                ReviewCommandId::new(),
                expected_snapshot_id,
                ReviewWorkspaceCommand::SaveFeedback {
                    feedback_id: None,
                    text: format!("baseline {} save {sample}", scenario.name()),
                    targets: vec![TargetEdit::Add {
                        asset_version_id: asset.id,
                        anchor: scenario.anchor()?,
                    }],
                },
            )
            .await?;
        let applied = composition.service.apply(envelope).await?;
        elapsed_us.push(started_at.elapsed().as_micros().min(u128::from(u64::MAX)) as u64);
        expected_snapshot_id = Some(applied.receipt.snapshot.snapshot_id);
    }

    elapsed_us.sort_unstable();
    let summary = LatencySummary {
        scenario: scenario.name(),
        samples,
        p50_us: percentile(&elapsed_us, 50),
        p95_us: percentile(&elapsed_us, 95),
        p99_us: percentile(&elapsed_us, 99),
    };
    if summary.p50_us == 0 || summary.p95_us == 0 || summary.p99_us == 0 {
        return Err("latency clock produced a zero percentile".into());
    }
    Ok(summary)
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    let rank = percentile.saturating_mul(sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn parse_samples(arguments: impl Iterator<Item = String>) -> Result<usize, Box<dyn Error>> {
    let mut arguments = arguments;
    let mut samples = 30;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--samples" => {
                samples = arguments
                    .next()
                    .ok_or("--samples requires a value")?
                    .parse()?;
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    if !(1..=1_000).contains(&samples) {
        return Err("samples must be between 1 and 1000".into());
    }
    Ok(samples)
}
