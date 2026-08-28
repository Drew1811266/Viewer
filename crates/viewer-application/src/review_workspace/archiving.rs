use super::*;
use std::collections::HashSet;
use viewer_domain::{ReviewArchiveId, ReviewStreamId, review::continuous::*};

pub(super) fn bases(
    repository: &dyn ContinuousReviewRepositoryPort,
    stream: ReviewStreamId,
    groups: &[ArchiveGroup],
    before: Option<SnapshotRef>,
) -> Result<Vec<ContinuousReviewState>, ReviewWorkspaceError> {
    let mut seen = HashSet::new();
    let mut result = vec![];
    for reference in before
        .into_iter()
        .chain(groups.iter().filter_map(|g| match g.basis {
            ArchiveBasis::Known { snapshot, .. } => Some(snapshot),
            ArchiveBasis::Unknown => None,
        }))
    {
        if seen.insert(reference) {
            result.push(repository.load_snapshot(stream, &reference)?.state);
        }
    }
    Ok(result)
}

pub(super) fn archive_plan(
    repository: &dyn ContinuousReviewRepositoryPort,
    current: &StoredContinuousSnapshot,
    selection: &ArchiveSelection,
) -> Result<
    (
        ArchivePlan,
        Vec<ContinuousReviewState>,
        Vec<ArchiveCoverage>,
    ),
    ReviewWorkspaceError,
> {
    let bases = bases(repository, current.state.stream_id, &selection.groups, None)?;
    let keys: Vec<_> = selection
        .groups
        .iter()
        .flat_map(|g| &g.targets)
        .copied()
        .collect();
    let coverage = repository.load_coverage(current.state.stream_id, current.reference, &keys)?;
    Ok((
        plan_archive(&current.state, &bases, selection, &coverage)?,
        bases,
        coverage,
    ))
}

impl ContinuousReviewService {
    pub async fn preview_archive(
        &self,
        selection: ArchiveSelection,
    ) -> Result<ArchivePlan, ReviewWorkspaceError> {
        let provider = self.provider.clone();
        let stream = self.context.stream_id;
        let read_provider = provider.clone();
        let repository = super::service::io(move || read_provider.open_reader()).await?;
        self.usages_for(
            &ReviewWorkspaceCommand::Archive(selection.clone()),
            repository,
            None,
        )
        .await?;
        super::service::work(move || {
            let reader = provider.open_reader()?;
            let current = reader
                .load_current(stream)?
                .ok_or(ContinuousReviewError::MissingReference)?;
            Ok(archive_plan(reader.as_ref(), &current, &selection)?.0)
        })
        .await
    }
    pub async fn preview_restore(
        &self,
        archive_id: ReviewArchiveId,
        decisions: Vec<RestoreDecision>,
    ) -> Result<RestorePlan, ReviewWorkspaceError> {
        let provider = self.provider.clone();
        let stream = self.context.stream_id;
        let prepared = self.prepared.lock().await.clone();
        super::service::work(move || {
            let reader = provider.open_reader()?;
            let mut current = reader
                .load_current(stream)?
                .ok_or(ContinuousReviewError::MissingReference)?;
            for decision in &decisions {
                if let RestoreChoice::ContinueAsNew {
                    target_asset_version_id,
                    ..
                } = decision.choice
                {
                    super::editing::add_asset(
                        &mut current.state,
                        target_asset_version_id,
                        &prepared,
                    )?;
                }
            }
            let archive = reader.load_archive(stream, archive_id)?;
            let bases = bases(
                reader.as_ref(),
                stream,
                &archive.groups,
                Some(archive.before),
            )?;
            Ok(plan_restore(&current.state, &archive, &bases, &decisions)?)
        })
        .await
    }
}
