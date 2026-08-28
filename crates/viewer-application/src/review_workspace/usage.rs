use super::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use viewer_domain::{
    AssetVersionId, RelativePath, ReviewTargetId,
    review::{AssetVersion, continuous::*},
};

#[derive(Clone)]
pub(super) struct VerifiedUsage {
    pub declaration: ReviewUsageDeclaration,
    target_assets: HashMap<ReviewTargetId, AssetVersionId>,
}

impl ContinuousReviewService {
    pub(super) async fn prepare_usages(
        &self,
        command: &ReviewWorkspaceCommand,
    ) -> Vec<PreparedUsageSelection> {
        let previews = self.usage_previews.lock().await;
        usage_ids(command)
            .into_iter()
            .map(|id| PreparedUsageSelection {
                id,
                candidate: previews.get(&id).map(|p| PreparedUsageCandidate {
                    canonical_digest: p.canonical_digest,
                    source_digest: p.source_digest,
                    source: p.source.clone(),
                }),
            })
            .collect()
    }

    pub fn with_usage_importer(mut self, importer: Arc<dyn UsageImportPort>) -> Self {
        self.usage_importer = Some(importer);
        self
    }
    pub async fn inspect_usage(
        &self,
        source: RelativePath,
    ) -> Result<UsageImportPreview, ReviewWorkspaceError> {
        let importer = self
            .usage_importer
            .clone()
            .ok_or(ReviewWorkspaceError::CapabilityUnavailable)?;
        let preview = super::service::work(move || Ok(importer.inspect(&source)?)).await?;
        if preview.declaration.project_id != self.context.project_id
            || preview.declaration.stream_id != self.context.stream_id
        {
            return Err(UsageImportError::WrongContext.into());
        }
        let mut previews = self.usage_previews.lock().await;
        if let Some(old) = previews.get(&preview.declaration.id)
            && (old.canonical_digest != preview.canonical_digest
                || old.declaration != preview.declaration)
        {
            return Err(UsageImportError::Conflict.into());
        }
        let retained = previews
            .values()
            .filter(|p| p.declaration.id != preview.declaration.id)
            .fold(0_usize, |sum, p| sum.saturating_add(preview_bytes(p)));
        if (previews.len() >= 128 && !previews.contains_key(&preview.declaration.id))
            || retained.saturating_add(preview_bytes(&preview)) > 64 * 1024 * 1024
        {
            return Err(UsageImportError::LimitExceeded.into());
        }
        previews.insert(preview.declaration.id, preview.clone());
        Ok(preview)
    }

    pub(super) async fn usages_for(
        &self,
        command: &ReviewWorkspaceCommand,
        repository: Arc<dyn ContinuousReviewRepositoryPort>,
        selections: Option<&[PreparedUsageSelection]>,
    ) -> Result<Vec<VerifiedUsage>, ReviewWorkspaceError> {
        let owned;
        let selections = if let Some(selections) = selections {
            if selections.iter().map(|s| s.id).collect::<Vec<_>>() != usage_ids(command) {
                return Err(UsageImportError::InvalidScope.into());
            }
            selections
        } else {
            owned = self.prepare_usages(command).await;
            &owned
        };
        let mut result = vec![];
        for selection in selections {
            let id = selection.id;
            let reader = repository.clone();
            let stream = self.context.stream_id;
            if let Some(stored) = super::service::io(move || reader.load_usage(stream, id)).await? {
                if let Some(expected) = &selection.candidate
                    && self.codec.usage_digest(&stored)? != expected.canonical_digest
                {
                    return Err(UsageImportError::SourceChanged.into());
                }
                if self
                    .usage_previews
                    .lock()
                    .await
                    .get(&id)
                    .is_some_and(|p| p.declaration != stored)
                {
                    return Err(UsageImportError::Conflict.into());
                }
                if matches!(command, ReviewWorkspaceCommand::AdoptUsage { .. }) {
                    return Err(ReviewWorkspaceError::NoChanges);
                }
                result.push(stored);
                continue;
            }
            let preview = self
                .usage_previews
                .lock()
                .await
                .get(&id)
                .cloned()
                .ok_or(ReviewWorkspaceError::CapabilityUnavailable)?;
            let expected = selection
                .candidate
                .as_ref()
                .ok_or(ReviewWorkspaceError::CapabilityUnavailable)?;
            if preview.canonical_digest != expected.canonical_digest
                || preview.source_digest != expected.source_digest
                || preview.source != expected.source
            {
                return Err(UsageImportError::SourceChanged.into());
            }
            let importer = self
                .usage_importer
                .clone()
                .ok_or(ReviewWorkspaceError::CapabilityUnavailable)?;
            let source = preview.source.clone();
            let current = super::service::work(move || Ok(importer.inspect(&source)?)).await?;
            if preview.canonical_digest != current.canonical_digest
                || preview.source_digest != current.source_digest
                || preview.declaration != current.declaration
            {
                return Err(UsageImportError::SourceChanged.into());
            }
            result.push(current.declaration);
        }
        validate_archive_scope(command, &result)?;
        let stream = self.context.stream_id;
        super::service::work(move || {
            let mut verified = vec![];
            for declaration in result {
                let basis = repository.load_snapshot(stream, &declaration.basis)?;
                let selected: HashSet<_> = declaration.targets.iter().copied().collect();
                let mut target_assets = HashMap::new();
                for feedback in &basis.state.feedback {
                    for target in &feedback.targets {
                        let key = TargetVersionKey {
                            feedback_id: feedback.id,
                            text_revision_id: feedback.text_revision_id,
                            target_id: target.id,
                            target_revision_id: target.revision_id,
                        };
                        if selected.contains(&key) {
                            target_assets.insert(target.id, target.asset_version_id);
                        }
                    }
                }
                if target_assets.len() != selected.len() {
                    return Err(UsageImportError::InvalidScope.into());
                }
                verified.push(VerifiedUsage {
                    declaration,
                    target_assets,
                });
            }
            Ok(verified)
        })
        .await
    }
}

