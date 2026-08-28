use super::*;
use viewer_domain::review::{FeedbackAnchor, continuous::*};

fn confirmation(value: &SourceBindingConfirmation) -> RecoveryTargetConfirmation {
    match value {
        SourceBindingConfirmation::UserConfirmed => RecoveryTargetConfirmation::UserConfirmed,
        SourceBindingConfirmation::ProducerVerifiedAndPositionConfirmed { usage_id } => {
            RecoveryTargetConfirmation::ProducerVerified {
                usage_id: *usage_id,
            }
        }
    }
}
fn add(
    input: &mut RecoveryEditorInput,
    selection: RecoveryTargetSelection,
    anchor: Option<&FeedbackAnchor>,
) {
    if let Some(anchor) = anchor {
        input.targets.push(VersionedTarget {
            id: selection.target_id,
            revision_id: selection.target_revision_id,
            asset_version_id: selection.asset_version_id,
            anchor: anchor.clone(),
            availability: ReviewAvailability::Ready,
        });
    }
    input.selections.push(selection);
}

pub(super) fn preserve(
    e: &ReviewCommandEnvelope,
    input: &mut RecoveryEditorInput,
) -> Result<(), ReviewWorkspaceError> {
    let ids = &e.generated;
    match &e.command {
        ReviewWorkspaceCommand::ContinueHistorical {
            history_ref,
            bindings,
        } => {
            if bindings.len() != ids.targets.len() {
                return Err(ContinuousReviewError::InvalidData.into());
            }
            input.history_ref = Some(history_ref.clone());
            input.feedback_id = Some(ids.feedback_id);
            for (b, &(target_id, target_revision_id)) in bindings.iter().zip(&ids.targets) {
                add(
                    input,
                    RecoveryTargetSelection {
                        origin: RecoveryTargetOrigin::Snapshot { key: b.target_key },
                        feedback_id: ids.feedback_id,
                        text_revision_id: ids.text_revision_id,
                        target_id,
                        target_revision_id,
                        asset_version_id: b.new_asset_version_id,
                        confirmation: confirmation(&b.confirmation),
                    },
                    Some(&b.anchor),
                );
            }
        }
        ReviewWorkspaceCommand::ContinueLegacy {
            history_ref,
            bindings,
        } => {
            if bindings.len() != ids.targets.len() {
                return Err(ContinuousReviewError::InvalidData.into());
            }
            input.history_ref = Some(history_ref.clone());
            input.feedback_id = Some(ids.feedback_id);
            for (b, &(target_id, target_revision_id)) in bindings.iter().zip(&ids.targets) {
                let k = b.legacy_target;
                add(
                    input,
                    RecoveryTargetSelection {
                        origin: RecoveryTargetOrigin::Legacy {
                            round_id: k.round_id,
                            feedback_id: k.feedback_id,
                            target_index: k.target_index,
                        },
                        feedback_id: ids.feedback_id,
                        text_revision_id: ids.text_revision_id,
                        target_id,
                        target_revision_id,
                        asset_version_id: b.new_asset_version_id,
                        confirmation: if b.position_confirmed {
                            RecoveryTargetConfirmation::UserConfirmed
                        } else {
                            RecoveryTargetConfirmation::Unconfirmed
                        },
                    },
                    Some(&b.anchor),
                );
            }
        }
        ReviewWorkspaceCommand::ConfirmSource(b) => {
            let revision = ids
                .targets
                .first()
                .ok_or(ContinuousReviewError::InvalidData)?
                .1;
            input.feedback_id = Some(b.target_key.feedback_id);
            add(
                input,
                RecoveryTargetSelection {
                    origin: RecoveryTargetOrigin::Current { key: b.target_key },
                    feedback_id: b.target_key.feedback_id,
                    text_revision_id: b.target_key.text_revision_id,
                    target_id: b.target_key.target_id,
                    target_revision_id: revision,
                    asset_version_id: b.new_asset_version_id,
                    confirmation: confirmation(&b.confirmation),
                },
                Some(&b.anchor),
            );
        }
        ReviewWorkspaceCommand::ConfirmApplicability {
            key,
            asset_version_id,
            anchor,
        } => {
            let revision = ids
                .targets
                .first()
                .ok_or(ContinuousReviewError::InvalidData)?
                .1;
            input.feedback_id = Some(key.feedback_id);
            add(
                input,
                RecoveryTargetSelection {
                    origin: RecoveryTargetOrigin::Current { key: *key },
                    feedback_id: key.feedback_id,
                    text_revision_id: key.text_revision_id,
                    target_id: key.target_id,
                    target_revision_id: revision,
                    asset_version_id: *asset_version_id,
                    confirmation: RecoveryTargetConfirmation::UserConfirmed,
                },
                Some(anchor),
            );
        }
        ReviewWorkspaceCommand::Restore {
            archive_id,
            decisions,
        } => {
            for d in decisions {
                if let RestoreChoice::ContinueAsNew {
                    feedback_id,
                    text_revision_id,
                    target_id,
                    target_revision_id,
                    target_asset_version_id,
                    confirmed_anchor,
                    ..
                } = &d.choice
                {
                    add(
                        input,
                        RecoveryTargetSelection {
                            origin: RecoveryTargetOrigin::Archive {
                                archive_id: *archive_id,
                                key: d.historical_key,
                            },
                            feedback_id: *feedback_id,
                            text_revision_id: *text_revision_id,
                            target_id: *target_id,
                            target_revision_id: *target_revision_id,
                            asset_version_id: *target_asset_version_id,
                            confirmation: if confirmed_anchor.is_some() {
                                RecoveryTargetConfirmation::UserConfirmed
                            } else {
                                RecoveryTargetConfirmation::Unconfirmed
                            },
                        },
                        confirmed_anchor.as_ref(),
                    );
                }
            }
        }
        _ => {}
    }
    Ok(())
}
