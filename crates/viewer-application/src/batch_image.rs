//! Batch image processing use case (0.2.4, ADR 0008).
//!
//! One batch = one write-lane task: files convert sequentially, progress is
//! reported per file, cancellation takes effect between files, and the batch
//! is never undoable (outputs are new files; the preview is the safety net).

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use viewer_domain::{
    EntityId,
    batch_image::{
        BatchImageConflictStrategy, BatchImageFailureReason, BatchImageItemOutcome,
        BatchImageOutput, BatchImagePlanDisposition, BatchImagePlanRow, BatchImageRejectReason,
        BatchImageSettings, BatchImageSkipReason, BatchImageTargetFormat,
    },
    operation::{BatchId, BatchLifecycle, BatchProgress},
};

use crate::ImageError;

/// Filesystem + encoding operations the batch service needs. The desktop
/// composition root implements this on top of std::fs, the macOS trash port,
/// and the platform image transformer.
#[async_trait::async_trait]
pub trait BatchImageIoPort: Send + Sync {
    async fn encode(
        &self,
        source: &Path,
        settings: &BatchImageSettings,
        temp: &Path,
    ) -> Result<(u32, u32, u64), ImageError>;
    async fn trash(&self, path: &Path) -> Result<(), String>;
    fn path_exists(&self, path: &Path) -> bool;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), String>;
    fn create_directory(&self, path: &Path) -> Result<(), String>;
    fn available_formats(&self) -> Vec<BatchImageTargetFormat>;
}

pub struct BatchImageService {
    io: Arc<dyn BatchImageIoPort>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchImageExecution {
    pub batch_id: BatchId,
    pub outcomes: Vec<(EntityId, BatchImageItemOutcome)>,
}

impl BatchImageService {
    pub fn new(io: Arc<dyn BatchImageIoPort>) -> Self {
        Self { io }
    }

    pub fn io(&self) -> &Arc<dyn BatchImageIoPort> {
        &self.io
    }

    /// Compute the per-file preflight preview: destination paths, conflict
    /// dispositions, and rejections. Pure with respect to file contents; it
    /// only probes destination existence.
    pub fn plan(
        &self,
        root: &Path,
        nodes: &[viewer_domain::file::FileNode],
        settings: &BatchImageSettings,
    ) -> Vec<BatchImagePlanRow> {
        nodes
            .iter()
            .map(|node| BatchImagePlanRow {
                entity_id: node.entity_id,
                relative_path: node.relative_path.as_str().to_owned(),
                disposition: self.plan_row(root, node, settings),
            })
            .collect()
    }

    fn plan_row(
        &self,
        root: &Path,
        node: &viewer_domain::file::FileNode,
        settings: &BatchImageSettings,
    ) -> BatchImagePlanDisposition {
        if !node.kind.is_previewable_image() {
            return BatchImagePlanDisposition::Rejected {
                reason: BatchImageRejectReason::NotAnImage,
            };
        }
        let Some(destination) = destination_path(root, node, settings) else {
            return BatchImagePlanDisposition::Rejected {
                reason: BatchImageRejectReason::UnwritableFormat,
            };
        };
        if !self.io.path_exists(&destination) {
            return BatchImagePlanDisposition::Convert {
                destination: display_relative(root, &destination),
            };
        }
        match settings.conflict {
            BatchImageConflictStrategy::Skip => BatchImagePlanDisposition::SkippedExisting,
            BatchImageConflictStrategy::Suffix => {
                match free_suffixed_path(&self.io, &destination) {
                    Some(free) => BatchImagePlanDisposition::ConvertSuffixed {
                        destination: display_relative(root, &free),
                    },
                    None => BatchImagePlanDisposition::SkippedExisting,
                }
            }
            BatchImageConflictStrategy::Overwrite => BatchImagePlanDisposition::OverwriteExisting {
                destination: display_relative(root, &destination),
            },
        }
    }

