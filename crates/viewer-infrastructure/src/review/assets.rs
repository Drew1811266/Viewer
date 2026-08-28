use super::ReviewChangeLedger;
use crate::video_probe::{MediaFileIdentity, VideoMetadataProbe, VideoProbeError};
use async_trait::async_trait;
use std::collections::HashSet;
use std::ffi::CString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;
use viewer_application::{
    BrowseIndexPort, ImageError, ImagePort, PreparedReviewAsset, ReviewAssetCatalogPort,
    ReviewAssetConflictKind, ReviewAssetError, ReviewAssetValidation, ReviewProgressPort,
    ReviewScope, ReviewScopeResolution, ReviewTaskCancellation, ReviewTaskProgress,
    metadata::IndexedNode,
};
use viewer_domain::file::{FileKind, FileNode, ImageIndexStatus};
use viewer_domain::review::{
    AssetEvidence, AssetVersion, MAX_ASSETS_PER_ROUND, ReviewAssetKind, ReviewMedia,
    ReviewabilityFailure,
};
use viewer_domain::video::{VideoFailureKind, VideoMetadata, VideoProbeStatus};
use viewer_domain::{AssetVersionId, EntityId, RelativePath};

const HASH_BUFFER_BYTES: usize = 64 * 1024;
mod continuous;
const MAX_EVIDENCE_CONCURRENCY: usize = 4;

pub struct IndexedReviewAssetCatalog {
    project_root: PathBuf,
    index: Arc<dyn BrowseIndexPort>,
    image: Arc<dyn ImagePort>,
    video: Arc<dyn VideoMetadataProbe>,
    changes: ReviewChangeLedger,
    evidence_gate: Arc<Semaphore>,
    continuous: std::sync::Mutex<continuous::CatalogState>,
}

impl IndexedReviewAssetCatalog {
    pub fn new(
        project_root: &Path,
        index: Arc<dyn BrowseIndexPort>,
        image: Arc<dyn ImagePort>,
        video: Arc<dyn VideoMetadataProbe>,
        changes: ReviewChangeLedger,
    ) -> Result<Self, ReviewAssetError> {
        let metadata =
            fs::symlink_metadata(project_root).map_err(|_| ReviewAssetError::UnsafeSource)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ReviewAssetError::UnsafeSource);
        }
        let project_root =
            fs::canonicalize(project_root).map_err(|_| ReviewAssetError::UnsafeSource)?;
        let concurrency = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .clamp(1, MAX_EVIDENCE_CONCURRENCY);
        Ok(Self {
            project_root,
            index,
            image,
            video,
            changes,
            evidence_gate: Arc::new(Semaphore::new(concurrency)),
            continuous: std::sync::Mutex::new(continuous::CatalogState::default()),
        })
    }

    fn indexed_node(&self, entity_id: EntityId) -> Result<IndexedNode, ReviewAssetError> {
        self.index
            .indexed_node(entity_id)
            .map_err(|_| ReviewAssetError::IndexUnavailable)?
            .ok_or(ReviewAssetError::NotFound)
    }

    fn scope_nodes(&self, scope: &ReviewScope) -> Result<Vec<IndexedNode>, ReviewAssetError> {
        match scope {
            ReviewScope::Selection { entity_ids } => {
                let mut seen = HashSet::with_capacity(entity_ids.len());
                entity_ids
                    .iter()
                    .filter(|entity_id| seen.insert(**entity_id))
                    .map(|entity_id| self.indexed_node(*entity_id))
                    .collect()
            }
            ReviewScope::Folder {
                folder_id,
                include_descendants,
            } => {
                if let Some(folder_id) = folder_id {
                    let folder = self.indexed_node(*folder_id)?;
                    if folder.node.kind != FileKind::Directory {
                        return Err(ReviewAssetError::InvalidScope);
                    }
                }
                let nodes = if *include_descendants {
                    self.index.descendants(*folder_id)
                } else {
                    self.index.direct_children(*folder_id)
                }
                .map_err(|_| ReviewAssetError::IndexUnavailable)?;
                nodes
                    .into_iter()
                    .map(|node| self.indexed_node(node.entity_id))
                    .collect()
            }
        }
    }

    fn exact_nodes(&self, entity_ids: &[EntityId]) -> Result<Vec<IndexedNode>, ReviewAssetError> {
        if entity_ids.len() > MAX_ASSETS_PER_ROUND {
            return Err(ReviewAssetError::LimitExceeded);
        }
        let mut seen = HashSet::with_capacity(entity_ids.len());
        let mut nodes = entity_ids
            .iter()
            .filter(|entity_id| seen.insert(**entity_id))
            .map(|entity_id| self.indexed_node(*entity_id))
            .collect::<Result<Vec<_>, _>>()?;
        nodes.sort_by(|left, right| {
            left.node
                .relative_path
                .as_str()
                .cmp(right.node.relative_path.as_str())
                .then_with(|| {
                    left.node
                        .entity_id
                        .to_string()
                        .cmp(&right.node.entity_id.to_string())
                })
        });
        if nodes
            .iter()
            .any(|node| candidate_kind(node.node.kind).is_none())
        {
            return Err(ReviewAssetError::InvalidScope);
        }
        Ok(nodes)
    }
}

