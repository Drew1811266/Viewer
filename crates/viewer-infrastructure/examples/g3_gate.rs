use serde::Serialize;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use viewer_application::{
    ScanPort, SearchPort,
    scan::{ScanEvent, ScanRequest, ScanTotals},
};
use viewer_domain::{
    SessionId,
    file::FileKind,
    search::{
        Generation, SearchFilters, SearchLayout, SearchQuery, SearchScope, SearchSort,
        SearchSortKey, SortDirection,
    },
};
use viewer_infrastructure::{
    scan::walker::ProjectWalker,
    search::{index::SessionIndex, query::SessionSearch, text::TextExtractor},
};

const EXPECTED_FOLDERS: u64 = 210;
const EXPECTED_FILES: u64 = 1_102;
const FOLDER_P95_LIMIT: Duration = Duration::from_millis(1_500);
const SCAN_P95_LIMIT: Duration = Duration::from_secs(3);
const SEARCH_P95_LIMIT: Duration = Duration::from_millis(100);
const PEAK_RSS_LIMIT_BYTES: u64 = 700_000_000;

type GateResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Serialize)]
struct BenchmarkReport {
    schema_version: u16,
    generated_at_unix_seconds: u64,
    device: DeviceReport,
    runs: usize,
    corpus: CorpusReport,
    first_folder_event: Measurement,
    base_scan: Measurement,
    indexed_search: Measurement,
    peak_rss_bytes: u64,
    peak_rss_limit_bytes: u64,
    fresh_session_index_each_run: bool,
    passed: bool,
}

#[derive(Serialize)]
struct DeviceReport {
    architecture: String,
    operating_system: String,
    processor: String,
}

#[derive(Serialize)]
struct CorpusReport {
    folders: u64,
    files: u64,
    failed: u64,
}

#[derive(Serialize)]
struct Measurement {
    samples: usize,
    p50_ms: f64,
    p95_ms: f64,
    limit_ms: f64,
    passed: bool,
}

struct RunResult {
    first_folder: Duration,
    base_scan: Duration,
    search: Vec<Duration>,
    totals: ScanTotals,
}

