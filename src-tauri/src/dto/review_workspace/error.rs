use super::ReviewWire;
use serde::Serialize;
use viewer_application::{ReviewArtifactError, ReviewAssetError, review_workspace::*};
use viewer_domain::review::continuous::ContinuousReviewError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewWorkspaceErrorCode {
    InvalidData,
    StaleSession,
    Cancelled,
    Busy,
    Internal,
    MigrationRequired,
    UnsupportedProtocol,
    StaleSnapshot,
    CommandConflict,
    ReadOnly,
    LeaseBusy,
    Integrity,
    LimitExceeded,
    LookupUnavailable,
    EvidenceAbsent,
    AmbiguousEvidence,
    Io,
    OutcomeUnknown,
    AssetUnavailable,
    SourceChanged,
    UnsafeSource,
    RenderFailed,
    UsageInvalid,
    NoChanges,
    CapabilityUnavailable,
    WrongContext,
    PreviewRequired,
    NeedsConfirmation,
    SelectionConflict,
    CommittedViewUnavailable,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewWorkspaceErrorDto {
    pub code: ReviewWorkspaceErrorCode,
    pub message: String,
    pub retryable: bool,
    pub committed_receipt: Option<Box<ReviewWire<ReviewCommitReceipt>>>,
}
impl ReviewWorkspaceErrorDto {
    pub fn new(code: ReviewWorkspaceErrorCode) -> Self {
        use ReviewWorkspaceErrorCode::*;
        let message = match code {
            StaleSession => "评审请求所属的项目会话已失效。",
            Cancelled => "本次评审操作已取消；请保留尚未提交的输入。",
            CommittedViewUnavailable => "评审已提交，但当前视图未能刷新；请保留提交回执。",
            OutcomeUnknown => "提交结果暂不能确认；请用原命令查询或重试。",
            ReadOnly => "当前项目为只读，不能修改评审。",
            StaleSnapshot => "评审内容已变化，请刷新并重新确认。",
            PreviewRequired => "请先预览并绑定当前素材版本。",
            SourceChanged => "素材已变化，请重新核对；不会自动替换意见绑定。",
            MigrationRequired => "旧评审需要显式迁移，当前未改动旧记录。",
            _ => "评审操作未完成，请根据错误类型检查后重试。",
        };
        Self {
            code,
            message: message.into(),
            retryable: matches!(
                code,
                Cancelled | Busy | LeaseBusy | Io | OutcomeUnknown | CommittedViewUnavailable
            ),
            committed_receipt: None,
        }
    }
    pub fn stale_with_receipt(receipt: Option<ReviewCommitReceipt>) -> Self {
        Self {
            committed_receipt: receipt.map(|value| Box::new(value.into())),
            ..Self::new(ReviewWorkspaceErrorCode::StaleSession)
        }
    }
}
impl From<ReviewWorkspaceError> for ReviewWorkspaceErrorDto {
    fn from(error: ReviewWorkspaceError) -> Self {
        use ReviewWorkspaceErrorCode as Code;
        let code = match error {
            ReviewWorkspaceError::CommittedViewUnavailable(receipt) => {
                return Self {
                    committed_receipt: Some(Box::new(receipt.into())),
                    ..Self::new(Code::CommittedViewUnavailable)
                };
            }
            // The async authoring route remains gated until its dedicated receipt DTO lands.
            ReviewWorkspaceError::CommittedAuthoringPatchUnavailable(_) => {
                Code::CommittedViewUnavailable
            }
            ReviewWorkspaceError::Repository(e) => match e {
                ReviewCommitError::MigrationRequired => Code::MigrationRequired,
                ReviewCommitError::UnsupportedProtocol => Code::UnsupportedProtocol,
                ReviewCommitError::StaleSnapshot => Code::StaleSnapshot,
                ReviewCommitError::CommandConflict => Code::CommandConflict,
                ReviewCommitError::ReadOnly => Code::ReadOnly,
                ReviewCommitError::LeaseBusy => Code::LeaseBusy,
                ReviewCommitError::Integrity => Code::Integrity,
                ReviewCommitError::LimitExceeded => Code::LimitExceeded,
                ReviewCommitError::LookupUnavailable => Code::LookupUnavailable,
                ReviewCommitError::EvidenceAbsent => Code::EvidenceAbsent,
                ReviewCommitError::AmbiguousEvidence => Code::AmbiguousEvidence,
                ReviewCommitError::Io => Code::Io,
                ReviewCommitError::OutcomeUnknown => Code::OutcomeUnknown,
            },
            ReviewWorkspaceError::Domain(e) => match e {
                ContinuousReviewError::StaleSnapshot => Code::StaleSnapshot,
                ContinuousReviewError::LimitExceeded => Code::LimitExceeded,
                ContinuousReviewError::NeedsConfirmation => Code::NeedsConfirmation,
                ContinuousReviewError::SelectionConflict => Code::SelectionConflict,
                _ => Code::InvalidData,
            },
            ReviewWorkspaceError::Asset(ReviewAssetError::Cancelled)
            | ReviewWorkspaceError::Evidence(ReviewArtifactError::Cancelled)
            | ReviewWorkspaceError::Cancelled => Code::Cancelled,
            ReviewWorkspaceError::Asset(ReviewAssetError::SourceChanged)
            | ReviewWorkspaceError::Evidence(ReviewArtifactError::SourceChanged)
            | ReviewWorkspaceError::Usage(UsageImportError::SourceChanged) => Code::SourceChanged,
            ReviewWorkspaceError::Asset(ReviewAssetError::LimitExceeded)
            | ReviewWorkspaceError::Evidence(ReviewArtifactError::LimitExceeded)
            | ReviewWorkspaceError::Usage(UsageImportError::LimitExceeded) => Code::LimitExceeded,
            ReviewWorkspaceError::Asset(ReviewAssetError::UnsafeSource)
            | ReviewWorkspaceError::Evidence(ReviewArtifactError::UnsafeSource)
            | ReviewWorkspaceError::Usage(UsageImportError::UnsafePath) => Code::UnsafeSource,
            ReviewWorkspaceError::Asset(_) => Code::AssetUnavailable,
            ReviewWorkspaceError::Evidence(_) => Code::RenderFailed,
            ReviewWorkspaceError::Usage(_) => Code::UsageInvalid,
            ReviewWorkspaceError::NoChanges => Code::NoChanges,
            ReviewWorkspaceError::CapabilityUnavailable => Code::CapabilityUnavailable,
            ReviewWorkspaceError::WrongContext => Code::WrongContext,
            ReviewWorkspaceError::PreviewRequired => Code::PreviewRequired,
        };
        Self::new(code)
    }
}
