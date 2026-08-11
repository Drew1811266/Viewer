use async_trait::async_trait;
use std::{fs, path::PathBuf, sync::Arc};
use viewer_application::{
    BrowseIndexPort, ClockPort, CommitStage, MetadataCommitOutcome, OperationCommit,
    OperationCommitError, OperationCommitPort, VolumePort,
    metadata::{
        FileCopyProjection, FileMoveProjection, FilePathMove, OperationProjectionPort,
        PortableMetadataPort,
    },
};
use viewer_domain::{
    EntityId, RelativePath,
    file::FileNode,
    operation::{ConflictPolicy, OperationKind},
    search::Generation,
};
use viewer_infrastructure::operation::journal::{JournalItem, OperationJournal};

pub struct DesktopOperationCommitPort {
    root: PathBuf,
    generation: Generation,
    index: Arc<dyn BrowseIndexPort>,
    projection: Arc<dyn OperationProjectionPort>,
    metadata: Arc<dyn PortableMetadataPort>,
    volume: Arc<dyn VolumePort>,
    clock: Arc<dyn ClockPort>,
    journal: Arc<OperationJournal>,
    allow_missing_index: bool,
}

impl DesktopOperationCommitPort {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root: PathBuf,
        generation: Generation,
        index: Arc<dyn BrowseIndexPort>,
        projection: Arc<dyn OperationProjectionPort>,
        metadata: Arc<dyn PortableMetadataPort>,
        volume: Arc<dyn VolumePort>,
        clock: Arc<dyn ClockPort>,
        journal: Arc<OperationJournal>,
    ) -> Self {
        Self {
            root,
            generation,
            index,
            projection,
            metadata,
            volume,
            clock,
            journal,
            allow_missing_index: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_recovery(
        root: PathBuf,
        generation: Generation,
        index: Arc<dyn BrowseIndexPort>,
        projection: Arc<dyn OperationProjectionPort>,
        metadata: Arc<dyn PortableMetadataPort>,
        volume: Arc<dyn VolumePort>,
        clock: Arc<dyn ClockPort>,
        journal: Arc<OperationJournal>,
    ) -> Self {
        Self {
            root,
            generation,
            index,
            projection,
            metadata,
            volume,
            clock,
            journal,
            allow_missing_index: true,
        }
    }

    fn journal_batch(
        &self,
        operation_id: viewer_domain::OperationId,
        stage: CommitStage,
    ) -> Result<(JournalItem, Vec<JournalItem>), OperationCommitError> {
        let current = self
            .journal
            .item(operation_id)
            .map_err(|_| OperationCommitError::new(stage, "journal_unavailable"))?
            .ok_or_else(|| OperationCommitError::new(stage, "journal_item_missing"))?;
        let batch = self
            .journal
            .incomplete_items()
            .map_err(|_| OperationCommitError::new(stage, "journal_unavailable"))?
            .into_iter()
            .filter(|item| item.batch_id == current.batch_id)
            .collect();
        Ok((current, batch))
    }

    fn blocking_cycle_moves(
        &self,
        current: &JournalItem,
        batch: &[JournalItem],
        destination: &RelativePath,
    ) -> Result<Vec<FilePathMove>, OperationCommitError> {
        batch
            .iter()
            .filter(|item| item.operation_id != current.operation_id && item.source == *destination)
            .filter_map(|item| item.temporary.as_ref().map(|temporary| (item, temporary)))
            .map(|(item, temporary)| {
                self.has_marker(&item.source).map(|present| {
                    present.then(|| FilePathMove {
                        source: item.source.clone(),
                        destination: temporary.clone(),
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|moves| moves.into_iter().flatten().collect())
    }

    fn has_marker(&self, path: &RelativePath) -> Result<bool, OperationCommitError> {
        self.metadata
            .markers_for_paths(std::slice::from_ref(path))
            .map(|markers| markers.iter().any(|marker| marker.relative_path == *path))
            .map_err(|_| metadata_commit_error("metadata_unavailable"))
    }

    fn marker_source(
        &self,
        current: &JournalItem,
    ) -> Result<Option<RelativePath>, OperationCommitError> {
        let paths = current.temporary.as_ref().map_or_else(
            || vec![current.source.clone()],
            |temporary| vec![temporary.clone(), current.source.clone()],
        );
        let markers = self
            .metadata
            .markers_for_paths(&paths)
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(paths
            .into_iter()
            .find(|path| markers.iter().any(|marker| marker.relative_path == *path)))
    }

    fn replaced_destination_marker(
        &self,
        current: &JournalItem,
        batch: &[JournalItem],
        destination: &RelativePath,
    ) -> Option<RelativePath> {
        if current.conflict_policy != ConflictPolicy::Replace {
            return None;
        }
        let is_cycle_blocker = batch.iter().any(|item| {
            item.operation_id != current.operation_id
                && item.source == *destination
                && item.temporary.is_some()
        });
        if is_cycle_blocker {
            return None;
        }
        Some(destination.clone())
    }

    fn metadata_moves(
        &self,
        current: &JournalItem,
        batch: &[JournalItem],
        destination: &RelativePath,
    ) -> Result<Vec<FilePathMove>, OperationCommitError> {
        if current.kind == OperationKind::Copy {
            return Ok(Vec::new());
        }
        let mut moves = self.blocking_cycle_moves(current, batch, destination)?;
        if let Some(source) = self.marker_source(current)? {
            moves.push(FilePathMove {
                source,
                destination: destination.clone(),
            });
        }
        Ok(moves)
    }

    fn source(&self, entity_id: EntityId) -> Result<Option<FileNode>, OperationCommitError> {
        let node = self
            .index
            .node(entity_id)
            .map_err(|_| index_commit_error("index_unavailable"))?;
        match node {
            Some(node) => Ok(Some(node)),
            None if self.allow_missing_index => Ok(None),
            None => Err(index_commit_error("projection_stale")),
        }
    }

    fn case_sensitive(&self) -> Result<bool, OperationCommitError> {
        self.volume
            .is_case_sensitive(&self.root)
            .map_err(|_| index_commit_error("volume_unavailable"))
    }

    fn destination_node(
        &self,
        path: &RelativePath,
        kind: viewer_domain::file::FileKind,
    ) -> Result<FileNode, OperationCommitError> {
        let candidate = self.root.join(path.as_str());
        let metadata = fs::symlink_metadata(&candidate)
            .map_err(|_| index_commit_error("destination_unavailable"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(index_commit_error("destination_invalid"));
        }
        let canonical = fs::canonicalize(&candidate)
            .map_err(|_| index_commit_error("destination_unavailable"))?;
        if !canonical.starts_with(&self.root) {
            return Err(index_commit_error("destination_outside_project"));
        }
        Ok(FileNode {
            entity_id: entity_id_for_metadata(&metadata, path),
            relative_path: path.clone(),
            kind,
            size: metadata.len(),
            modified_ns: modified_ns(&metadata),
        })
    }
}

#[async_trait]
impl OperationCommitPort for DesktopOperationCommitPort {
    async fn commit_metadata(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        if commit.kind == OperationKind::Trash {
            return Ok(());
        }
        let destination = commit
            .destination
            .as_ref()
            .ok_or_else(|| metadata_commit_error("destination_missing"))?;
        let (current, batch) = self.journal_batch(commit.operation_id, CommitStage::Metadata)?;
        let case_sensitive = self
            .volume
            .is_case_sensitive(&self.root)
            .map_err(|_| metadata_commit_error("volume_unavailable"))?;
        let moves = self.metadata_moves(&current, &batch, destination)?;
        if moves.is_empty() {
            return Ok(());
        }
        self.metadata
            .move_paths(&moves, case_sensitive, self.clock.unix_millis())
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(())
    }

    async fn commit_metadata_barrier(
        &self,
        commit: &OperationCommit,
    ) -> Result<MetadataCommitOutcome, OperationCommitError> {
        if commit.kind == OperationKind::Trash {
            return Ok(MetadataCommitOutcome::CallerAdvancesJournal);
        }
        let destination = commit
            .destination
            .as_ref()
            .ok_or_else(|| metadata_commit_error("destination_missing"))?;
        let (current, batch) = self.journal_batch(commit.operation_id, CommitStage::Metadata)?;
        if current.conflict_policy != ConflictPolicy::Replace {
            self.commit_metadata(commit).await?;
            return Ok(MetadataCommitOutcome::CallerAdvancesJournal);
        }

        let case_sensitive = self
            .volume
            .is_case_sensitive(&self.root)
            .map_err(|_| metadata_commit_error("volume_unavailable"))?;
        let moves = self.metadata_moves(&current, &batch, destination)?;
        let replaced_destination = self.replaced_destination_marker(&current, &batch, destination);
        self.metadata
            .commit_replace(
                current.operation_id,
                replaced_destination.as_ref(),
                &moves,
                case_sensitive,
                self.clock.unix_millis(),
            )
            .map_err(|_| metadata_commit_error("metadata_unavailable"))?;
        Ok(MetadataCommitOutcome::JournalAdvanced)
    }

    async fn sync_index(&self, commit: &OperationCommit) -> Result<(), OperationCommitError> {
        let Some(source) = self.source(commit.entity_id)? else {
            return Ok(());
        };
        let case_sensitive = self.case_sensitive()?;
        match commit.kind {
            OperationKind::Rename | OperationKind::Move => {
                let destination_path = commit
                    .destination
                    .clone()
                    .ok_or_else(|| index_commit_error("destination_missing"))?;
                let destination = self.destination_node(&destination_path, source.kind)?;
                let (current, batch) =
                    self.journal_batch(commit.operation_id, CommitStage::Index)?;
                let mut moves = Vec::new();
                if let Some(blocker) = self
                    .index
                    .node_by_relative_path(&destination_path)
                    .map_err(|_| index_commit_error("index_unavailable"))?
                    .filter(|blocker| blocker.entity_id != source.entity_id)
                {
                    if let Some(temporary) = batch
                        .iter()
                        .find(|item| {
                            item.operation_id != current.operation_id
                                && item.entity_id == blocker.entity_id
                                && item.source == destination_path
                        })
                        .and_then(|item| item.temporary.clone())
                    {
                        let temporary_destination = FileNode {
                            relative_path: temporary,
                            ..blocker.clone()
                        };
                        moves.push(FileMoveProjection {
                            source: blocker,
                            destination: temporary_destination,
                        });
                    } else {
                        self.projection
                            .apply_trash(&[blocker])
                            .map_err(|_| index_commit_error("projection_stale"))?;
                    }
                }
                moves.push(FileMoveProjection {
                    source,
                    destination,
                });
                self.projection
                    .apply_move(&moves, case_sensitive)
                    .map_err(|_| index_commit_error("projection_stale"))
            }
            OperationKind::Copy => {
                let destination_path = commit
                    .destination
                    .as_ref()
                    .ok_or_else(|| index_commit_error("destination_missing"))?;
                let destination = self.destination_node(destination_path, source.kind)?;
                if let Some(replaced) = self
                    .index
                    .node_by_relative_path(destination_path)
                    .map_err(|_| index_commit_error("index_unavailable"))?
                    .filter(|replaced| replaced.entity_id != destination.entity_id)
                {
                    self.projection
                        .apply_trash(&[replaced])
                        .map_err(|_| index_commit_error("projection_stale"))?;
                }
                self.projection
                    .apply_copy(
                        &[FileCopyProjection {
                            source,
                            destination,
                        }],
                        case_sensitive,
                        self.generation,
                    )
                    .map_err(|_| index_commit_error("projection_stale"))
            }
            OperationKind::Trash => self
                .projection
                .apply_trash(&[source])
                .map_err(|_| index_commit_error("projection_stale")),
            OperationKind::SetReviewState | OperationKind::SetFavorite => {
                Err(index_commit_error("operation_kind_invalid"))
            }
        }
    }
}

fn metadata_commit_error(code: &str) -> OperationCommitError {
    OperationCommitError::new(CommitStage::Metadata, code)
}

fn index_commit_error(code: &str) -> OperationCommitError {
    OperationCommitError::new(CommitStage::Index, code)
}

#[cfg(unix)]
fn entity_id_for_metadata(metadata: &fs::Metadata, _path: &RelativePath) -> EntityId {
    use std::os::unix::fs::MetadataExt;
    EntityId::from_u128((u128::from(metadata.dev()) << 64) | u128::from(metadata.ino()))
}

#[cfg(not(unix))]
fn entity_id_for_metadata(metadata: &fs::Metadata, path: &RelativePath) -> EntityId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.as_str().hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    EntityId::from_u128(u128::from(hasher.finish()))
}

#[cfg(unix)]
fn modified_ns(metadata: &fs::Metadata) -> i128 {
    use std::os::unix::fs::MetadataExt;
    i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec())
}

#[cfg(not(unix))]
fn modified_ns(metadata: &fs::Metadata) -> i128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos() as i128)
}
