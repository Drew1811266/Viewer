use super::*;
use std::collections::HashMap;
use viewer_domain::{AssetVersionId, review::continuous::*};

pub(super) struct Transition {
    pub next: ContinuousReviewState,
    pub changes: Vec<ReviewChange>,
    pub archives: Vec<ArchiveCheckpoint>,
}

pub(super) fn prepare(
    repository: &dyn ContinuousReviewRepositoryPort,
    current: Option<&StoredContinuousSnapshot>,
    empty: &ContinuousReviewState,
    envelope: &ReviewCommandEnvelope,
    assets: &HashMap<AssetVersionId, crate::PreparedReviewAsset>,
    usages: &[super::usage::VerifiedUsage],
) -> Result<Transition, ReviewWorkspaceError> {
    let before = current.map_or(empty, |s| &s.state);
    let snapshot = envelope.generated.snapshot_id;
    let mut archives = vec![];
    let mut explicit_changes = None;
    let mut kind = ReviewChangeKind::Edited;
    let next = match &envelope.command {
        ReviewWorkspaceCommand::SaveFeedback { .. } => {
            super::editing::save(before, envelope, assets)?
        }
        ReviewWorkspaceCommand::Withdraw { targets } => {
            for key in targets {
                if before.target_key(key.target_id) != Some(*key) {
                    return Err(ContinuousReviewError::StaleSnapshot.into());
                }
            }
            withdraw_targets(
                before,
                &targets.iter().map(|k| k.target_id).collect::<Vec<_>>(),
                snapshot,
            )?
        }
        ReviewWorkspaceCommand::Archive(selection) => {
            let current = current.ok_or(ContinuousReviewError::MissingReference)?;
            let (_, bases, coverage) =
                super::archiving::archive_plan(repository, current, selection)?;
            let (next, plan) = apply_archive(before, &bases, selection, &coverage, snapshot)?;
            if plan.is_noop() {
                return Err(ReviewWorkspaceError::NoChanges);
            }
            let id = envelope.generated.archive_id;
            archives.push(ArchiveCheckpoint::from_plan(
                before,
                current.reference,
                &plan,
                id,
                envelope.generated.created_at_ms,
            )?);
            explicit_changes = Some(plan.changes(id));
            next
        }
        ReviewWorkspaceCommand::Restore {
            archive_id,
            decisions,
        } => {
            let archive = repository.load_archive(before.stream_id, *archive_id)?;
            let bases = super::archiving::bases(
                repository,
                before.stream_id,
                &archive.groups,
                Some(archive.before),
            )?;
            let mut with_assets = before.clone();
            for decision in decisions {
                if let RestoreChoice::ContinueAsNew {
                    target_asset_version_id,
                    ..
                } = decision.choice
                {
                    super::editing::add_asset(&mut with_assets, target_asset_version_id, assets)?;
                }
            }
            let (next, plan) = apply_restore(&with_assets, &archive, &bases, decisions, snapshot)?;
            if plan.restored.is_empty() {
                return Err(ReviewWorkspaceError::NoChanges);
            }
            let mut changes = super::editing::changes(before, &next, kind);
            for item in plan.restored.iter().filter(|r| !r.continued_as_new) {
                let change = ReviewChange {
                    target_id: item.target.id,
                    before: before.target_key(item.target.id),
                    after: next.target_key(item.target.id),
                    kind: ReviewChangeKind::Restored,
                    archive_id: Some(*archive_id),
                    historical_key: Some(item.historical_key),
                };
                if let Some(old) = changes.iter_mut().find(|c| c.target_id == item.target.id) {
                    *old = change;
                } else {
                    changes.push(change);
                }
            }
            explicit_changes = Some(changes);
            next
        }
        ReviewWorkspaceCommand::ContinueHistorical { .. } => {
            super::history::continue_historical(repository, before, envelope, assets, usages)?
        }
        ReviewWorkspaceCommand::ContinueLegacy { .. } => {
            super::legacy_history::continue_legacy(repository, before, envelope, assets)?
        }
        ReviewWorkspaceCommand::ConfirmSource(decision) => {
            let mut with_assets = before.clone();
            super::editing::add_asset(&mut with_assets, decision.new_asset_version_id, assets)?;
            let new = with_assets
                .assets
                .iter()
                .find(|a| a.id == decision.new_asset_version_id)
                .ok_or(ContinuousReviewError::MissingReference)?;
            super::usage::validate_binding(&with_assets, new, decision, usages)?;
            kind = ReviewChangeKind::Rebound;
            apply_source_binding(
                &with_assets,
                decision,
                envelope
                    .generated
                    .targets
                    .first()
                    .ok_or(ContinuousReviewError::InvalidData)?
                    .1,
                snapshot,
            )?
        }
        ReviewWorkspaceCommand::ConfirmApplicability {
            key,
            asset_version_id,
            anchor,
        } => {
            if !before
                .feedback
                .iter()
                .flat_map(|f| &f.targets)
                .any(|t| t.id == key.target_id && t.asset_version_id == *asset_version_id)
            {
                return Err(ContinuousReviewError::MissingReference.into());
            }
            confirm_applicability(
                before,
                *key,
                anchor.clone(),
                envelope
                    .generated
                    .targets
                    .first()
                    .ok_or(ContinuousReviewError::InvalidData)?
                    .1,
                snapshot,
            )?
        }
        ReviewWorkspaceCommand::AdoptUsage { declaration_id } => {
            if !usages.iter().any(|u| u.declaration.id == *declaration_id) {
                return Err(ReviewWorkspaceError::CapabilityUnavailable);
            }
            let mut next = before.clone();
            next.snapshot_id = snapshot;
            next.parent = None;
            next
        }
        ReviewWorkspaceCommand::Migrate(_) => {
            return Err(ReviewWorkspaceError::CapabilityUnavailable);
        }
    };
    if next.snapshot_id == before.snapshot_id {
        return Err(ReviewWorkspaceError::NoChanges);
    }
    let changes = explicit_changes.unwrap_or_else(|| super::editing::changes(before, &next, kind));
    Ok(Transition {
        next,
        changes,
        archives,
    })
}