#[async_trait]
impl ReviewAssetCatalogPort for IndexedReviewAssetCatalog {
    async fn resolve_scope(
        &self,
        scope: &ReviewScope,
    ) -> Result<ReviewScopeResolution, ReviewAssetError> {
        let nodes = self.scope_nodes(scope)?;
        let mut candidates = Vec::new();
        let mut image_count = 0_u32;
        let mut video_count = 0_u32;
        let mut excluded_count = 0_u32;
        for indexed in nodes {
            match candidate_kind(indexed.node.kind) {
                Some(ReviewAssetKind::Image) => {
                    image_count = image_count
                        .checked_add(1)
                        .ok_or(ReviewAssetError::LimitExceeded)?;
                    candidates.push(indexed.node);
                }
                Some(ReviewAssetKind::Video) => {
                    video_count = video_count
                        .checked_add(1)
                        .ok_or(ReviewAssetError::LimitExceeded)?;
                    candidates.push(indexed.node);
                }
                None => {
                    excluded_count = excluded_count
                        .checked_add(1)
                        .ok_or(ReviewAssetError::LimitExceeded)?;
                }
            }
        }
        candidates.sort_by(|left, right| {
            left.relative_path
                .as_str()
                .cmp(right.relative_path.as_str())
                .then_with(|| left.entity_id.to_string().cmp(&right.entity_id.to_string()))
        });
        candidates.dedup_by_key(|node| node.entity_id);
        if candidates.len() > MAX_ASSETS_PER_ROUND {
            return Err(ReviewAssetError::LimitExceeded);
        }
        Ok(ReviewScopeResolution {
            candidate_entity_ids: candidates.into_iter().map(|node| node.entity_id).collect(),
            image_count,
            video_count,
            excluded_count,
        })
    }

