use super::*;
use viewer_domain::review::continuous::{ReviewAvailability, VersionedTarget};

/// Preserve the submitted editor input, not a later state or a regenerated command.
/// Redraw carries its original asset identity even when another writer removed that target.
pub(super) fn draft(
    envelope: &ReviewCommandEnvelope,
    failure: ReviewRecoveryFailure,
) -> Result<RecoveryDraft, ReviewWorkspaceError> {
    let mut input = RecoveryEditorInput {
        migration: match &envelope.command {
            ReviewWorkspaceCommand::Migrate(plan) => Some(plan.clone()),
            _ => None,
        },
        selections: vec![],
        text: String::new(),
        feedback_id: None,
        targets: vec![],
        history_ref: None,
    };
    if let ReviewWorkspaceCommand::SaveFeedback {
        feedback_id,
        text,
        targets,
    } = &envelope.command
    {
        if targets.len() != envelope.generated.targets.len() {
            return Err(
                viewer_domain::review::continuous::ContinuousReviewError::InvalidData.into(),
            );
        }
        input.text = text.clone();
        input.feedback_id = Some(feedback_id.unwrap_or(envelope.generated.feedback_id));
        for (edit, &(new_id, revision_id)) in targets.iter().zip(&envelope.generated.targets) {
            let (id, asset_version_id, anchor) = match edit {
                TargetEdit::Add {
                    asset_version_id,
                    anchor,
                } => (new_id, *asset_version_id, anchor),
                TargetEdit::Redraw {
                    key,
                    asset_version_id,
                    anchor,
                } => (key.target_id, *asset_version_id, anchor),
            };
            input.targets.push(VersionedTarget {
                id,
                revision_id,
                asset_version_id,
                anchor: anchor.clone(),
                availability: ReviewAvailability::Ready,
            });
        }
    }
    super::recovery_selection::preserve(envelope, &mut input)?;
    Ok(RecoveryDraft {
        stream_id: envelope.context.stream_id,
        command_id: envelope.command_id,
        expected_snapshot_id: envelope.expected_snapshot_id,
        payload_digest: envelope.payload_digest,
        editor_input: input,
        failure,
    })
}

pub(super) fn failure(error: &ReviewWorkspaceError) -> ReviewRecoveryFailure {
    match error {
        ReviewWorkspaceError::Cancelled
        | ReviewWorkspaceError::Asset(crate::ReviewAssetError::Cancelled)
        | ReviewWorkspaceError::Evidence(crate::ReviewArtifactError::Cancelled) => {
            ReviewRecoveryFailure::Cancelled
        }
        ReviewWorkspaceError::Repository(ReviewCommitError::OutcomeUnknown) => {
            ReviewRecoveryFailure::CommitUnknown
        }
        ReviewWorkspaceError::Repository(ReviewCommitError::StaleSnapshot) => {
            ReviewRecoveryFailure::StaleSnapshot
        }
        ReviewWorkspaceError::Evidence(crate::ReviewArtifactError::SourceChanged)
        | ReviewWorkspaceError::Usage(UsageImportError::SourceChanged)
        | ReviewWorkspaceError::Asset(crate::ReviewAssetError::SourceChanged) => {
            ReviewRecoveryFailure::SourceChanged
        }
        ReviewWorkspaceError::Evidence(_) => ReviewRecoveryFailure::RenderFailed,
        _ => ReviewRecoveryFailure::WriteFailed,
    }
}
