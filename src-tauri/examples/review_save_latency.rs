use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::Serialize;
use viewer_application::{
    ReviewTaskCancellation,
    review_workspace::{
        ContinuousReviewPublication, ReviewMaterializationOutcome, ReviewMaterializationService,
        ReviewWorkspaceCommand, TargetEdit,
    },
};
use viewer_desktop::dto::review_workspace::ReviewAuthoringApplyResultDto;
use viewer_domain::{
    ReviewCommandId,
    review::{FeedbackAnchor, NormalizedRect},
};

#[path = "support/continuous_review_fixture.rs"]
mod continuous_review_fixture;
use continuous_review_fixture::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LatencySummary<'a> {
    phase: &'static str,
    scenario: &'static str,
    source_project: &'a Path,
    source_asset: &'a Path,
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

struct Options {
    project: PathBuf,
    samples: usize,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("review save latency harness failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let options = parse_options(std::env::args().skip(1))?;
    let source = first_image(&options.project)?;
    for scenario in [SaveScenario::Asset, SaveScenario::Region] {
        let (authoring, materialization) =
            measure(&options.project, &source, scenario, options.samples).await?;
        println!(
            "{}",
            serde_json::to_string(&summary(
                "authoring",
                scenario,
                &options.project,
                &source,
                &authoring,
            ))?
        );
        println!(
            "{}",
            serde_json::to_string(&summary(
                "materialization",
                scenario,
                &options.project,
                &source,
                &materialization,
            ))?
        );
    }
    Ok(())
}

async fn measure(
    source_project: &Path,
    source: &Path,
    scenario: SaveScenario,
    samples: usize,
) -> Result<(Vec<u64>, Vec<u64>), Box<dyn Error>> {
    let project = tempfile::tempdir()?;
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    let fixture_name = format!("source.{extension}");
    fs::copy(source, project.path().join(&fixture_name))?;

    // Corpus selection, safe temporary staging, indexing, source hashing, and first preparation
    // are deliberately outside both measured phases. The user's corpus is never modified.
    let composition = build_portable_composition(project.path())?;
    composition
        .provider
        .bootstrap_authoring(composition.stream_id)?;
    let asset = composition
        .prepare_named_assets(&[&fixture_name])
        .await?
        .remove(0);
    let authoring = composition.provider.authoring_writer()?;
    let materializer = ReviewMaterializationService::new(
        authoring,
        std::sync::Arc::new(ContinuousReviewPublication::new(
            composition.stream_id,
            composition.provider.clone(),
            composition.assets.clone(),
            composition.evidence.clone(),
        )),
        composition.clock.clone(),
    );
    let mut expected_snapshot_id = None;
    let mut authoring_us = Vec::with_capacity(samples);
    let mut materialization_us = Vec::with_capacity(samples);

    for sample in 0..samples {
        let started_at = Instant::now();
        let envelope = composition
            .service
            .prepare(
                ReviewCommandId::new(),
                expected_snapshot_id,
                ReviewWorkspaceCommand::SaveFeedback {
                    feedback_id: None,
                    text: format!("real corpus {} save {sample}", scenario.name()),
                    targets: vec![TargetEdit::Add {
                        asset_version_id: asset.id,
                        anchor: scenario.anchor()?,
                    }],
                },
            )
            .await?;
        let applied = composition
            .service
            .apply_authoring_with_cancellation(envelope, ReviewTaskCancellation::default())
            .await?;
        serde_json::to_vec(&ReviewAuthoringApplyResultDto::from(applied.clone()))?;
        authoring_us.push(elapsed_us(started_at));
        expected_snapshot_id = Some(applied.receipt.head.snapshot_id);

        let started_at = Instant::now();
        let outcome = materializer
            .run_one(ReviewTaskCancellation::default())
            .await?;
        materialization_us.push(elapsed_us(started_at));
        if !matches!(outcome, ReviewMaterializationOutcome::Published { .. }) {
            return Err(format!(
                "real corpus materialization did not publish for {} sample {sample}: {outcome:?} ({})",
                scenario.name(),
                source_project.display()
            )
            .into());
        }
    }
    Ok((authoring_us, materialization_us))
}

fn summary<'a>(
    phase: &'static str,
    scenario: SaveScenario,
    project: &'a Path,
    source: &'a Path,
    samples: &[u64],
) -> LatencySummary<'a> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    LatencySummary {
        phase,
        scenario: scenario.name(),
        source_project: project,
        source_asset: source,
        samples: samples.len(),
        p50_us: percentile(&sorted, 50),
        p95_us: percentile(&sorted, 95),
        p99_us: percentile(&sorted, 99),
    }
}

fn elapsed_us(started_at: Instant) -> u64 {
    started_at.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    let rank = percentile.saturating_mul(sorted.len()).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn parse_options(arguments: impl Iterator<Item = String>) -> Result<Options, Box<dyn Error>> {
    let mut arguments = arguments;
    let mut project = None;
    let mut samples = 30;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--project" => {
                project = Some(PathBuf::from(
                    arguments.next().ok_or("--project requires a value")?,
                ));
            }
            "--samples" => {
                samples = arguments
                    .next()
                    .ok_or("--samples requires a value")?
                    .parse()?;
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    let project = project.ok_or("--project is required")?;
    if !project.is_dir() {
        return Err(format!("project is not a directory: {}", project.display()).into());
    }
    if !(1..=1_000).contains(&samples) {
        return Err("samples must be between 1 and 1000".into());
    }
    Ok(Options { project, samples })
}

fn first_image(project: &Path) -> Result<PathBuf, Box<dyn Error>> {
    fn visit(directory: &Path, values: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if entry.file_name() != ".viewer" {
                    visit(&path, values)?;
                }
                continue;
            }
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(extension.as_str(), "jpg" | "jpeg" | "png") {
                values.push(path);
            }
        }
        Ok(())
    }

    let mut images = Vec::new();
    visit(project, &mut images)?;
    images.sort();
    images
        .into_iter()
        .next()
        .ok_or_else(|| "project contains no supported image".into())
}
