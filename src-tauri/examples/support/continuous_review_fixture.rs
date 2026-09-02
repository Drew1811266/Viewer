#![allow(dead_code)]

use std::{error::Error, fs, os::unix::fs::MetadataExt, path::Path, str::FromStr, sync::Arc};

use tempfile::TempDir;
use viewer_application::{
    BrowseIndexPort, ClockPort, ImagePort, ProjectAccess, ReviewTaskCancellation,
    review_assets::ContinuousReviewAssetPort,
    review_evidence::ReviewEvidencePort,
    review_workspace::{
        ContinuousReviewRepositoryProviderPort, ContinuousReviewService, ReviewWorkspaceContext,
    },
};
use viewer_domain::{
    EntityId, ProjectId, RelativePath, ReviewStreamId,
    file::{FileKind, FileNode},
    review::AssetVersion,
    search::Generation,
};
use viewer_infrastructure::{
    SystemClock,
    portable::PortableProjectMetadata,
    review::{
        ContinuousReviewCommandCodec, IndexedReviewAssetCatalog, ProjectReviewRepositoryProvider,
        ReviewChangeLedger,
    },
    search::index::SessionIndex,
    video_probe::UnavailableVideoProbe,
};
use viewer_platform_macos::image::{MacImagePort, MacReviewEvidenceRenderer};

pub fn project_id() -> ProjectId {
    ProjectId::from_str("00000000-0000-4000-8000-000000000001").expect("fixed harness project id")
}

pub struct Composition {
    _cache: TempDir,
    pub provider: Arc<ProjectReviewRepositoryProvider>,
    pub service: ContinuousReviewService,
    pub assets: Arc<dyn ContinuousReviewAssetPort>,
    pub evidence: Arc<dyn ReviewEvidencePort>,
    pub clock: Arc<dyn ClockPort>,
    pub nodes: Vec<FileNode>,
    pub project_id: ProjectId,
    pub stream_id: ReviewStreamId,
}

impl Composition {
    pub async fn prepare_named_assets(
        &self,
        names: &[&str],
    ) -> Result<Vec<AssetVersion>, Box<dyn Error>> {
        let entities = names
            .iter()
            .map(|name| entity_for_path(&self.nodes, name))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self
            .service
            .prepare_assets(&entities, ReviewTaskCancellation::default())
            .await?)
    }
}

pub fn build_composition(project: &Path) -> Result<Composition, Box<dyn Error>> {
    let provider = Arc::new(ProjectReviewRepositoryProvider::new(project, project_id()));
    let service_provider: Arc<dyn ContinuousReviewRepositoryProviderPort> = provider.clone();
    let stream_id = provider.manual_review_stream()?;
    build_composition_with_context(
        project,
        provider,
        service_provider,
        ReviewWorkspaceContext {
            project_id: project_id(),
            stream_id,
            production: None,
        },
    )
}

/// Builds the same persistent project identity boundary used by the desktop session before the
/// review provider is created. Performance harnesses use this instead of the deterministic
/// protocol-fixture identity so the authoring SQLite store is exercised realistically.
pub fn build_portable_composition(project: &Path) -> Result<Composition, Box<dyn Error>> {
    let metadata = PortableProjectMetadata::open(project, ProjectAccess::ReadWrite, 0)?;
    let persistent_project_id = metadata.project_id();
    drop(metadata);
    let provider = Arc::new(ProjectReviewRepositoryProvider::new(
        project,
        persistent_project_id,
    ));
    let service_provider: Arc<dyn ContinuousReviewRepositoryProviderPort> = provider.clone();
    let stream_id = provider.manual_review_stream()?;
    build_composition_with_context(
        project,
        provider,
        service_provider,
        ReviewWorkspaceContext {
            project_id: persistent_project_id,
            stream_id,
            production: None,
        },
    )
}

pub fn build_composition_with_provider(
    project: &Path,
    provider: Arc<ProjectReviewRepositoryProvider>,
    service_provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
) -> Result<Composition, Box<dyn Error>> {
    let stream_id = provider.manual_review_stream()?;
    build_composition_with_context(
        project,
        provider,
        service_provider,
        ReviewWorkspaceContext {
            project_id: project_id(),
            stream_id,
            production: None,
        },
    )
}

pub fn build_composition_with_context(
    project: &Path,
    provider: Arc<ProjectReviewRepositoryProvider>,
    service_provider: Arc<dyn ContinuousReviewRepositoryProviderPort>,
    context: ReviewWorkspaceContext,
) -> Result<Composition, Box<dyn Error>> {
    let cache = tempfile::tempdir()?;
    let index = Arc::new(SessionIndex::open(
        cache.path().join("review-index.sqlite"),
    )?);
    let nodes = indexed_media_nodes(project)?;
    index.upsert_batch(&nodes, Generation::new(1))?;
    let browse: Arc<dyn BrowseIndexPort> = index;
    let image: Arc<dyn ImagePort> = Arc::new(MacImagePort::new(cache.path().join("images"))?);
    let catalog: Arc<dyn ContinuousReviewAssetPort> = Arc::new(IndexedReviewAssetCatalog::new(
        project,
        browse,
        image,
        Arc::new(UnavailableVideoProbe),
        ReviewChangeLedger::default(),
    )?);
    let evidence_root = cache.path().join("continuous-evidence");
    fs::create_dir(&evidence_root)?;
    let evidence: Arc<dyn ReviewEvidencePort> =
        Arc::new(MacReviewEvidenceRenderer::new(&evidence_root)?);
    let clock: Arc<dyn ClockPort> = Arc::new(SystemClock);
    let service = ContinuousReviewService::new(
        context.clone(),
        service_provider,
        catalog.clone(),
        evidence.clone(),
        Arc::new(ContinuousReviewCommandCodec),
        clock.clone(),
    );
    Ok(Composition {
        _cache: cache,
        provider,
        service,
        assets: catalog,
        evidence,
        clock,
        nodes,
        project_id: context.project_id,
        stream_id: context.stream_id,
    })
}

fn indexed_media_nodes(project: &Path) -> Result<Vec<FileNode>, Box<dyn Error>> {
    let mut nodes = Vec::new();
    for entry in fs::read_dir(project)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        let extension = Path::new(&filename)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let kind = match extension.as_str() {
            "jpg" | "jpeg" => FileKind::Jpeg,
            "png" => FileKind::Png,
            "mp4" | "mov" | "mkv" | "webm" => FileKind::Video,
            _ => continue,
        };
        nodes.push(FileNode {
            entity_id: entity_id(&metadata),
            relative_path: RelativePath::parse(&filename)?,
            kind,
            size: metadata.len(),
            modified_ns: modified_ns(&metadata),
        });
    }
    nodes.sort_by(|left, right| {
        left.relative_path
            .as_str()
            .cmp(right.relative_path.as_str())
    });
    if nodes.is_empty() {
        return Err("scenario project contains no indexed media".into());
    }
    Ok(nodes)
}

fn entity_for_path(nodes: &[FileNode], path: &str) -> Result<EntityId, Box<dyn Error>> {
    nodes
        .iter()
        .find(|node| node.relative_path.as_str() == path)
        .map(|node| node.entity_id)
        .ok_or_else(|| format!("missing scenario asset: {path}").into())
}

fn entity_id(metadata: &fs::Metadata) -> EntityId {
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

fn modified_ns(metadata: &fs::Metadata) -> i128 {
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}
