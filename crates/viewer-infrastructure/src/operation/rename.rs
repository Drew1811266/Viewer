use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use viewer_application::{ClockPort, FileMutationPort, FileOperationError, FileSnapshot};
use viewer_domain::{EntityId, OperationId, RelativePath, operation::OperationState};

use super::{
    copy::{hash_file_sync, sync_parent},
    journal::OperationJournal,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameMapping {
    pub operation_id: OperationId,
    pub entity_id: EntityId,
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenameStage {
    ToTemporary {
        operation_id: OperationId,
        entity_id: EntityId,
        source: PathBuf,
        temporary: PathBuf,
    },
    ToFinal {
        operation_id: OperationId,
        entity_id: EntityId,
        source: PathBuf,
        destination: PathBuf,
    },
}

impl RenameStage {
    pub const fn operation_id(&self) -> OperationId {
        match self {
            Self::ToTemporary { operation_id, .. } | Self::ToFinal { operation_id, .. } => {
                *operation_id
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenamePlan {
    pub stages: Vec<RenameStage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenameItemStatus {
    Completed,
    Failed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameItemResult {
    pub operation_id: OperationId,
    pub status: RenameItemStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameBatchResult {
    pub items: Vec<RenameItemResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RenamePlanError {
    #[error("rename path is outside the project: {0}")]
    OutsideProject(PathBuf),
    #[error("rename path targets reserved Viewer metadata: {0}")]
    ReservedPath(PathBuf),
    #[error("rename source is missing or is not a regular file: {0}")]
    SourceMissing(PathBuf),
    #[error("rename source is duplicated: {0}")]
    DuplicateSource(PathBuf),
    #[error("rename destination is duplicated: {0}")]
    DuplicateDestination(PathBuf),
    #[error("rename destination is occupied by an unrelated file: {0}")]
    DestinationOccupied(PathBuf),
    #[error("rename would not change the path: {0}")]
    NoOp(PathBuf),
    #[error("rename temporary path is already occupied: {0}")]
    TemporaryOccupied(PathBuf),
    #[error("inspect rename path {path}: {message}")]
    Io { path: PathBuf, message: String },
}

pub struct RenamePlanner;

impl RenamePlanner {
    pub fn plan(
        project_root: impl AsRef<Path>,
        case_sensitive: bool,
        mappings: &[RenameMapping],
    ) -> Result<RenamePlan, RenamePlanError> {
        let project_root =
            std::fs::canonicalize(project_root.as_ref()).map_err(|error| RenamePlanError::Io {
                path: project_root.as_ref().to_path_buf(),
                message: error.to_string(),
            })?;
        let mut validated = Vec::with_capacity(mappings.len());
        let mut source_keys = HashMap::new();
        let mut destination_keys = HashSet::new();

        for mapping in mappings {
            let source = resolve_source(&project_root, &mapping.source)?;
            let destination = resolve_destination(&project_root, &mapping.destination)?;
            if source == destination {
                return Err(RenamePlanError::NoOp(mapping.source.clone()));
            }
            let source_key = path_key(&source, case_sensitive);
            let destination_key = path_key(&destination, case_sensitive);
            if source_keys
                .insert(source_key.clone(), validated.len())
                .is_some()
            {
                return Err(RenamePlanError::DuplicateSource(mapping.source.clone()));
            }
            if !destination_keys.insert(destination_key.clone()) {
                return Err(RenamePlanError::DuplicateDestination(
                    mapping.destination.clone(),
                ));
            }
            validated.push(ValidatedMapping {
                mapping: mapping.clone(),
                source,
                destination,
                source_key,
                destination_key,
            });
        }

        for item in &validated {
            if item.destination.exists() && !source_keys.contains_key(&item.destination_key) {
                return Err(RenamePlanError::DestinationOccupied(
                    item.mapping.destination.clone(),
                ));
            }
        }

        let mut staged = cycle_members(&validated, &source_keys);
        if !case_sensitive {
            for (index, item) in validated.iter().enumerate() {
                if item.source_key == item.destination_key && item.source != item.destination {
                    staged.insert(index);
                }
            }
        }

        let mut temporary_paths = HashMap::new();
        let mut stages = Vec::new();
        for (index, item) in validated.iter().enumerate() {
            if !staged.contains(&index) {
                continue;
            }
            let temporary = item
                .source
                .parent()
                .ok_or_else(|| RenamePlanError::OutsideProject(item.mapping.source.clone()))?
                .join(format!(".viewer-rename-{}.part", item.mapping.operation_id));
            if temporary.exists() {
                return Err(RenamePlanError::TemporaryOccupied(temporary));
            }
            temporary_paths.insert(index, temporary.clone());
            stages.push(RenameStage::ToTemporary {
                operation_id: item.mapping.operation_id,
                entity_id: item.mapping.entity_id,
                source: item.source.clone(),
                temporary,
            });
        }

        let mut direct = (0..validated.len())
            .filter(|index| !staged.contains(index))
            .collect::<HashSet<_>>();
        while !direct.is_empty() {
            let next = direct
                .iter()
                .copied()
                .filter(|index| {
                    !direct.iter().any(|other| {
                        validated[*other].source_key == validated[*index].destination_key
                    })
                })
                .min();
            let Some(index) = next else {
                return Err(RenamePlanError::Io {
                    path: project_root,
                    message: "rename dependency cycle was not staged".into(),
                });
            };
            direct.remove(&index);
            let item = &validated[index];
            stages.push(RenameStage::ToFinal {
                operation_id: item.mapping.operation_id,
                entity_id: item.mapping.entity_id,
                source: item.source.clone(),
                destination: item.destination.clone(),
            });
        }

        for (index, item) in validated.iter().enumerate() {
            let Some(temporary) = temporary_paths.remove(&index) else {
                continue;
            };
            stages.push(RenameStage::ToFinal {
                operation_id: item.mapping.operation_id,
                entity_id: item.mapping.entity_id,
                source: temporary,
                destination: item.destination.clone(),
            });
        }

        Ok(RenamePlan { stages })
    }
}

pub struct RenameExecutor {
    project_root: PathBuf,
    journal: Arc<OperationJournal>,
    mutation: Arc<dyn FileMutationPort>,
    clock: Arc<dyn ClockPort>,
}

impl RenameExecutor {
    pub fn new(
        project_root: impl AsRef<Path>,
        journal: Arc<OperationJournal>,
        mutation: Arc<dyn FileMutationPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Result<Self, FileOperationError> {
        let project_root = std::fs::canonicalize(project_root.as_ref()).map_err(|error| {
            FileOperationError::io(
                "canonicalize rename project root",
                project_root.as_ref(),
                &error,
            )
        })?;
        Ok(Self {
            project_root,
            journal,
            mutation,
            clock,
        })
    }

    pub async fn execute(&self, plan: &RenamePlan) -> RenameBatchResult {
        let mut snapshots = HashMap::new();
        let mut statuses = HashMap::new();
        let mut operation_order = Vec::new();

        for stage in &plan.stages {
            let operation_id = stage.operation_id();
            if statuses.contains_key(&operation_id) {
                continue;
            }
            operation_order.push(operation_id);
            let source = match stage {
                RenameStage::ToTemporary { source, .. } | RenameStage::ToFinal { source, .. } => {
                    source
                }
            };
            match self.capture_identity(source).await {
                Ok(snapshot) => {
                    snapshots.insert(operation_id, snapshot);
                    statuses.insert(operation_id, RenameItemStatus::Completed);
                }
                Err(error) => {
                    statuses.insert(operation_id, RenameItemStatus::Failed(error.to_string()));
                }
            }
        }

        for stage in &plan.stages {
            let operation_id = stage.operation_id();
            if matches!(
                statuses.get(&operation_id),
                Some(RenameItemStatus::Failed(_))
            ) {
                continue;
            }
            let result = match stage {
                RenameStage::ToTemporary {
                    source, temporary, ..
                } => {
                    self.execute_temporary_stage(operation_id, source, temporary)
                        .await
                }
                RenameStage::ToFinal {
                    source,
                    destination,
                    ..
                } => {
                    let expected = snapshots
                        .get(&operation_id)
                        .expect("snapshot exists for an executable rename stage");
                    self.execute_final_stage(operation_id, source, destination, expected)
                        .await
                }
            };
            if let Err(error) = result {
                statuses.insert(operation_id, RenameItemStatus::Failed(error));
            }
        }

        RenameBatchResult {
            items: operation_order
                .into_iter()
                .map(|operation_id| RenameItemResult {
                    operation_id,
                    status: statuses
                        .remove(&operation_id)
                        .unwrap_or_else(|| RenameItemStatus::Failed("missing batch status".into())),
                })
                .collect(),
        }
    }

    async fn execute_temporary_stage(
        &self,
        operation_id: OperationId,
        source: &Path,
        temporary: &Path,
    ) -> Result<(), String> {
        let temporary_relative = self
            .relative_path(temporary)
            .map_err(|error| error.to_string())?;
        self.journal
            .register_temporary(
                operation_id,
                OperationState::Prepared,
                &temporary_relative,
                self.now(),
            )
            .map_err(|error| error.to_string())?;
        self.mutation
            .rename(source, temporary)
            .await
            .map_err(|error| error.to_string())?;
        sync_path_parent(temporary)
            .await
            .map_err(|error| error.to_string())?;
        self.journal
            .advance(
                operation_id,
                OperationState::Prepared,
                OperationState::Staged,
                self.now(),
            )
            .map_err(|error| error.to_string())
    }

    async fn execute_final_stage(
        &self,
        operation_id: OperationId,
        source: &Path,
        destination: &Path,
        expected: &RenameExpectedIdentity,
    ) -> Result<(), String> {
        let item = self
            .journal
            .item(operation_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("operation {operation_id} is absent from journal"))?;
        match item.state {
            OperationState::Prepared => self
                .journal
                .advance(
                    operation_id,
                    OperationState::Prepared,
                    OperationState::Staged,
                    self.now(),
                )
                .map_err(|error| error.to_string())?,
            OperationState::Staged => {}
            state => {
                return Err(format!(
                    "operation {operation_id} cannot rename from {state:?}"
                ));
            }
        }

        self.mutation
            .rename(source, destination)
            .await
            .map_err(|error| error.to_string())?;
        sync_path_parent(destination)
            .await
            .map_err(|error| error.to_string())?;
        let actual = self
            .mutation
            .snapshot(destination)
            .await
            .map_err(|error| error.to_string())?;
        let actual_fallback_hash = if expected.snapshot.file_id.is_none() {
            Some(
                hash_path(destination)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        if !same_identity(expected, &actual, actual_fallback_hash) {
            return Err(FileOperationError::IdentityChanged.to_string());
        }

        self.journal
            .advance(
                operation_id,
                OperationState::Staged,
                OperationState::FsApplied,
                self.now(),
            )
            .and_then(|()| {
                self.journal.advance(
                    operation_id,
                    OperationState::FsApplied,
                    OperationState::Verified,
                    self.now(),
                )
            })
            .and_then(|()| {
                self.journal.advance(
                    operation_id,
                    OperationState::Verified,
                    OperationState::MetaCommitted,
                    self.now(),
                )
            })
            .and_then(|()| {
                self.journal.advance(
                    operation_id,
                    OperationState::MetaCommitted,
                    OperationState::IndexSynced,
                    self.now(),
                )
            })
            .and_then(|()| {
                self.journal.advance(
                    operation_id,
                    OperationState::IndexSynced,
                    OperationState::Completed,
                    self.now(),
                )
            })
            .map_err(|error| error.to_string())
    }

    fn relative_path(&self, path: &Path) -> Result<RelativePath, FileOperationError> {
        let relative = path
            .strip_prefix(&self.project_root)
            .map_err(|_| FileOperationError::OutsideProject)?;
        let relative = relative
            .to_str()
            .ok_or(FileOperationError::OutsideProject)?;
        RelativePath::parse(relative).map_err(|_| FileOperationError::ReservedPath)
    }

    fn now(&self) -> i64 {
        self.clock.unix_millis()
    }

    async fn capture_identity(
        &self,
        path: &Path,
    ) -> Result<RenameExpectedIdentity, FileOperationError> {
        let snapshot = self.mutation.snapshot(path).await?;
        let fallback_hash = if snapshot.file_id.is_none() {
            Some(hash_path(path).await?)
        } else {
            None
        };
        Ok(RenameExpectedIdentity {
            snapshot,
            fallback_hash,
        })
    }
}

async fn sync_path_parent(path: &Path) -> Result<(), FileOperationError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    tokio::task::spawn_blocking(move || sync_parent(&path))
        .await
        .map_err(|error| FileOperationError::Io {
            action: "rename directory sync worker",
            path: error_path,
            message: error.to_string(),
        })?
}

struct RenameExpectedIdentity {
    snapshot: FileSnapshot,
    fallback_hash: Option<[u8; 32]>,
}

async fn hash_path(path: &Path) -> Result<[u8; 32], FileOperationError> {
    let path = path.to_path_buf();
    let error_path = path.clone();
    tokio::task::spawn_blocking(move || hash_file_sync(&path).map(|(_, hash)| hash))
        .await
        .map_err(|error| FileOperationError::Io {
            action: "rename fingerprint worker",
            path: error_path,
            message: error.to_string(),
        })?
}

fn same_identity(
    expected: &RenameExpectedIdentity,
    actual: &FileSnapshot,
    actual_fallback_hash: Option<[u8; 32]>,
) -> bool {
    expected.snapshot.volume_id == actual.volume_id
        && expected.snapshot.len == actual.len
        && match (expected.snapshot.file_id, actual.file_id) {
            (Some(expected), Some(actual)) => expected == actual,
            (None, None) => expected.fallback_hash == actual_fallback_hash,
            _ => false,
        }
}

struct ValidatedMapping {
    mapping: RenameMapping,
    source: PathBuf,
    destination: PathBuf,
    source_key: String,
    destination_key: String,
}

fn cycle_members(
    mappings: &[ValidatedMapping],
    source_keys: &HashMap<String, usize>,
) -> HashSet<usize> {
    let mut cycles = HashSet::new();
    for start in 0..mappings.len() {
        let mut positions = HashMap::new();
        let mut path = Vec::new();
        let mut current = start;
        loop {
            if let Some(position) = positions.get(&current).copied() {
                cycles.extend(path[position..].iter().copied());
                break;
            }
            positions.insert(current, path.len());
            path.push(current);
            let Some(next) = source_keys.get(&mappings[current].destination_key).copied() else {
                break;
            };
            current = next;
        }
    }
    cycles
}

fn resolve_source(root: &Path, relative: &Path) -> Result<PathBuf, RenamePlanError> {
    validate_relative(relative)?;
    let candidate = root.join(relative);
    let symlink_metadata = std::fs::symlink_metadata(&candidate)
        .map_err(|_| RenamePlanError::SourceMissing(relative.to_path_buf()))?;
    if symlink_metadata.file_type().is_symlink() || !symlink_metadata.is_file() {
        return Err(RenamePlanError::SourceMissing(relative.to_path_buf()));
    }
    let canonical = std::fs::canonicalize(&candidate)
        .map_err(|_| RenamePlanError::SourceMissing(relative.to_path_buf()))?;
    if !canonical.starts_with(root) {
        return Err(RenamePlanError::OutsideProject(relative.to_path_buf()));
    }
    Ok(canonical)
}

fn resolve_destination(root: &Path, relative: &Path) -> Result<PathBuf, RenamePlanError> {
    validate_relative(relative)?;
    let candidate = root.join(relative);
    let parent = candidate
        .parent()
        .ok_or_else(|| RenamePlanError::OutsideProject(relative.to_path_buf()))?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|error| RenamePlanError::Io {
        path: relative.to_path_buf(),
        message: error.to_string(),
    })?;
    if !canonical_parent.starts_with(root) {
        return Err(RenamePlanError::OutsideProject(relative.to_path_buf()));
    }
    Ok(canonical_parent.join(
        candidate
            .file_name()
            .ok_or_else(|| RenamePlanError::OutsideProject(relative.to_path_buf()))?,
    ))
}

fn validate_relative(path: &Path) -> Result<(), RenamePlanError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(RenamePlanError::OutsideProject(path.to_path_buf()));
    }
    if path.components().any(|component| {
        matches!(component, Component::Normal(value) if value.to_string_lossy().eq_ignore_ascii_case(".viewer"))
    }) {
        return Err(RenamePlanError::ReservedPath(path.to_path_buf()));
    }
    Ok(())
}

fn path_key(path: &Path, case_sensitive: bool) -> String {
    let key = path.to_string_lossy().into_owned();
    if case_sensitive {
        key
    } else {
        key.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::{RenameMapping, RenamePlanError, RenamePlanner, RenameStage};
    use std::path::PathBuf;
    use viewer_domain::{EntityId, OperationId};
    use viewer_test_support::project_fixture::ProjectFixture;

    fn mapping(source: &str, destination: &str) -> RenameMapping {
        RenameMapping {
            operation_id: OperationId::new(),
            entity_id: EntityId::new(),
            source: PathBuf::from(source),
            destination: PathBuf::from(destination),
        }
    }

    #[test]
    fn rename_planner_stages_every_member_of_a_cycle() {
        let project = ProjectFixture::new();
        project.create_file("A.jpg", b"A");
        project.create_file("B.jpg", b"B");
        let plan = RenamePlanner::plan(
            project.root(),
            true,
            &[mapping("A.jpg", "B.jpg"), mapping("B.jpg", "A.jpg")],
        )
        .unwrap();

        assert_eq!(plan.stages.len(), 4);
        assert!(matches!(plan.stages[0], RenameStage::ToTemporary { .. }));
        assert!(matches!(plan.stages[1], RenameStage::ToTemporary { .. }));
        assert!(matches!(plan.stages[2], RenameStage::ToFinal { .. }));
        assert!(matches!(plan.stages[3], RenameStage::ToFinal { .. }));
    }

    #[test]
    fn rename_planner_rejects_duplicate_destination() {
        let project = ProjectFixture::new();
        project.create_file("A.jpg", b"A");
        project.create_file("B.jpg", b"B");
        let result = RenamePlanner::plan(
            project.root(),
            true,
            &[mapping("A.jpg", "C.jpg"), mapping("B.jpg", "C.jpg")],
        );
        assert!(matches!(
            result,
            Err(RenamePlanError::DuplicateDestination(_))
        ));
    }

    #[test]
    fn rename_planner_stages_case_only_rename_on_insensitive_volume() {
        let project = ProjectFixture::new();
        project.create_file("A.jpg", b"A");
        let plan =
            RenamePlanner::plan(project.root(), false, &[mapping("A.jpg", "a.jpg")]).unwrap();
        assert_eq!(plan.stages.len(), 2);
        assert!(matches!(plan.stages[0], RenameStage::ToTemporary { .. }));
        assert!(matches!(plan.stages[1], RenameStage::ToFinal { .. }));
    }

    #[test]
    fn rename_planner_rejects_outside_project_path() {
        let project = ProjectFixture::new();
        project.create_file("A.jpg", b"A");
        let result = RenamePlanner::plan(project.root(), true, &[mapping("../A.jpg", "B.jpg")]);
        assert!(matches!(result, Err(RenamePlanError::OutsideProject(_))));
    }
}