    async fn prepare_assets(
        &self,
        entity_ids: &[EntityId],
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError> {
        if cancellation.is_cancelled() {
            self.changes.clear();
            return Err(ReviewAssetError::Cancelled);
        }
        let nodes = match self.exact_nodes(entity_ids) {
            Ok(nodes) => nodes,
            Err(error) => {
                self.changes.clear();
                return Err(error);
            }
        };
        let tracked = nodes
            .iter()
            .map(|indexed| indexed.node.entity_id)
            .collect::<Vec<_>>();
        self.changes.replace_members(&tracked);
        let total = u32::try_from(nodes.len()).map_err(|_| ReviewAssetError::LimitExceeded)?;
        let mut tasks = JoinSet::new();
        for (position, indexed) in nodes.into_iter().enumerate() {
            let root = self.project_root.clone();
            let image = Arc::clone(&self.image);
            let video = Arc::clone(&self.video);
            let changes = self.changes.clone();
            let cancellation = cancellation.clone();
            let gate = Arc::clone(&self.evidence_gate);
            tasks.spawn(async move {
                let _permit = tokio::select! {
                    permit = gate.acquire_owned() => {
                        permit.map_err(|_| ReviewAssetError::Unavailable)?
                    }
                    () = wait_until_cancelled(cancellation.clone()) => {
                        return Err(ReviewAssetError::Cancelled);
                    }
                };
                let prepared =
                    prepare_one(root, indexed, image, video, changes, cancellation).await?;
                Ok::<_, ReviewAssetError>((position, prepared))
            });
        }

        let mut prepared = Vec::with_capacity(tracked.len());
        let mut completed = 0_u32;
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(item)) => {
                    prepared.push(item);
                    completed = completed.saturating_add(1);
                    progress.report(ReviewTaskProgress { completed, total });
                }
                Ok(Err(error)) => {
                    tasks.abort_all();
                    self.changes.clear();
                    return Err(error);
                }
                Err(_) => {
                    tasks.abort_all();
                    self.changes.clear();
                    return Err(ReviewAssetError::Unavailable);
                }
            }
        }
        prepared.sort_by_key(|(position, _)| *position);
        Ok(prepared.into_iter().map(|(_, asset)| asset).collect())
    }

    async fn revalidate_assets(
        &self,
        assets: &[PreparedReviewAsset],
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<ReviewAssetValidation>, ReviewAssetError> {
        let total = u32::try_from(assets.len()).map_err(|_| ReviewAssetError::LimitExceeded)?;
        let mut validations = Vec::with_capacity(assets.len());
        for prepared in assets {
            if cancellation.is_cancelled() {
                return Err(ReviewAssetError::Cancelled);
            }
            validations.push(self.revalidate_one(prepared, &cancellation).await?);
            progress.report(ReviewTaskProgress {
                completed: u32::try_from(validations.len())
                    .map_err(|_| ReviewAssetError::LimitExceeded)?,
                total,
            });
        }
        Ok(validations)
    }

    fn release_tracking(&self) {
        self.changes.clear();
    }
}