fn preview_bytes(p: &UsageImportPreview) -> usize {
    let mut count = 4096_usize
        .saturating_add(std::mem::size_of_val(p.declaration.targets.as_slice()))
        .saturating_add(std::mem::size_of_val(p.declaration.outputs.as_slice()))
        .saturating_add(std::mem::size_of_val(p.outputs.as_slice()));
    for output in &p.declaration.outputs {
        count = count.saturating_add(output.relative_path.as_str().len());
    }
    for output in &p.outputs {
        count = count.saturating_add(output.output.relative_path.as_str().len());
    }
    count
}
fn validate_archive_scope(
    command: &ReviewWorkspaceCommand,
    usages: &[ReviewUsageDeclaration],
) -> Result<(), ReviewWorkspaceError> {
    if let ReviewWorkspaceCommand::Archive(selection) = command {
        for group in &selection.groups {
            if let ArchiveBasis::Known {
                snapshot,
                source: ArchiveBasisSource::AgentDeclared { usage_id },
            } = group.basis
            {
                let usage = usages
                    .iter()
                    .find(|u| u.id == usage_id)
                    .ok_or(UsageImportError::UnknownBasis)?;
                let keys: HashSet<_> = usage.targets.iter().collect();
                if usage.basis != snapshot || group.targets.iter().any(|k| !keys.contains(k)) {
                    return Err(UsageImportError::InvalidScope.into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_binding(
    before: &ContinuousReviewState,
    new: &AssetVersion,
    decision: &SourceBindingDecision,
    usages: &[VerifiedUsage],
) -> Result<(), ReviewWorkspaceError> {
    let SourceBindingConfirmation::ProducerVerifiedAndPositionConfirmed { usage_id } =
        decision.confirmation
    else {
        return Ok(());
    };
    let verified = usages
        .iter()
        .find(|u| u.declaration.id == usage_id)
        .ok_or(ReviewWorkspaceError::CapabilityUnavailable)?;
    let usage = &verified.declaration;
    let old = before
        .feedback
        .iter()
        .flat_map(|f| &f.targets)
        .find(|t| t.id == decision.target_key.target_id)
        .ok_or(ContinuousReviewError::MissingReference)?;
    if verified.target_assets.get(&old.id) != Some(&old.asset_version_id)
        || new.id != decision.new_asset_version_id
        || !usage.targets.iter().any(|k| {
            k.target_id == decision.target_key.target_id
                && k.feedback_id == decision.target_key.feedback_id
        })
        || !usage.outputs.iter().any(|o| {
            o.previous_asset_version_id == old.asset_version_id
                && o.relative_path == new.relative_path
                && Some(o.blake3) == new.evidence.blake3
        })
    {
        return Err(UsageImportError::InvalidScope.into());
    }
    Ok(())
}

fn usage_ids(command: &ReviewWorkspaceCommand) -> Vec<viewer_domain::ReviewUsageId> {
    let mut ids = vec![];
    let mut seen = HashSet::new();
    let mut add = |id| {
        if seen.insert(id) {
            ids.push(id);
        }
    };
    match command {
        ReviewWorkspaceCommand::AdoptUsage { declaration_id } => add(*declaration_id),
        ReviewWorkspaceCommand::Archive(selection) => {
            for group in &selection.groups {
                if let ArchiveBasis::Known {
                    source: ArchiveBasisSource::AgentDeclared { usage_id },
                    ..
                } = group.basis
                {
                    add(usage_id);
                }
            }
        }
        ReviewWorkspaceCommand::ConfirmSource(binding) => {
            if let SourceBindingConfirmation::ProducerVerifiedAndPositionConfirmed { usage_id } =
                binding.confirmation
            {
                add(usage_id);
            }
        }
        ReviewWorkspaceCommand::ContinueHistorical { bindings, .. } => {
            for binding in bindings {
                if let SourceBindingConfirmation::ProducerVerifiedAndPositionConfirmed {
                    usage_id,
                } = binding.confirmation
                {
                    add(usage_id);
                }
            }
        }
        _ => {}
    }
    ids
}

pub(super) fn selection_bytes(selections: &[PreparedUsageSelection]) -> usize {
    selections.iter().fold(0_usize, |sum, s| {
        sum.saturating_add(256)
            .saturating_add(s.candidate.as_ref().map_or(0, |p| p.source.as_str().len()))
    })
}
