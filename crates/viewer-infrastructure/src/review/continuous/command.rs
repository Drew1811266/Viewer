//! Internal command digest, not the persisted state digest or an IPC serialization format.
//! Ordered JSON scalar/tuple values, separated by NUL (never raw within JSON), streamed into
//! BLAKE3 with a 64 MiB total cap. Explicit tags and lengths make every enum/sequence unambiguous.
use super::command_fields::*;
use std::io::Write;
use viewer_application::review_workspace::*;
use viewer_domain::review::{MAX_FEEDBACK_TEXT_BYTES, MAX_TARGETS_PER_FEEDBACK, continuous::*};

pub struct ContinuousReviewCommandCodec;
impl ReviewCommandCodecPort for ContinuousReviewCommandCodec {
    fn digest(&self, e: &ReviewCommandEnvelope) -> Result<[u8; 32], ReviewWorkspaceError> {
        if e.generated.targets.len() > MAX_TARGETS_PER_FEEDBACK
            || !(0..=9_007_199_254_740_991).contains(&e.generated.created_at_ms)
        {
            return Err(ContinuousReviewError::LimitExceeded.into());
        }
        let mut out = Encoder::new();
        out.field(&"viewer.review.command/1")?;
        out.field(&(
            e.context.project_id,
            e.context.stream_id,
            e.command_id,
            e.expected_snapshot_id,
        ))?;
        out.field(
            &e.context
                .production
                .as_ref()
                .map(|p| (p.task_id.as_str(), p.batch_id.as_str())),
        )?;
        out.field(&(
            e.generated.snapshot_id,
            e.generated.feedback_id,
            e.generated.text_revision_id,
            e.generated.archive_id,
            &e.generated.targets,
            e.generated.created_at_ms,
        ))?;
        match &e.command {
            ReviewWorkspaceCommand::SaveFeedback {
                feedback_id,
                text,
                targets,
            } => {
                if text.len() > MAX_FEEDBACK_TEXT_BYTES || targets.len() > MAX_TARGETS_PER_FEEDBACK
                {
                    return Err(ContinuousReviewError::LimitExceeded.into());
                }
                out.field(&("saveFeedback", feedback_id, text, targets.len()))?;
                for target in targets {
                    match target {
                        TargetEdit::Add {
                            asset_version_id,
                            anchor,
                        } => {
                            out.field(&("add", asset_version_id))?;
                            out.anchor(anchor)?;
                        }
                        TargetEdit::Redraw {
                            key,
                            asset_version_id,
                            anchor,
                        } => {
                            out.field(&("redraw", asset_version_id))?;
                            out.key(key)?;
                            out.anchor(anchor)?;
                        }
                    }
                }
            }
            ReviewWorkspaceCommand::Withdraw { targets } => {
                out.field(&"withdraw")?;
                out.keys(targets)?;
            }
            ReviewWorkspaceCommand::Archive(selection) => {
                out.field(&(
                    "archive",
                    selection.expected_snapshot_id,
                    selection.groups.len(),
                ))?;
                if selection.groups.len() > 10_000 {
                    return Err(ContinuousReviewError::LimitExceeded.into());
                }
                for group in &selection.groups {
                    match group.basis {
                        ArchiveBasis::Unknown => out.field(&"unknown")?,
                        ArchiveBasis::Known { snapshot, source } => {
                            out.field(&"known")?;
                            out.snapshot(&snapshot)?;
                            match source {
                                ArchiveBasisSource::UserSelected => out.field(&"userSelected")?,
                                ArchiveBasisSource::AgentDeclared { usage_id } => {
                                    out.field(&("agentDeclared", usage_id))?
                                }
                            }
                        }
                    }
                    out.keys(&group.targets)?;
                }
            }
            ReviewWorkspaceCommand::Restore {
                archive_id,
                decisions,
            } => {
                out.field(&("restore", archive_id, decisions.len()))?;
                out.limit_targets(decisions.len())?;
                for decision in decisions {
                    out.key(&decision.historical_key)?;
                    match &decision.choice {
                        RestoreChoice::PreserveCurrent => out.field(&"preserveCurrent")?,
                        RestoreChoice::UseHistorical => out.field(&"useHistorical")?,
                        RestoreChoice::ContinueAsNew {
                            feedback_id,
                            text_revision_id,
                            target_id,
                            target_revision_id,
                            target_asset_version_id,
                            confirmed_anchor,
                            created_at_ms,
                        } => {
                            out.field(&(
                                "continueAsNew",
                                feedback_id,
                                text_revision_id,
                                target_id,
                                target_revision_id,
                                target_asset_version_id,
                                created_at_ms,
                                confirmed_anchor.is_some(),
                            ))?;
                            if let Some(anchor) = confirmed_anchor {
                                out.anchor(anchor)?;
                            }
                        }
                    }
                }
            }
            ReviewWorkspaceCommand::ContinueHistorical {
                history_ref,
                bindings,
            } => {
                out.field(&("continueHistorical", bindings.len()))?;
                out.limit_targets(bindings.len())?;
                out.history(history_ref)?;
                for binding in bindings {
                    out.binding(binding)?;
                }
            }
            ReviewWorkspaceCommand::ConfirmSource(binding) => {
                out.field(&"confirmSource")?;
                out.binding(binding)?;
            }
            ReviewWorkspaceCommand::AdoptUsage { declaration_id } => {
                out.field(&("adoptUsage", declaration_id))?
            }
            ReviewWorkspaceCommand::Migrate(plan) => {
                out.field(&("migrate", plan.inspection_digest))?;
                match &plan.choice {
                    MigrationChoice::KeepHistoryOnly => out.field(&"keepHistoryOnly")?,
                    MigrationChoice::ContinueSelected {
                        legacy_targets,
                        bindings,
                    } => {
                        out.field(&("continueSelected", legacy_targets.len(), bindings.len()))?;
                        out.limit_targets(legacy_targets.len())?;
                        out.limit_targets(bindings.len())?;
                        for target in legacy_targets {
                            out.legacy(target)?;
                        }
                        for binding in bindings {
                            out.legacy(&binding.legacy_target)?;
                            out.field(&(binding.new_asset_version_id, binding.position_confirmed))?;
                            out.anchor(&binding.anchor)?;
                        }
                    }
                }
            }
        }
        out.flush()
            .map_err(|_| ReviewWorkspaceError::Repository(ReviewCommitError::Io))?;
        Ok(out.finish())
    }
}