impl IndexedReviewAssetCatalog {
    async fn revalidate_one(
        &self,
        prepared: &PreparedReviewAsset,
        cancellation: &ReviewTaskCancellation,
    ) -> Result<ReviewAssetValidation, ReviewAssetError> {
        let conflict = |kind| ReviewAssetValidation::Conflict {
            asset_version_id: prepared.asset.id,
            relative_path: prepared.asset.relative_path.clone(),
            kind,
        };
        let pending = || ReviewAssetValidation::Pending {
            asset_version_id: prepared.asset.id,
            relative_path: prepared.asset.relative_path.clone(),
        };
        if prepared.asset.source_entity_id != Some(prepared.entity_id) {
            return Ok(conflict(ReviewAssetConflictKind::Replaced));
        }
        let indexed = match self.index.indexed_node(prepared.entity_id) {
            Ok(Some(indexed)) => indexed,
            Ok(None) => return Ok(conflict(ReviewAssetConflictKind::Missing)),
            Err(_) => return Err(ReviewAssetError::IndexUnavailable),
        };
        if indexed.node.relative_path != prepared.asset.relative_path {
            return Ok(conflict(ReviewAssetConflictKind::Moved));
        }
        if indexed.node.size != prepared.asset.evidence.size_bytes {
            return Ok(conflict(ReviewAssetConflictKind::SizeChanged));
        }
        if candidate_kind(indexed.node.kind) != Some(asset_kind(&prepared.asset.media)) {
            return Ok(conflict(ReviewAssetConflictKind::MediaChanged));
        }
        let current_metadata =
            match validate_current_owned_metadata(&self.project_root, &indexed.node) {
                Ok(metadata) => metadata,
                Err(_) => return Ok(conflict(ReviewAssetConflictKind::Replaced)),
            };
        let current_modified_ns = modified_ns(&current_metadata);
        let current_revision = self.changes.revision(prepared.entity_id);
        let needs_hash = current_modified_ns != prepared.asset.evidence.modified_ns
            || current_revision != prepared.change_revision;

        let mut source_failure = None;
        let mut current_digest = prepared.asset.evidence.blake3;
        let media_identity;
        if needs_hash || prepared.asset.evidence.blake3.is_none() {
            let root = self.project_root.clone();
            let mut node = indexed.node.clone();
            node.modified_ns = current_modified_ns;
            let cancellation = cancellation.clone();
            let capture =
                tokio::task::spawn_blocking(move || capture_source(&root, &node, &cancellation))
                    .await
                    .map_err(|_| ReviewAssetError::Unavailable)?;
            let capture = match capture {
                Ok(capture) => capture,
                Err(ReviewAssetError::Cancelled) => return Err(ReviewAssetError::Cancelled),
                Err(_) => return Ok(conflict(ReviewAssetConflictKind::Replaced)),
            };
            source_failure = capture.failure;
            current_digest = capture.digest;
            media_identity = capture.media_identity;
            if prepared.asset.evidence.blake3 != current_digest {
                return Ok(conflict(ReviewAssetConflictKind::ContentChanged));
            }
        } else {
            media_identity = MediaFileIdentity::from_metadata(&current_metadata);
        }

        let assessment = match assess_media(
            &self.image,
            &self.video,
            &self.project_root.join(indexed.node.relative_path.as_str()),
            &indexed,
            source_failure,
            &media_identity,
            cancellation,
        )
        .await
        {
            Ok(assessment) => assessment,
            Err(ReviewAssetError::SourceChanged | ReviewAssetError::UnsafeSource) => {
                return Ok(conflict(ReviewAssetConflictKind::Replaced));
            }
            Err(error) => return Err(error),
        };
        let current_after = match validate_current_owned_metadata(&self.project_root, &indexed.node)
        {
            Ok(metadata) => metadata,
            Err(_) => return Ok(conflict(ReviewAssetConflictKind::Replaced)),
        };
        if !same_metadata(&current_metadata, &current_after) {
            return Ok(conflict(ReviewAssetConflictKind::Replaced));
        }
        let (media, failure) = match assessment {
            FailureAssessment::Pending => return Ok(pending()),
            FailureAssessment::Known { media, failure } => (media, failure),
        };
        if media != prepared.asset.media || failure != prepared.failure {
            return Ok(conflict(ReviewAssetConflictKind::MediaChanged));
        }
        let mut current = prepared.clone();
        current.asset.evidence.modified_ns = current_modified_ns;
        current.asset.evidence.blake3 = current_digest;
        current.change_revision = current_revision;
        current.source_path = self.project_root.join(current.asset.relative_path.as_str());
        Ok(ReviewAssetValidation::Current(current))
    }
}

async fn prepare_one(
    project_root: PathBuf,
    indexed: IndexedNode,
    image: Arc<dyn ImagePort>,
    video: Arc<dyn VideoMetadataProbe>,
    changes: ReviewChangeLedger,
    cancellation: ReviewTaskCancellation,
) -> Result<PreparedReviewAsset, ReviewAssetError> {
    if cancellation.is_cancelled() {
        return Err(ReviewAssetError::Cancelled);
    }
    let change_revision = changes.revision(indexed.node.entity_id);
    let root = project_root.clone();
    let node = indexed.node.clone();
    let hash_cancellation = cancellation.clone();
    let captured =
        tokio::task::spawn_blocking(move || capture_source(&root, &node, &hash_cancellation))
            .await
            .map_err(|_| ReviewAssetError::Unavailable)??;
    let assessment = assess_media(
        &image,
        &video,
        &project_root.join(indexed.node.relative_path.as_str()),
        &indexed,
        captured.failure,
        &captured.media_identity,
        &cancellation,
    )
    .await?;
    validate_owned_metadata(&project_root, &indexed.node)?;
    let FailureAssessment::Known { media, failure } = assessment else {
        return Err(ReviewAssetError::Pending);
    };
    let source_path = project_root.join(indexed.node.relative_path.as_str());
    Ok(PreparedReviewAsset {
        entity_id: indexed.node.entity_id,
        asset: AssetVersion {
            id: AssetVersionId::new(),
            source_entity_id: Some(indexed.node.entity_id),
            relative_path: indexed.node.relative_path,
            evidence: AssetEvidence {
                size_bytes: indexed.node.size,
                modified_ns: indexed.node.modified_ns,
                blake3: captured.digest,
            },
            media,
            producer_asset_id: None,
            parent_asset_version_id: None,
        },
        failure,
        change_revision,
        source_path,
    })
}

