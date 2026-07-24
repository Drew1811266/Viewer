use serde::{Deserialize, Serialize};
use viewer_application::file_commands::{
    ConflictResolution, FileCommandAction, FileCommandItem, FileCommandKind, FileCommandPreflight,
    FileCommandPreflightState,
};
use viewer_domain::{
    EntityId,
    operation::{
        BatchItemResult, BatchLifecycle, BatchProgress, BatchResultPage, ConflictPolicy,
        RenamePreflight, RenameRuleSet, SequenceRule,
    },
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

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PreflightFileCommandRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub kind: FileCommandKind,
    pub items: Vec<FileCommandItemRequestDto>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BeginFinderDragRequestDto {
    pub session_id: String,
    pub generation: u64,
    pub entity_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinderDragReceiptDto {
    pub file_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCommandPreflightStateDto {
    Ready,
    Conflict,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCommandPreflightRowDto {
    pub entity_id: String,
    pub relative_path: String,
    pub state: FileCommandPreflightStateDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<viewer_domain::operation::BatchResultCode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCommandPreflightDto {
    pub rows: Vec<FileCommandPreflightRowDto>,
    pub executable: bool,
}

impl From<FileCommandPreflight> for FileCommandPreflightDto {
    fn from(preflight: FileCommandPreflight) -> Self {
        Self {
            rows: preflight
                .rows()
                .iter()
                .map(|row| {
                    let (state, code) = match row.state {
                        FileCommandPreflightState::Ready => {
                            (FileCommandPreflightStateDto::Ready, None)
                        }
                        FileCommandPreflightState::Conflict => {
                            (FileCommandPreflightStateDto::Conflict, None)
                        }
                        FileCommandPreflightState::Blocked(code) => {
                            (FileCommandPreflightStateDto::Blocked, Some(code))
                        }
                    };
                    FileCommandPreflightRowDto {
                        entity_id: row.entity_id.to_string(),
                        relative_path: row.relative_path.as_str().to_owned(),
                        state,
                        code,
                    }
                })
                .collect(),
            executable: preflight.is_executable(),
        }
    }
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
