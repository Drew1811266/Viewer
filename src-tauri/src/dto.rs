use serde::{Deserialize, Serialize};
use viewer_application::scan::{ScanEvent, ScanTotals};
use viewer_application::{
    ActiveProject, ImageBackend, ProjectAccess,
    browse::{
        BrowserFile, ContentFolderCard, FolderReviewProgress, FolderTreeItem, FolderWorkspace,
        SelectionAgreement, SelectionInfo, SelectionTypeCounts,
    },
    file_commands::{ConflictResolution, FileCommandAction, FileCommandItem, FileCommandKind},
    metadata::{IndexProgress, MarkerChange},
};
use viewer_domain::{
    EntityId, RelativePath, TaskId,
    file::{FileNode, ImageMetadata, Marker, ReviewState},
    operation::{
        BatchItemResult, BatchLifecycle, BatchProgress, BatchResultPage, ConflictPolicy,
        RenamePreflight, RenameRuleSet, SequenceRule,
    },
    search::{MatchRange, SearchHit, SearchPage},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SequenceRuleRequestDto {
    pub start: u32,
    pub digits: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct RenameRulesRequestDto {
    pub find: String,
    pub replacement: String,
    pub prefix: String,
    pub suffix: String,
    pub sequence: Option<SequenceRuleRequestDto>,
}

impl From<RenameRulesRequestDto> for RenameRuleSet {
    fn from(rules: RenameRulesRequestDto) -> Self {
        Self {
            find: rules.find,
            replacement: rules.replacement,
            prefix: rules.prefix,
            suffix: rules.suffix,
            sequence: rules.sequence.map(|sequence| SequenceRule {
                start: sequence.start,
                digits: sequence.digits,
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PreviewRenameRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
    pub rules: RenameRulesRequestDto,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "snake_case")]
pub enum FileCommandActionRequestDto {
    Rename {
        #[serde(rename = "proposedName")]
        proposed_name: String,
        #[serde(rename = "editExtension")]
        edit_extension: bool,
    },
    Copy {
        #[serde(rename = "destinationFolderId")]
        destination_folder_id: String,
    },
    Move {
        #[serde(rename = "destinationFolderId")]
        destination_folder_id: String,
    },
    Trash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileCommandRequestError {
    InvalidEntityId,
    InvalidDestinationFolderId,
}

impl FileCommandActionRequestDto {
    pub fn into_domain(self) -> Result<FileCommandAction, FileCommandRequestError> {
        Ok(match self {
            Self::Rename {
                proposed_name,
                edit_extension,
            } => FileCommandAction::Rename {
                proposed_name,
                edit_extension,
            },
            Self::Copy {
                destination_folder_id,
            } => FileCommandAction::Copy {
                destination_folder: destination_folder_id
                    .parse()
                    .map_err(|_| FileCommandRequestError::InvalidDestinationFolderId)?,
            },
            Self::Move {
                destination_folder_id,
            } => FileCommandAction::Move {
                destination_folder: destination_folder_id
                    .parse()
                    .map_err(|_| FileCommandRequestError::InvalidDestinationFolderId)?,
            },
            Self::Trash => FileCommandAction::Trash,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FileCommandItemRequestDto {
    pub entity_id: String,
    pub action: FileCommandActionRequestDto,
}

impl FileCommandItemRequestDto {
    pub fn into_domain(self) -> Result<FileCommandItem, FileCommandRequestError> {
        Ok(FileCommandItem {
            entity_id: self
                .entity_id
                .parse()
                .map_err(|_| FileCommandRequestError::InvalidEntityId)?,
            action: self.action.into_domain()?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConflictResolutionRequestDto {
    pub entity_id: EntityId,
    pub policy: ConflictPolicy,
    pub apply_to_remaining: bool,
}

impl From<ConflictResolutionRequestDto> for ConflictResolution {
    fn from(resolution: ConflictResolutionRequestDto) -> Self {
        Self {
            entity_id: resolution.entity_id,
            policy: resolution.policy,
            apply_to_remaining: resolution.apply_to_remaining,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExecuteFileCommandRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub kind: FileCommandKind,
    pub items: Vec<FileCommandItemRequestDto>,
    pub conflicts: Vec<ConflictResolutionRequestDto>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreviewRowDto {
    pub entity_id: String,
    pub source_relative_path: String,
    pub destination_relative_path: Option<String>,
    pub proposed_name: String,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreviewDto {
    pub rows: Vec<RenamePreviewRowDto>,
    pub executable: bool,
}

impl From<RenamePreflight> for RenamePreviewDto {
    fn from(preview: RenamePreflight) -> Self {
        Self {
            rows: preview
                .rows
                .into_iter()
                .map(|row| RenamePreviewRowDto {
                    entity_id: row.entity_id.to_string(),
                    source_relative_path: row.source.as_str().to_owned(),
                    destination_relative_path: row.destination.map(|path| path.as_str().to_owned()),
                    proposed_name: row.proposed_name,
                    errors: row
                        .errors
                        .into_iter()
                        .map(|error| error.as_str().to_owned())
                        .collect(),
                })
                .collect(),
            executable: preview.executable,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStartedDto {
    pub batch_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperationStatusRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub batch_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperationResultsRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub batch_id: String,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CancelOperationRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub batch_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UndoLastOperationRequestDto {
    pub session_id: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgressDto {
    pub session_id: String,
    pub generation: u64,
    pub batch_id: String,
    pub lifecycle: BatchLifecycle,
    pub requested: u32,
    pub completed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub cancelled: u32,
    pub active_entity_id: Option<String>,
}

impl OperationProgressDto {
    pub fn from_progress(
        session_id: impl ToString,
        generation: u64,
        progress: BatchProgress,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            generation,
            batch_id: progress.batch_id.to_string(),
            lifecycle: progress.lifecycle,
            requested: progress.requested,
            completed: progress.completed,
            failed: progress.failed,
            skipped: progress.skipped,
            cancelled: progress.cancelled,
            active_entity_id: progress.active_entity_id.map(|id| id.to_string()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResultItemDto {
    pub entity_id: String,
    pub relative_path: String,
    pub status: viewer_domain::operation::BatchItemStatus,
    pub code: viewer_domain::operation::BatchResultCode,
}

impl From<BatchItemResult> for OperationResultItemDto {
    fn from(item: BatchItemResult) -> Self {
        Self {
            entity_id: item.entity_id.to_string(),
            relative_path: item.relative_path.as_str().to_owned(),
            status: item.status,
            code: item.code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationResultPageDto {
    pub total: u32,
    pub offset: u32,
    pub items: Vec<OperationResultItemDto>,
}

impl From<BatchResultPage> for OperationResultPageDto {
    fn from(page: BatchResultPage) -> Self {
        Self {
            total: page.total,
            offset: page.offset,
            items: page.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoReceiptDto {
    pub batch_id: String,
    pub kind: viewer_domain::operation::OperationKind,
    pub action_count: u32,
}

impl From<viewer_application::undo::UndoReceipt> for UndoReceiptDto {
    fn from(receipt: viewer_application::undo::UndoReceipt) -> Self {
        Self {
            batch_id: receipt.batch_id.to_string(),
            kind: receipt.kind,
            action_count: receipt.action_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectChangeReasonDto {
    ExternalChange,
    ExpectedViewerChange,
    Overflow,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectChangedDto {
    pub session_id: String,
    pub generation: u64,
    pub reason: ProjectChangeReasonDto,
    pub added: u64,
    pub removed: u64,
    pub modified: u64,
    pub moved: u64,
    pub marker_paths_moved: u64,
    pub failed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseBlockedDto {
    pub session_id: String,
    pub generation: u64,
    pub batch_id: String,
}

impl ProjectChangedDto {
    pub fn from_summary(
        session_id: impl ToString,
        generation: u64,
        summary: viewer_application::watcher::ReconcileSummary,
    ) -> Self {
        let reason = match summary.reason {
            viewer_application::watcher::ReconcileReason::ExternalChange => {
                ProjectChangeReasonDto::ExternalChange
            }
            viewer_application::watcher::ReconcileReason::ExpectedViewerChange(_) => {
                ProjectChangeReasonDto::ExpectedViewerChange
            }
            viewer_application::watcher::ReconcileReason::Overflow => {
                ProjectChangeReasonDto::Overflow
            }
        };
        Self {
            session_id: session_id.to_string(),
            generation,
            reason,
            added: summary.added,
            removed: summary.removed,
            modified: summary.modified,
            moved: summary.moved,
            marker_paths_moved: summary.marker_paths_moved,
            failed: summary.failed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAccessDto {
    ReadWrite,
    ReadOnly,
}

impl From<ProjectAccess> for ProjectAccessDto {
    fn from(access: ProjectAccess) -> Self {
        match access {
            ProjectAccess::ReadWrite => Self::ReadWrite,
            ProjectAccess::ReadOnly => Self::ReadOnly,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub project_id: String,
    pub session_id: String,
    pub generation: u64,
    pub display_name: String,
    pub access: ProjectAccessDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_report: Option<RecoveryReportDto>,
}

impl From<&ActiveProject> for ProjectSnapshot {
    fn from(project: &ActiveProject) -> Self {
        Self {
            project_id: project.project_id.to_string(),
            session_id: project.session_id.to_string(),
            generation: project.generation.get(),
            display_name: project.display_name.clone(),
            access: project.access.into(),
            recovery_report: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReportDto {
    pub recovered: u32,
    pub needs_user_review: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedNodeDto {
    pub entity_id: String,
    pub relative_path: String,
    pub kind: viewer_domain::file::FileKind,
    pub size: u64,
    pub modified_ns: String,
}

impl From<&FileNode> for ScannedNodeDto {
    fn from(node: &FileNode) -> Self {
        Self {
            entity_id: node.entity_id.to_string(),
            relative_path: node.relative_path.as_str().to_owned(),
            kind: node.kind,
            size: node.size,
            modified_ns: node.modified_ns.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTotalsDto {
    pub folders: u64,
    pub files: u64,
    pub failed: u64,
}

impl From<ScanTotals> for ScanTotalsDto {
    fn from(totals: ScanTotals) -> Self {
        Self {
            folders: totals.folders,
            files: totals.files,
            failed: totals.failed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScanEventDto {
    Folders {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        nodes: Vec<ScannedNodeDto>,
    },
    Files {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        nodes: Vec<ScannedNodeDto>,
    },
    FailedItem {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "relativePath")]
        relative_path: String,
        code: String,
    },
    Finished {
        #[serde(rename = "sessionId")]
        session_id: String,
        generation: u64,
        #[serde(rename = "taskId")]
        task_id: String,
        totals: ScanTotalsDto,
    },
}

impl ScanEventDto {
    pub fn from_event(active: &ActiveProject, task_id: TaskId, event: &ScanEvent) -> Self {
        let session_id = active.session_id.to_string();
        let generation = event.generation().get();
        let task_id = task_id.to_string();
        match event {
            ScanEvent::Folders { nodes, .. } => Self::Folders {
                session_id,
                generation,
                task_id,
                nodes: nodes.iter().map(ScannedNodeDto::from).collect(),
            },
            ScanEvent::Files { nodes, .. } => Self::Files {
                session_id,
                generation,
                task_id,
                nodes: nodes.iter().map(ScannedNodeDto::from).collect(),
            },
            ScanEvent::FailedItem {
                relative_display,
                code,
                ..
            } => Self::FailedItem {
                session_id,
                generation,
                task_id,
                relative_path: safe_relative_display(relative_display),
                code: code.clone(),
            },
            ScanEvent::Finished { totals, .. } => Self::Finished {
                session_id,
                generation,
                task_id,
                totals: (*totals).into(),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgressDto {
    pub session_id: String,
    pub generation: u64,
    pub images_total: u64,
    pub images_ready: u64,
    pub images_failed: u64,
    pub text_total: u64,
    pub text_ready: u64,
    pub text_skipped: u64,
    pub text_failed: u64,
    pub complete: bool,
}

impl IndexProgressDto {
    pub fn from_progress(active: &ActiveProject, progress: IndexProgress) -> Self {
        Self {
            session_id: active.session_id.to_string(),
            generation: active.generation.get(),
            images_total: progress.images_total,
            images_ready: progress.images_ready,
            images_failed: progress.images_failed,
            text_total: progress.text_total,
            text_ready: progress.text_ready,
            text_skipped: progress.text_skipped,
            text_failed: progress.text_failed,
            complete: progress.is_complete(),
        }
    }
}

fn safe_relative_display(value: &str) -> String {
    RelativePath::parse(value)
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|_| "unavailable".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerDto {
    pub review_state: Option<ReviewState>,
    pub favorite: bool,
}

impl From<Marker> for MarkerDto {
    fn from(marker: Marker) -> Self {
        Self {
            review_state: marker.review_state,
            favorite: marker.favorite,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadataDto {
    pub width: u32,
    pub height: u32,
}

impl From<ImageMetadata> for ImageMetadataDto {
    fn from(metadata: ImageMetadata) -> Self {
        Self {
            width: metadata.width,
            height: metadata.height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderReviewProgressDto {
    pub total: u64,
    pub keep: u64,
    pub pending: u64,
    pub reject: u64,
    pub unmarked: u64,
    pub favorite: u64,
}

impl From<FolderReviewProgress> for FolderReviewProgressDto {
    fn from(progress: FolderReviewProgress) -> Self {
        Self {
            total: progress.total,
            keep: progress.keep,
            pending: progress.pending,
            reject: progress.reject,
            unmarked: progress.unmarked,
            favorite: progress.favorite,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderTreeItemDto {
    pub entity_id: String,
    pub parent_entity_id: Option<String>,
    pub relative_path: String,
    pub name: String,
    pub marker: MarkerDto,
}

impl From<FolderTreeItem> for FolderTreeItemDto {
    fn from(folder: FolderTreeItem) -> Self {
        Self {
            entity_id: folder.entity_id.to_string(),
            parent_entity_id: folder.parent_entity_id.map(|id| id.to_string()),
            relative_path: folder.relative_path.as_str().to_owned(),
            name: folder.name,
            marker: folder.marker.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserFileDto {
    pub entity_id: String,
    pub relative_path: String,
    pub name: String,
    pub kind: viewer_domain::file::FileKind,
    pub size: u64,
    pub modified_ns: String,
    pub marker: MarkerDto,
    pub image_metadata: Option<ImageMetadataDto>,
    pub image_url: Option<String>,
}

impl From<BrowserFile> for BrowserFileDto {
    fn from(file: BrowserFile) -> Self {
        Self {
            entity_id: file.entity_id.to_string(),
            relative_path: file.relative_path.as_str().to_owned(),
            name: file.name,
            kind: file.kind,
            size: file.size,
            modified_ns: file.modified_ns.to_string(),
            marker: file.marker.into(),
            image_metadata: file.image_metadata.map(Into::into),
            image_url: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentFolderCardDto {
    pub entity_id: String,
    pub relative_path: String,
    pub name: String,
    pub marker: MarkerDto,
    pub image_count: u64,
    pub text_count: u64,
    pub review_progress: FolderReviewProgressDto,
    pub representative_images: Vec<BrowserFileDto>,
}

impl From<ContentFolderCard> for ContentFolderCardDto {
    fn from(folder: ContentFolderCard) -> Self {
        Self {
            entity_id: folder.entity_id.to_string(),
            relative_path: folder.relative_path.as_str().to_owned(),
            name: folder.name,
            marker: folder.marker.into(),
            image_count: folder.image_count,
            text_count: folder.text_count,
            review_progress: folder.review_progress.into(),
            representative_images: folder
                .representative_images
                .into_iter()
                .map(BrowserFileDto::from)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum SelectionAgreementDto<T> {
    NoneSelected,
    Common(T),
    Mixed,
}

impl<T, U> From<SelectionAgreement<T>> for SelectionAgreementDto<U>
where
    U: From<T>,
{
    fn from(agreement: SelectionAgreement<T>) -> Self {
        match agreement {
            SelectionAgreement::NoneSelected => Self::NoneSelected,
            SelectionAgreement::Common(value) => Self::Common(value.into()),
            SelectionAgreement::Mixed => Self::Mixed,
        }
    }
}

impl<T> SelectionAgreementDto<T> {
    pub const fn state_name(&self) -> &'static str {
        match self {
            Self::NoneSelected => "none_selected",
            Self::Common(_) => "common",
            Self::Mixed => "mixed",
        }
    }
}

impl SelectionAgreementDto<Option<ReviewState>> {
    pub const fn review_value(&self) -> Option<ReviewState> {
        match self {
            Self::Common(value) => *value,
            Self::NoneSelected | Self::Mixed => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionTypeCountsDto {
    pub folders: u64,
    pub images: u64,
    pub text_files: u64,
}

impl From<SelectionTypeCounts> for SelectionTypeCountsDto {
    fn from(types: SelectionTypeCounts) -> Self {
        Self {
            folders: types.folders,
            images: types.images,
            text_files: types.text_files,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionInfoDto {
    pub relative_paths: Vec<String>,
    pub total_size: u64,
    pub types: SelectionTypeCountsDto,
    pub common_review: SelectionAgreementDto<Option<ReviewState>>,
    pub common_favorite: SelectionAgreementDto<bool>,
}

impl From<SelectionInfo> for SelectionInfoDto {
    fn from(info: SelectionInfo) -> Self {
        Self {
            relative_paths: info
                .relative_paths
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            total_size: info.total_size,
            types: info.types.into(),
            common_review: match info.common_review {
                SelectionAgreement::NoneSelected => SelectionAgreementDto::NoneSelected,
                SelectionAgreement::Common(value) => SelectionAgreementDto::Common(value),
                SelectionAgreement::Mixed => SelectionAgreementDto::Mixed,
            },
            common_favorite: info.common_favorite.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchRangeDto {
    pub start: u32,
    pub end: u32,
}

impl From<MatchRange> for MatchRangeDto {
    fn from(range: MatchRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub entity_id: String,
    pub relative_path: String,
    pub name: String,
    pub kind: viewer_domain::file::FileKind,
    pub size: u64,
    pub modified_ns: String,
    pub marker: MarkerDto,
    pub image_metadata: Option<ImageMetadataDto>,
    pub matched_field: viewer_domain::search::MatchedField,
    pub score: i64,
    pub group_relative_path: Option<String>,
    pub match_ranges: Vec<MatchRangeDto>,
}

impl From<SearchHit> for SearchHitDto {
    fn from(hit: SearchHit) -> Self {
        let name = hit
            .node
            .relative_path
            .as_str()
            .rsplit_once('/')
            .map_or(hit.node.relative_path.as_str(), |(_, name)| name)
            .to_owned();
        Self {
            entity_id: hit.node.entity_id.to_string(),
            relative_path: hit.node.relative_path.as_str().to_owned(),
            name,
            kind: hit.node.kind,
            size: hit.node.size,
            modified_ns: hit.node.modified_ns.to_string(),
            marker: hit.marker.into(),
            image_metadata: hit.image_metadata.map(Into::into),
            matched_field: hit.matched_field,
            score: hit.score,
            group_relative_path: hit.group_relative_path.map(|path| path.as_str().to_owned()),
            match_ranges: hit.match_ranges.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPageDto {
    pub revision: u64,
    pub total: u32,
    pub hits: Vec<SearchHitDto>,
    pub progress: SearchProgressDto,
}

impl SearchPageDto {
    pub fn from_page(revision: u64, page: SearchPage) -> Self {
        Self {
            revision,
            total: page.total,
            hits: page.hits.into_iter().map(Into::into).collect(),
            progress: page.progress.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProgressDto {
    pub images_total: u64,
    pub images_ready: u64,
    pub images_failed: u64,
    pub text_total: u64,
    pub text_ready: u64,
    pub text_skipped: u64,
    pub text_failed: u64,
    pub complete: bool,
}

impl From<IndexProgress> for SearchProgressDto {
    fn from(progress: IndexProgress) -> Self {
        Self {
            images_total: progress.images_total,
            images_ready: progress.images_ready,
            images_failed: progress.images_failed,
            text_total: progress.text_total,
            text_ready: progress.text_ready,
            text_skipped: progress.text_skipped,
            text_failed: progress.text_failed,
            complete: progress.is_complete(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSnippetDto {
    pub revision: u64,
    pub entity_id: String,
    pub snippet: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerChangeDto {
    pub entity_id: String,
    pub relative_path: String,
    pub kind: viewer_domain::file::FileKind,
    pub marker: MarkerDto,
}

impl From<MarkerChange> for MarkerChangeDto {
    fn from(change: MarkerChange) -> Self {
        Self {
            entity_id: change.target.entity_id.to_string(),
            relative_path: change.target.relative_path.as_str().to_owned(),
            kind: change.target.kind,
            marker: change.marker.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkerBatchResultDto {
    pub changes: Vec<MarkerChangeDto>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchFiltersRequestDto {
    pub kinds: Vec<viewer_domain::file::FileKind>,
    pub review_states: Vec<ReviewState>,
    pub favorite_only: bool,
    pub unmarked_only: bool,
    pub orientations: Vec<viewer_domain::search::ImageOrientation>,
    pub width_min: Option<u32>,
    pub width_max: Option<u32>,
    pub height_min: Option<u32>,
    pub height_max: Option<u32>,
    pub size_min: Option<u64>,
    pub size_max: Option<u64>,
    pub modified_ns_min: Option<String>,
    pub modified_ns_max: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchSortRequestDto {
    pub key: viewer_domain::search::SearchSortKey,
    pub direction: viewer_domain::search::SortDirection,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchProjectRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub revision: u64,
    pub text: String,
    pub scope_folder_id: Option<String>,
    #[serde(default)]
    pub filters: SearchFiltersRequestDto,
    pub sort: SearchSortRequestDto,
    pub layout: viewer_domain::search::SearchLayout,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchTextSnippetRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub revision: u64,
    pub entity_id: String,
    pub query: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SetReviewStateRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
    pub review_state: Option<ReviewState>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToggleFavoriteRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SelectionInfoRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
}

impl From<Vec<MarkerChange>> for MarkerBatchResultDto {
    fn from(changes: Vec<MarkerChange>) -> Self {
        Self {
            changes: changes.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "workspace", rename_all = "snake_case")]
pub enum FolderWorkspaceDto {
    Category {
        folders: Vec<ContentFolderCardDto>,
    },
    Content {
        images: Vec<BrowserFileDto>,
        #[serde(rename = "textFiles")]
        text_files: Vec<BrowserFileDto>,
    },
    Empty,
}

impl From<FolderWorkspace> for FolderWorkspaceDto {
    fn from(workspace: FolderWorkspace) -> Self {
        match workspace {
            FolderWorkspace::Category { folders } => Self::Category {
                folders: folders
                    .into_iter()
                    .map(ContentFolderCardDto::from)
                    .collect(),
            },
            FolderWorkspace::Content { images, text_files } => Self::Content {
                images: images.into_iter().map(BrowserFileDto::from).collect(),
                text_files: text_files.into_iter().map(BrowserFileDto::from).collect(),
            },
            FolderWorkspace::Empty => Self::Empty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageBackendDto {
    QuickLook,
    ImageIo,
}

impl From<ImageBackend> for ImageBackendDto {
    fn from(backend: ImageBackend) -> Self {
        match backend {
            ImageBackend::QuickLook => Self::QuickLook,
            ImageBackend::ImageIo => Self::ImageIo,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRepresentationDto {
    pub cache_key: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub backend: ImageBackendDto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextPreviewFormatDto {
    PlainText,
    Markdown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreviewDto {
    pub entity_id: String,
    pub format: TextPreviewFormatDto,
    pub plain_text: Option<String>,
    pub markdown_html: Option<String>,
    pub encoding: viewer_application::TextEncoding,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRepresentationRequestDto {
    Thumbnail {
        #[serde(rename = "maxPixels")]
        max_pixels: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
    },
    FitPreview {
        #[serde(rename = "maxWidth")]
        max_width: u32,
        #[serde(rename = "maxHeight")]
        max_height: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
    },
    Original100Percent,
}

impl From<ImageRepresentationRequestDto> for viewer_domain::image::ImageRepresentationKind {
    fn from(request: ImageRepresentationRequestDto) -> Self {
        match request {
            ImageRepresentationRequestDto::Thumbnail {
                max_pixels,
                scale_milli,
            } => Self::Thumbnail {
                max_pixels,
                scale_milli,
            },
            ImageRepresentationRequestDto::FitPreview {
                max_width,
                max_height,
                scale_milli,
            } => Self::FitPreview {
                max_width,
                max_height,
                scale_milli,
            },
            ImageRepresentationRequestDto::Original100Percent => Self::Original100Percent,
        }
    }
}

#[cfg(test)]
mod tests {
    use viewer_application::browse::{SelectionAgreement, SelectionInfo, SelectionTypeCounts};
    use viewer_domain::RelativePath;

    #[test]
    fn failed_scan_item_display_never_exposes_an_absolute_or_reserved_path() {
        assert_eq!(super::safe_relative_display("catalog/id-1"), "catalog/id-1");
        for unsafe_value in ["/Users/example/secret", "../outside", ".viewer/index"] {
            assert_eq!(super::safe_relative_display(unsafe_value), "unavailable");
        }
    }

    #[test]
    fn selection_dto_keeps_unmarked_distinct_from_mixed_and_exposes_relative_paths_only() {
        let dto = super::SelectionInfoDto::from(SelectionInfo {
            relative_paths: vec![RelativePath::parse("catalog/id-2/image.jpg").unwrap()],
            total_size: 42,
            types: SelectionTypeCounts {
                folders: 0,
                images: 1,
                text_files: 0,
            },
            common_review: SelectionAgreement::Common(None),
            common_favorite: SelectionAgreement::Mixed,
        });

        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["relativePaths"][0], "catalog/id-2/image.jpg");
        assert_eq!(json["commonReview"]["state"], "common");
        assert!(json["commonReview"]["value"].is_null());
        assert_eq!(json["commonFavorite"]["state"], "mixed");
    }

    #[test]
    fn m2_command_requests_accept_only_the_frozen_camel_case_shape() {
        let request: super::SearchProjectRequestDto = serde_json::from_value(serde_json::json!({
            "sessionId": "00000000-0000-0000-0000-000000000001",
            "generation": 3,
            "revision": 7,
            "text": "鞋",
            "scopeFolderId": null,
            "filters": {
                "kinds": ["jpeg"],
                "reviewStates": ["keep"],
                "favoriteOnly": true,
                "modifiedNsMin": "123"
            },
            "sort": { "key": "natural_name", "direction": "ascending" },
            "layout": "flat",
            "offset": 0,
            "limit": 200
        }))
        .unwrap();
        assert_eq!(request.generation, 3);
        assert_eq!(request.revision, 7);
        assert_eq!(request.filters.modified_ns_min.as_deref(), Some("123"));

        let invalid =
            serde_json::from_value::<super::SetReviewStateRequestDto>(serde_json::json!({
                "session_id": "not-camel-case",
                "generation": 1,
                "entityIds": [],
                "reviewState": null
            }));
        assert!(invalid.is_err());
    }
}