struct CapturedSource {
    digest: Option<[u8; 32]>,
    failure: Option<ReviewabilityFailure>,
    media_identity: MediaFileIdentity,
}

fn capture_source(
    root: &Path,
    node: &FileNode,
    cancellation: &ReviewTaskCancellation,
) -> Result<CapturedSource, ReviewAssetError> {
    let path_metadata = validate_owned_metadata(root, node)?;
    let mut file = match open_owned_source(root, &node.relative_path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Ok(CapturedSource {
                digest: None,
                failure: Some(ReviewabilityFailure::PermissionDenied),
                media_identity: MediaFileIdentity::from_metadata(&path_metadata),
            });
        }
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
            return Err(ReviewAssetError::UnsafeSource);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ReviewAssetError::SourceChanged);
        }
        Err(_) => return Err(ReviewAssetError::Unavailable),
    };
    let opened_before = file.metadata().map_err(|_| ReviewAssetError::Unavailable)?;
    if !same_source(&path_metadata, &opened_before, node) {
        return Err(ReviewAssetError::SourceChanged);
    }
    let digest = hash_blake3_streaming(&mut file, cancellation)?;
    let opened_after = file.metadata().map_err(|_| ReviewAssetError::Unavailable)?;
    let path_after = validate_owned_metadata(root, node)?;
    if !same_metadata(&opened_before, &opened_after) || !same_metadata(&opened_after, &path_after) {
        return Err(ReviewAssetError::SourceChanged);
    }
    Ok(CapturedSource {
        digest: Some(digest),
        failure: None,
        media_identity: MediaFileIdentity::from_metadata(&opened_after),
    })
}

fn candidate_kind(kind: FileKind) -> Option<ReviewAssetKind> {
    match kind {
        FileKind::Jpeg | FileKind::Png | FileKind::UnsupportedImage => Some(ReviewAssetKind::Image),
        FileKind::Video => Some(ReviewAssetKind::Video),
        FileKind::Directory | FileKind::Markdown | FileKind::Text | FileKind::Other => None,
    }
}

fn asset_kind(media: &ReviewMedia) -> ReviewAssetKind {
    match media {
        ReviewMedia::Image { .. } => ReviewAssetKind::Image,
        ReviewMedia::Video { .. } => ReviewAssetKind::Video,
    }
}

enum FailureAssessment {
    Known {
        media: ReviewMedia,
        failure: Option<ReviewabilityFailure>,
    },
    Pending,
}