    /// Execute the batch sequentially. `progress` receives one update per
    /// file plus the terminal lifecycle transition.
    pub async fn execute(
        &self,
        root: &Path,
        nodes: &[viewer_domain::file::FileNode],
        settings: &BatchImageSettings,
        cancellation: &tokio_util::sync::CancellationToken,
        progress: &(dyn Fn(BatchProgress) + Send + Sync),
    ) -> BatchImageExecution {
        let batch_id = BatchId::new();
        let requested = u32::try_from(nodes.len()).unwrap_or(u32::MAX);
        let mut completed = 0_u32;
        let mut failed = 0_u32;
        let mut skipped = 0_u32;
        let mut cancelled = 0_u32;
        let mut outcomes = Vec::with_capacity(nodes.len());

        progress(snapshot(
            batch_id,
            requested,
            completed,
            failed,
            skipped,
            cancelled,
            BatchLifecycle::Running,
            None,
        ));
        for node in nodes {
            if cancellation.is_cancelled() {
                cancelled += 1;
                outcomes.push((
                    node.entity_id,
                    BatchImageItemOutcome::Skipped {
                        reason: BatchImageSkipReason::Cancelled,
                    },
                ));
                progress(snapshot(
                    batch_id,
                    requested,
                    completed,
                    failed,
                    skipped,
                    cancelled,
                    BatchLifecycle::Cancelling,
                    Some(node.entity_id),
                ));
                continue;
            }
            if !node.kind.is_previewable_image() {
                failed += 1;
                outcomes.push((
                    node.entity_id,
                    BatchImageItemOutcome::Failed {
                        reason: BatchImageFailureReason::DecodeFailed,
                    },
                ));
                progress(snapshot(
                    batch_id,
                    requested,
                    completed,
                    failed,
                    skipped,
                    cancelled,
                    BatchLifecycle::Running,
                    Some(node.entity_id),
                ));
                continue;
            }
            let Some(destination) = destination_path(root, node, settings) else {
                failed += 1;
                outcomes.push((
                    node.entity_id,
                    BatchImageItemOutcome::Failed {
                        reason: BatchImageFailureReason::EncodeFailed,
                    },
                ));
                progress(snapshot(
                    batch_id,
                    requested,
                    completed,
                    failed,
                    skipped,
                    cancelled,
                    BatchLifecycle::Running,
                    Some(node.entity_id),
                ));
                continue;
            };
            let destination = match settings.conflict {
                BatchImageConflictStrategy::Skip if self.io.path_exists(&destination) => {
                    skipped += 1;
                    outcomes.push((
                        node.entity_id,
                        BatchImageItemOutcome::Skipped {
                            reason: BatchImageSkipReason::Conflict,
                        },
                    ));
                    progress(snapshot(
                        batch_id,
                        requested,
                        completed,
                        failed,
                        skipped,
                        cancelled,
                        BatchLifecycle::Running,
                        Some(node.entity_id),
                    ));
                    continue;
                }
                BatchImageConflictStrategy::Suffix if self.io.path_exists(&destination) => {
                    match free_suffixed_path(&self.io, &destination) {
                        Some(free) => free,
                        None => {
                            skipped += 1;
                            outcomes.push((
                                node.entity_id,
                                BatchImageItemOutcome::Skipped {
                                    reason: BatchImageSkipReason::Conflict,
                                },
                            ));
                            progress(snapshot(
                                batch_id,
                                requested,
                                completed,
                                failed,
                                skipped,
                                cancelled,
                                BatchLifecycle::Running,
                                Some(node.entity_id),
                            ));
                            continue;
                        }
                    }
                }
                _ => destination,
            };

            match self.convert(root, node, &destination, settings).await {
                Ok(outcome) => {
                    completed += 1;
                    outcomes.push((node.entity_id, outcome));
                }
                Err(reason) => {
                    failed += 1;
                    outcomes.push((node.entity_id, BatchImageItemOutcome::Failed { reason }));
                }
            }
            progress(snapshot(
                batch_id,
                requested,
                completed,
                failed,
                skipped,
                cancelled,
                BatchLifecycle::Running,
                Some(node.entity_id),
            ));
        }
        progress(snapshot(
            batch_id,
            requested,
            completed,
            failed,
            skipped,
            cancelled,
            BatchLifecycle::Completed,
            None,
        ));
        BatchImageExecution { batch_id, outcomes }
    }