fn main() {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to create G3 benchmark runtime: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = runtime.block_on(run()) {
        eprintln!("G3 benchmark failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> GateResult<()> {
    let mut arguments = std::env::args_os().skip(1);
    let corpus = PathBuf::from(arguments.next().ok_or("missing corpus path")?);
    let report_path = PathBuf::from(arguments.next().ok_or("missing report path")?);
    let runs = arguments
        .next()
        .map(|value| value.to_string_lossy().parse::<usize>())
        .transpose()?
        .unwrap_or(20);
    if runs == 0 || arguments.next().is_some() {
        return Err("usage: g3_gate <corpus> <report-path> [positive-runs]".into());
    }
    let corpus = fs::canonicalize(corpus)?;
    let mut folder_samples = Vec::with_capacity(runs);
    let mut scan_samples = Vec::with_capacity(runs);
    let mut search_samples = Vec::with_capacity(runs * 4);
    let mut corpus_totals = None;
    for _ in 0..runs {
        let result = run_once(&corpus).await?;
        if result.totals.folders != EXPECTED_FOLDERS
            || result.totals.files != EXPECTED_FILES
            || result.totals.failed != 0
        {
            return Err(format!(
                "unexpected corpus totals: {} folders, {} files, {} failures",
                result.totals.folders, result.totals.files, result.totals.failed
            )
            .into());
        }
        corpus_totals = Some(result.totals);
        folder_samples.push(result.first_folder);
        scan_samples.push(result.base_scan);
        search_samples.extend(result.search);
    }

    let first_folder_event = measurement(&folder_samples, FOLDER_P95_LIMIT);
    let base_scan = measurement(&scan_samples, SCAN_P95_LIMIT);
    let indexed_search = measurement(&search_samples, SEARCH_P95_LIMIT);
    let peak_rss_bytes = peak_rss_bytes();
    let passed = first_folder_event.passed
        && base_scan.passed
        && indexed_search.passed
        && peak_rss_bytes > 0
        && peak_rss_bytes <= PEAK_RSS_LIMIT_BYTES;
    let totals = corpus_totals.ok_or("benchmark did not run")?;
    let report = BenchmarkReport {
        schema_version: 1,
        generated_at_unix_seconds: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs(),
        device: device_report(),
        runs,
        corpus: CorpusReport {
            folders: totals.folders,
            files: totals.files,
            failed: totals.failed,
        },
        first_folder_event,
        base_scan,
        indexed_search,
        peak_rss_bytes,
        peak_rss_limit_bytes: PEAK_RSS_LIMIT_BYTES,
        fresh_session_index_each_run: true,
        passed,
    };
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!("G3 benchmark report: {}", report_path.display());
    println!("G3 benchmark passed: {}", report.passed);
    if !report.passed {
        return Err("one or more G3 performance budgets failed".into());
    }
    Ok(())
}

async fn run_once(corpus: &Path) -> GateResult<RunResult> {
    let session_directory = tempfile::tempdir()?;
    let index = Arc::new(SessionIndex::open(
        session_directory.path().join("session.sqlite"),
    )?);
    let session_id = SessionId::new();
    let generation = Generation::new(1);
    let (sink, mut events) = tokio::sync::mpsc::channel(8);
    let start = Instant::now();
    let root = corpus.to_path_buf();
    let scan = tokio::spawn(async move {
        ProjectWalker
            .scan(
                ScanRequest {
                    session_id,
                    generation,
                    root,
                },
                sink,
            )
            .await
    });
    let mut first_folder = None;
    let mut base_scan = None;
    let mut totals = None;
    let mut text_nodes = Vec::new();
    while let Some(event) = events.recv().await {
        match event {
            ScanEvent::Folders { generation, nodes } => {
                first_folder.get_or_insert_with(|| start.elapsed());
                index.upsert_batch(&nodes, generation)?;
            }
            ScanEvent::Files { generation, nodes } => {
                text_nodes.extend(
                    nodes
                        .iter()
                        .filter(|node| matches!(node.kind, FileKind::Markdown | FileKind::Text))
                        .cloned(),
                );
                index.upsert_batch(&nodes, generation)?;
            }
            ScanEvent::FailedItem {
                relative_display,
                code,
                ..
            } => {
                return Err(format!("scan failure {code} at {relative_display}").into());
            }
            ScanEvent::Finished { totals: value, .. } => {
                base_scan = Some(start.elapsed());
                totals = Some(value);
            }
        }
    }
    scan.await??;
    for node in &text_nodes {
        let status = TextExtractor::extract(corpus.join(node.relative_path.as_str()))?;
        index.replace_text(node.entity_id, &node.relative_path, &status)?;
    }
    let search = SessionSearch::new(session_id, Arc::clone(&index));
    let mut search_times = Vec::new();
    for text in ["image-0005", "category-05", "产品图", "白色"] {
        let started = Instant::now();
        let page = search
            .search(session_id, generation, search_query(text))
            .await?;
        search_times.push(started.elapsed());
        if page.total == 0 {
            return Err(format!("benchmark query returned no results: {text}").into());
        }
    }
    drop(search);
    drop(index);
    drop(session_directory);
    Ok(RunResult {
        first_folder: first_folder.ok_or("scanner did not publish folders")?,
        base_scan: base_scan.ok_or("scanner did not finish")?,
        search: search_times,
        totals: totals.ok_or("scanner omitted totals")?,
    })
}

fn search_query(text: &str) -> SearchQuery {
    SearchQuery {
        text: text.into(),
        scope: SearchScope::Project,
        filters: SearchFilters::default(),
        sort: SearchSort {
            key: SearchSortKey::Relevance,
            direction: SortDirection::Ascending,
        },
        layout: SearchLayout::Flat,
        offset: 0,
        limit: 50,
    }
}

fn measurement(samples: &[Duration], limit: Duration) -> Measurement {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let p50 = percentile(&sorted, 0.50);
    let p95 = percentile(&sorted, 0.95);
    Measurement {
        samples: sorted.len(),
        p50_ms: milliseconds(p50),
        p95_ms: milliseconds(p95),
        limit_ms: milliseconds(limit),
        passed: p95 <= limit,
    }
}

fn percentile(sorted: &[Duration], quantile: f64) -> Duration {
    let rank = (quantile * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn device_report() -> DeviceReport {
    DeviceReport {
        architecture: std::env::consts::ARCH.into(),
        operating_system: command_output("sw_vers", &["-productVersion"]),
        processor: command_output("sysctl", &["-n", "machdep.cpu.brand_string"]),
    }
}

fn command_output(command: &str, arguments: &[&str]) -> String {
    std::process::Command::new(command)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` points to writable storage for `getrusage`, and the
    // function initializes it on a zero return code.
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if result != 0 {
        return 0;
    }
    // SAFETY: A successful `getrusage` call initialized the structure.
    let usage = unsafe { usage.assume_init() };
    #[cfg(target_os = "macos")]
    let bytes = usage.ru_maxrss as u64;
    #[cfg(not(target_os = "macos"))]
    let bytes = (usage.ru_maxrss as u64).saturating_mul(1_024);
    bytes
}