async fn assess_media(
    image: &Arc<dyn ImagePort>,
    video: &Arc<dyn VideoMetadataProbe>,
    source: &Path,
    indexed: &IndexedNode,
    source_failure: Option<ReviewabilityFailure>,
    media_identity: &MediaFileIdentity,
    cancellation: &ReviewTaskCancellation,
) -> Result<FailureAssessment, ReviewAssetError> {
    if cancellation.is_cancelled() {
        return Err(ReviewAssetError::Cancelled);
    }
    if let Some(failure) = source_failure {
        return Ok(FailureAssessment::Known {
            media: unavailable_media(indexed.node.kind),
            failure: Some(failure),
        });
    }
    match indexed.node.kind {
        FileKind::UnsupportedImage => Ok(FailureAssessment::Known {
            media: ReviewMedia::Image {
                width: None,
                height: None,
            },
            failure: Some(ReviewabilityFailure::Unsupported),
        }),
        FileKind::Jpeg | FileKind::Png => {
            if let (ImageIndexStatus::Ready, Some(metadata)) =
                (indexed.image_status, indexed.image_metadata)
            {
                return Ok(FailureAssessment::Known {
                    media: ReviewMedia::Image {
                        width: Some(metadata.width),
                        height: Some(metadata.height),
                    },
                    failure: None,
                });
            }
            let probe = tokio::select! {
                result = image.probe(source) => result,
                () = wait_until_cancelled(cancellation.clone()) => {
                    return Err(ReviewAssetError::Cancelled);
                }
            };
            match probe {
                Ok(probe) if probe.width > 0 && probe.height > 0 => Ok(FailureAssessment::Known {
                    media: ReviewMedia::Image {
                        width: Some(probe.width),
                        height: Some(probe.height),
                    },
                    failure: None,
                }),
                Ok(_) | Err(ImageError::BudgetExceeded | ImageError::Io(_)) => {
                    Ok(FailureAssessment::Pending)
                }
                Err(ImageError::Unsupported) => Ok(FailureAssessment::Known {
                    media: unavailable_media(indexed.node.kind),
                    failure: Some(ReviewabilityFailure::Unsupported),
                }),
                Err(ImageError::Corrupt) => Ok(FailureAssessment::Known {
                    media: unavailable_media(indexed.node.kind),
                    failure: Some(ReviewabilityFailure::Damaged),
                }),
                Err(ImageError::Cancelled) => Err(ReviewAssetError::Cancelled),
            }
        }
        FileKind::Video => {
            if let Some(metadata) = indexed.video_metadata.as_ref()
                && metadata.probe_status == VideoProbeStatus::Ready
                && valid_video_media(metadata)
            {
                return Ok(FailureAssessment::Known {
                    media: video_media(metadata),
                    failure: None,
                });
            }
            let token = CancellationToken::new();
            if cancellation.is_cancelled() {
                token.cancel();
            }
            let probe_token = token.clone();
            let result = tokio::select! {
                result = video.probe_identity_bound(source, media_identity, probe_token) => result,
                () = wait_until_cancelled(cancellation.clone()) => {
                    token.cancel();
                    return Err(ReviewAssetError::Cancelled);
                }
            };
            match result {
                Ok(metadata) => assess_video_metadata(&metadata),
                Err(VideoProbeError::Failed(failure)) => assess_video_failure(failure),
                Err(VideoProbeError::Cancelled) => Err(ReviewAssetError::Cancelled),
                Err(VideoProbeError::SourceChanged) => Err(ReviewAssetError::SourceChanged),
            }
        }
        FileKind::Directory | FileKind::Markdown | FileKind::Text | FileKind::Other => {
            Err(ReviewAssetError::InvalidScope)
        }
    }
}

fn assess_video_metadata(metadata: &VideoMetadata) -> Result<FailureAssessment, ReviewAssetError> {
    match metadata.probe_status {
        VideoProbeStatus::Ready if valid_video_media(metadata) => Ok(FailureAssessment::Known {
            media: video_media(metadata),
            failure: None,
        }),
        VideoProbeStatus::Ready | VideoProbeStatus::Pending => Ok(FailureAssessment::Pending),
        VideoProbeStatus::Failed(failure) => assess_video_failure(failure),
    }
}

fn assess_video_failure(failure: VideoFailureKind) -> Result<FailureAssessment, ReviewAssetError> {
    let failure = match failure {
        VideoFailureKind::Unsupported => ReviewabilityFailure::Unsupported,
        VideoFailureKind::Damaged => ReviewabilityFailure::Damaged,
        VideoFailureKind::Unreadable => ReviewabilityFailure::Unreadable,
        VideoFailureKind::Missing => ReviewabilityFailure::Missing,
        VideoFailureKind::DecodeFallbackFailed => ReviewabilityFailure::DecodeFailed,
        VideoFailureKind::EngineInitialization
        | VideoFailureKind::RenderSurface
        | VideoFailureKind::ThumbnailUnavailable => return Ok(FailureAssessment::Pending),
    };
    Ok(FailureAssessment::Known {
        media: ReviewMedia::Video {
            duration_us: None,
            display_width: None,
            display_height: None,
        },
        failure: Some(failure),
    })
}