    async fn convert(
        &self,
        root: &Path,
        node: &viewer_domain::file::FileNode,
        destination: &Path,
        settings: &BatchImageSettings,
    ) -> Result<BatchImageItemOutcome, BatchImageFailureReason> {
        if let Some(parent) = destination.parent()
            && !self.io.path_exists(parent)
            && self.io.create_directory(parent).is_err()
        {
            return Err(BatchImageFailureReason::PlacementFailed);
        }
        let source = root.join(node.relative_path.as_str());
        let temp = destination.with_extension(format!(
            "viewer-batch-{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let encoded = self
            .io
            .encode(&source, settings, &temp)
            .await
            .map_err(|_| BatchImageFailureReason::EncodeFailed)?;
        let placed = if self.io.path_exists(destination)
            && matches!(settings.conflict, BatchImageConflictStrategy::Overwrite)
        {
            // Overwrite keeps the ADR 0002 shape: the original goes to the
            // system trash immediately before the prepared output renames in.
            if self.io.trash(destination).await.is_err() {
                let _ = std::fs::remove_file(&temp);
                return Err(BatchImageFailureReason::PlacementFailed);
            }
            self.io.rename(&temp, destination)
        } else {
            self.io.rename(&temp, destination)
        };
        if placed.is_err() {
            let _ = std::fs::remove_file(&temp);
            return Err(BatchImageFailureReason::PlacementFailed);
        }
        Ok(BatchImageItemOutcome::Converted {
            destination: display_relative(root, destination),
            width: encoded.0,
            height: encoded.1,
            size_bytes: encoded.2,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn snapshot(
    batch_id: BatchId,
    requested: u32,
    completed: u32,
    failed: u32,
    skipped: u32,
    cancelled: u32,
    lifecycle: BatchLifecycle,
    active_entity_id: Option<EntityId>,
) -> BatchProgress {
    BatchProgress {
        batch_id,
        lifecycle,
        requested,
        completed,
        failed,
        skipped,
        cancelled,
        active_entity_id,
    }
}

/// Absolute destination path for `node` under the chosen output policy.
fn destination_path(
    root: &Path,
    node: &viewer_domain::file::FileNode,
    settings: &BatchImageSettings,
) -> Option<PathBuf> {
    settings.validate().ok()?;
    let relative = Path::new(node.relative_path.as_str());
    let file_name = relative.file_name()?.to_str()?;
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    let output_name = format!("{}.{}", stem, settings.format.extension());
    let destination = match &settings.output {
        BatchImageOutput::Sibling { suffix } => {
            let output_name = format!("{stem}{suffix}.{ext}", ext = settings.format.extension());
            relative.with_file_name(output_name)
        }
        BatchImageOutput::Subfolder { name } => relative.with_file_name(name).join(output_name),
    };
    let destination = root.join(destination);
    // Defence in depth: the relative source path is already validated and the
    // output name is a single component, but never hand an escaping path to
    // the filesystem or the trash port.
    destination.starts_with(root).then_some(destination)
}

fn free_suffixed_path(io: &Arc<dyn BatchImageIoPort>, destination: &Path) -> Option<PathBuf> {
    let mut index = 2_u32;
    loop {
        let candidate = suffix_path(destination, index)?;
        if !io.path_exists(&candidate) {
            return Some(candidate);
        }
        index = index.checked_add(1)?;
    }
}

fn suffix_path(destination: &Path, index: u32) -> Option<PathBuf> {
    let file_name = destination.file_name()?.to_str()?;
    let (stem, extension) = file_name
        .rsplit_once('.')
        .map(|(stem, extension)| (stem, extension.to_owned()))
        .unwrap_or((file_name, String::new()));
    let name = if extension.is_empty() {
        format!("{stem}-{index}")
    } else {
        format!("{stem}-{index}.{extension}")
    };
    Some(destination.with_file_name(name))
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
