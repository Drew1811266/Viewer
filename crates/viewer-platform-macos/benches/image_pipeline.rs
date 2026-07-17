use serde::Serialize;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use viewer_application::{ImageBackend, ImageError, ImagePort, ImageRequest};
use viewer_domain::{EntityId, SessionId, image::ImageRepresentationKind};
use viewer_platform_macos::image::{
    ImageIoBackend, MacImagePort, QuickLookBackend, quick_look::QuickLookThumbnailBackend,
};
use viewer_test_support::image_fixtures::image_fixture;

const VALID_FIXTURES: [&str; 4] = ["srgb.jpg", "p3.jpg", "rotated-6.jpg", "alpha.png"];
const THUMBNAIL_ITERATIONS: usize = 20;
const PREVIEW_ITERATIONS: usize = 10;
const CANCELLATION_REQUESTS: usize = 100;
const PREVIEW_P95_LIMIT_MS: f64 = 800.0;
const PEAK_RSS_LIMIT_BYTES: u64 = 700_000_000;

type BenchResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Serialize)]
struct BenchmarkReport {
    schema_version: u16,
    generated_at_unix_seconds: u64,
    device: DeviceReport,
    corpus: Vec<String>,
    thumbnail_measurements: Vec<Measurement>,
    preview_measurements: Vec<Measurement>,
    cancellation: CancellationReport,
    four_proxy: FourProxyReport,
    consistency_artifacts: Vec<ConsistencyArtifact>,
    corrupt_fixture_isolated: bool,
    peak_rss_bytes: u64,
    preview_p95_limit_ms: f64,
    peak_rss_limit_bytes: u64,
    errors: Vec<String>,
    passed: bool,
}

#[derive(Serialize)]
struct DeviceReport {
    chip: String,
    model: String,
    os: String,
    memory_bytes: u64,
}

#[derive(Serialize)]
struct Measurement {
    fixture: String,
    representation: String,
    physical_limit: String,
    iterations: usize,
    cold_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    backends: Vec<String>,
}

#[derive(Serialize)]
struct CancellationReport {
    requested: usize,
    completed_before_cancel: usize,
    active_cancelled: usize,
    other_errors: usize,
    stale_publications: usize,
    stale_files: usize,
    elapsed_ms: f64,
}

#[derive(Serialize)]
struct FourProxyReport {
    requested: usize,
    completed: usize,
    elapsed_ms: f64,
    peak_rss_bytes: u64,
}

#[derive(Serialize)]
struct ConsistencyArtifact {
    fixture: String,
    quick_look_path: String,
    image_io_path: String,
    quick_look_dimensions: (u32, u32),
    image_io_dimensions: (u32, u32),
    quick_look_profile: Option<String>,
    image_io_profile: Option<String>,
}