fn unavailable_media(kind: FileKind) -> ReviewMedia {
    if kind == FileKind::Video {
        ReviewMedia::Video {
            duration_us: None,
            display_width: None,
            display_height: None,
        }
    } else {
        ReviewMedia::Image {
            width: None,
            height: None,
        }
    }
}

fn valid_video_media(metadata: &VideoMetadata) -> bool {
    metadata.duration_us.is_none_or(|duration| duration > 0)
        && match (metadata.display_width, metadata.display_height) {
            (None, None) => true,
            (Some(width), Some(height)) => width > 0 && height > 0,
            _ => false,
        }
}

fn video_media(metadata: &VideoMetadata) -> ReviewMedia {
    ReviewMedia::Video {
        duration_us: metadata.duration_us,
        display_width: metadata.display_width,
        display_height: metadata.display_height,
    }
}

fn validate_owned_metadata(root: &Path, node: &FileNode) -> Result<Metadata, ReviewAssetError> {
    let metadata = validate_current_owned_metadata(root, node)?;
    if modified_ns(&metadata) != node.modified_ns {
        return Err(ReviewAssetError::SourceChanged);
    }
    Ok(metadata)
}

fn validate_current_owned_metadata(
    root: &Path,
    node: &FileNode,
) -> Result<Metadata, ReviewAssetError> {
    let mut current = root.to_path_buf();
    let components = node.relative_path.as_str().split('/').collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| ReviewAssetError::SourceChanged)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ReviewAssetError::UnsafeSource);
        }
    }
    let metadata = fs::symlink_metadata(root.join(node.relative_path.as_str()))
        .map_err(|_| ReviewAssetError::SourceChanged)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ReviewAssetError::UnsafeSource);
    }
    if entity_id(&metadata, &node.relative_path) != node.entity_id || metadata.len() != node.size {
        return Err(ReviewAssetError::SourceChanged);
    }
    Ok(metadata)
}

async fn wait_until_cancelled(cancellation: ReviewTaskCancellation) {
    while !cancellation.is_cancelled() {
        sleep(Duration::from_millis(10)).await;
    }
}

fn open_owned_source(root: &Path, relative: &RelativePath) -> io::Result<File> {
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    let components = relative.as_str().split('/').collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        let component = CString::new(component.as_bytes())?;
        let descriptor = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                component.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        directory = unsafe { File::from_raw_fd(descriptor) };
    }
    let leaf = components
        .last()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty relative path"))?;
    let leaf = CString::new(leaf.as_bytes())?;
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if descriptor < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }
}

fn hash_blake3_streaming(
    file: &mut File,
    cancellation: &ReviewTaskCancellation,
) -> Result<[u8; 32], ReviewAssetError> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    loop {
        if cancellation.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        let read = file
            .read(&mut buffer)
            .map_err(|_| ReviewAssetError::Unavailable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn same_source(path: &Metadata, opened: &Metadata, node: &FileNode) -> bool {
    same_metadata(path, opened)
        && entity_id(opened, &node.relative_path) == node.entity_id
        && opened.len() == node.size
        && modified_ns(opened) == node.modified_ns
}

fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && modified_ns(left) == modified_ns(right)
        && changed_ns(left) == changed_ns(right)
        && file_identity(left) == file_identity(right)
}

#[cfg(unix)]
fn file_identity(metadata: &Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(unix)]
fn entity_id(metadata: &Metadata, _relative: &RelativePath) -> EntityId {
    let (device, inode) = file_identity(metadata);
    EntityId::from_u128((u128::from(device) << 64) | u128::from(inode))
}

#[cfg(unix)]
fn modified_ns(metadata: &Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

#[cfg(unix)]
fn changed_ns(metadata: &Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec())
}