fn main() {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to create G1 benchmark runtime: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = runtime.block_on(run()) {
        eprintln!("G1 image benchmark failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> BenchResult<()> {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report_directory = repository_root.join("target/g1-image-gate");
    let consistency_directory = report_directory.join("consistency");
    if report_directory.exists() {
        fs::remove_dir_all(&report_directory)?;
    }
    fs::create_dir_all(&consistency_directory)?;

    let cache = tempfile::tempdir()?;
    let port = MacImagePort::new(cache.path())?;
    let mut errors = Vec::new();
    let mut thumbnail_measurements = Vec::new();
    for fixture in VALID_FIXTURES {
        for max_pixels in [256_u32, 512, 1_024] {
            let kind = ImageRepresentationKind::Thumbnail {
                max_pixels,
                scale_milli: 2_000,
            };
            match measure_requests(
                &port,
                fixture,
                kind,
                THUMBNAIL_ITERATIONS,
                format!("{max_pixels}px"),
            )
            .await
            {
                Ok(measurement) => thumbnail_measurements.push(measurement),
                Err(error) => errors.push(format!(
                    "thumbnail {fixture} at {max_pixels}px failed: {error}"
                )),
            }
        }
    }

    let mut preview_measurements = Vec::new();
    for fixture in VALID_FIXTURES {
        let kind = ImageRepresentationKind::FitPreview {
            max_width: 2_560,
            max_height: 1_600,
            scale_milli: 2_000,
        };
        match measure_requests(&port, fixture, kind, PREVIEW_ITERATIONS, "2560x1600".into()).await {
            Ok(measurement) => preview_measurements.push(measurement),
            Err(error) => errors.push(format!("fit preview {fixture} failed: {error}")),
        }
    }

    let cancellation = measure_cancellation().await?;
    let four_proxy = measure_four_proxies().await?;
    let consistency_artifacts = generate_consistency_artifacts(&consistency_directory).await?;
    let corrupt_fixture_isolated = matches!(
        ImageIoBackend::default().probe_sync(image_fixture("corrupt.jpg")),
        Err(ImageError::Corrupt)
    );
    if !corrupt_fixture_isolated {
        errors.push("corrupt JPEG was not isolated as ImageError::Corrupt".into());
    }

    for artifact in &consistency_artifacts {
        if artifact.quick_look_dimensions != artifact.image_io_dimensions {
            errors.push(format!(
                "{} orientation/dimensions differ between backends",
                artifact.fixture
            ));
        }
        if artifact.quick_look_profile != artifact.image_io_profile {
            errors.push(format!(
                "{} color profiles differ between backends: {:?} vs {:?}",
                artifact.fixture, artifact.quick_look_profile, artifact.image_io_profile
            ));
        }
    }

    let peak_rss_bytes = peak_rss_bytes();
    let preview_p95_passed = preview_measurements
        .iter()
        .all(|measurement| measurement.p95_ms <= PREVIEW_P95_LIMIT_MS);
    let passed = errors.is_empty()
        && preview_p95_passed
        && cancellation.other_errors == 0
        && cancellation.stale_publications == 0
        && cancellation.stale_files == 0
        && peak_rss_bytes > 0
        && peak_rss_bytes <= PEAK_RSS_LIMIT_BYTES;
    let report = BenchmarkReport {
        schema_version: 1,
        generated_at_unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        device: device_report(),
        corpus: VALID_FIXTURES.iter().map(ToString::to_string).collect(),
        thumbnail_measurements,
        preview_measurements,
        cancellation,
        four_proxy,
        consistency_artifacts,
        corrupt_fixture_isolated,
        peak_rss_bytes,
        preview_p95_limit_ms: PREVIEW_P95_LIMIT_MS,
        peak_rss_limit_bytes: PEAK_RSS_LIMIT_BYTES,
        errors,
        passed,
    };
    let report_path = report_directory.join("benchmark-report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!("G1 benchmark report: {}", report_path.display());
    println!(
        "Peak RSS: {:.1} MB",
        report.peak_rss_bytes as f64 / 1_000_000.0
    );
    println!("Benchmark gate passed: {}", report.passed);
    if !report.passed {
        return Err(std::io::Error::other("benchmark acceptance failed").into());
    }
    Ok(())
}

async fn measure_requests(
    port: &MacImagePort,
    fixture: &str,
    kind: ImageRepresentationKind,
    iterations: usize,
    physical_limit: String,
) -> BenchResult<Measurement> {
    let session_id = SessionId::new();
    let source = image_fixture(fixture);
    let cold_start = Instant::now();
    let cold = port
        .render(image_request(session_id, source.clone(), kind))
        .await?;
    let cold_ms = elapsed_ms(cold_start.elapsed());
    let mut backends = vec![backend_name(cold.backend).to_owned()];
    remove_artifact(cold.cache_path)?;

    let mut durations = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let artifact = port
            .render(image_request(session_id, source.clone(), kind))
            .await?;
        durations.push(elapsed_ms(start.elapsed()));
        let backend = backend_name(artifact.backend).to_owned();
        if !backends.contains(&backend) {
            backends.push(backend);
        }
        remove_artifact(artifact.cache_path)?;
    }
    durations.sort_by(f64::total_cmp);

    Ok(Measurement {
        fixture: fixture.into(),
        representation: match kind {
            ImageRepresentationKind::Thumbnail { .. } => "thumbnail".into(),
            ImageRepresentationKind::FitPreview { .. } => "fit-preview".into(),
            ImageRepresentationKind::Original100Percent => "original-100-percent".into(),
        },
        physical_limit,
        iterations,
        cold_ms,
        p50_ms: percentile(&durations, 50),
        p95_ms: percentile(&durations, 95),
        backends,
    })
}

async fn measure_cancellation() -> BenchResult<CancellationReport> {
    let cache = tempfile::tempdir()?;
    let port = Arc::new(MacImagePort::new(cache.path())?);
    let started = Instant::now();
    let mut completed_before_cancel = 0;
    let mut active_cancelled = 0;
    let mut other_errors = 0;
    let mut stale_publications = 0;
    let mut stale_files = 0;

    for _ in 0..CANCELLATION_REQUESTS {
        let session_id = SessionId::new();
        let request = image_request(
            session_id,
            image_fixture("p3.jpg"),
            ImageRepresentationKind::Thumbnail {
                max_pixels: 1_024,
                scale_milli: 2_000,
            },
        );
        let task = {
            let port = Arc::clone(&port);
            tokio::spawn(async move { port.render(request).await })
        };
        tokio::task::yield_now().await;
        if task.is_finished() {
            completed_before_cancel += 1;
            match task.await? {
                Ok(artifact) => remove_artifact(artifact.cache_path)?,
                Err(_) => other_errors += 1,
            }
            continue;
        }

        port.cancel_session(session_id).await;
        match task.await? {
            Err(ImageError::Cancelled) => active_cancelled += 1,
            Ok(artifact) => {
                stale_publications += 1;
                remove_artifact(artifact.cache_path)?;
            }
            Err(_) => other_errors += 1,
        }
        let session_directory = cache.path().join(session_id.to_string());
        if session_directory.exists() && fs::read_dir(session_directory)?.next().is_some() {
            stale_files += 1;
        }
    }

    Ok(CancellationReport {
        requested: CANCELLATION_REQUESTS,
        completed_before_cancel,
        active_cancelled,
        other_errors,
        stale_publications,
        stale_files,
        elapsed_ms: elapsed_ms(started.elapsed()),
    })
}

async fn measure_four_proxies() -> BenchResult<FourProxyReport> {
    let cache = tempfile::tempdir()?;
    let port = Arc::new(MacImagePort::new(cache.path())?);
    let session_id = SessionId::new();
    let start = Instant::now();
    let mut tasks = Vec::new();
    for fixture in VALID_FIXTURES {
        let port = Arc::clone(&port);
        let request = image_request(
            session_id,
            image_fixture(fixture),
            ImageRepresentationKind::FitPreview {
                max_width: 3_000,
                max_height: 2_000,
                scale_milli: 2_000,
            },
        );
        tasks.push(tokio::spawn(async move { port.render(request).await }));
    }
    let mut completed = 0;
    for task in tasks {
        let artifact = task.await??;
        completed += 1;
        remove_artifact(artifact.cache_path)?;
    }
    Ok(FourProxyReport {
        requested: VALID_FIXTURES.len(),
        completed,
        elapsed_ms: elapsed_ms(start.elapsed()),
        peak_rss_bytes: peak_rss_bytes(),
    })
}

async fn generate_consistency_artifacts(directory: &Path) -> BenchResult<Vec<ConsistencyArtifact>> {
    let quick_look = QuickLookBackend::new()?;
    let image_io = ImageIoBackend::default();
    let mut artifacts = Vec::new();
    for fixture in VALID_FIXTURES {
        let stem = fixture.replace('.', "-");
        let quick_look_path = directory.join(format!("{stem}-quick-look.png"));
        let image_io_path = directory.join(format!("{stem}-image-io.png"));
        let request = image_request(
            SessionId::new(),
            image_fixture(fixture),
            ImageRepresentationKind::Thumbnail {
                max_pixels: 512,
                scale_milli: 2_000,
            },
        );
        let quick_look_dimensions = quick_look
            .thumbnail(&request, &quick_look_path)
            .await
            .map(|dimensions| (dimensions.width, dimensions.height))?;
        let image_io_dimensions =
            image_io.render_thumbnail_sync(&request.source, 512, &image_io_path)?;
        let quick_look_probe = image_io.probe_sync(&quick_look_path)?;
        let image_io_probe = image_io.probe_sync(&image_io_path)?;
        artifacts.push(ConsistencyArtifact {
            fixture: fixture.into(),
            quick_look_path: path_relative_to(directory, &quick_look_path),
            image_io_path: path_relative_to(directory, &image_io_path),
            quick_look_dimensions,
            image_io_dimensions,
            quick_look_profile: quick_look_probe.icc_profile_name,
            image_io_profile: image_io_probe.icc_profile_name,
        });
    }
    Ok(artifacts)
}

fn image_request(
    session_id: SessionId,
    source: PathBuf,
    kind: ImageRepresentationKind,
) -> ImageRequest {
    ImageRequest {
        session_id,
        entity_id: EntityId::new(),
        source,
        kind,
    }
}

fn remove_artifact(path: PathBuf) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn percentile(sorted: &[f64], percentile: usize) -> f64 {
    let rank = (sorted.len() * percentile).div_ceil(100).max(1) - 1;
    sorted[rank]
}

fn backend_name(backend: ImageBackend) -> &'static str {
    match backend {
        ImageBackend::QuickLook => "quick-look",
        ImageBackend::ImageIo => "image-io",
    }
}

fn elapsed_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into())
}

fn device_report() -> DeviceReport {
    DeviceReport {
        chip: command_output("sysctl", &["-n", "machdep.cpu.brand_string"]),
        model: command_output("sysctl", &["-n", "hw.model"]),
        os: command_output("sw_vers", &["-productVersion"]),
        memory_bytes: command_output("sysctl", &["-n", "hw.memsize"])
            .parse()
            .unwrap_or_default(),
    }
}

#[cfg(target_os = "macos")]
fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` points to writable storage for exactly one `rusage`;
    // `getrusage` initializes it on a zero return code.
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if result != 0 {
        return 0;
    }
    // SAFETY: A zero return from `getrusage` initialized the structure.
    let usage = unsafe { usage.assume_init() };
    u64::try_from(usage.ru_maxrss).unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn peak_rss_bytes() -> u64 {
    0
}
