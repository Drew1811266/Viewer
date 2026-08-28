use super::{
    commit,
    faults::{NoReviewCommitFaults, ReviewCommitFaultInjector, ReviewCommitFaultPoint},
    history, migration_inspect,
    owned_io::Directory,
    repository::{ContinuousReviewRepository, View, protocol_error},
};
use crate::review::{MAX_REVIEW_INDEX_BYTES, atomic::atomic_replace_at_with_barrier, v3};
use std::{path::Path, sync::Arc};
use viewer_application::review_workspace::*;
use viewer_domain::ProjectId;

pub(in crate::review) fn inspect(
    path: &Path,
    project: ProjectId,
) -> Result<Option<MigrationInspection>, ReviewCommitError> {
    migration_inspect::inspect(path, project)
}

pub(in crate::review) fn migrate(
    path: &Path,
    project: ProjectId,
    request: MigrationCommitRequest,
    faults: Option<Arc<dyn ReviewCommitFaultInjector>>,
) -> Result<ReviewCommitReceipt, ReviewCommitError> {
    let repository = ContinuousReviewRepository::open_migration(
        path,
        project,
        faults.unwrap_or_else(|| Arc::new(NoReviewCommitFaults)),
    )?;
    let writer = repository
        .writer
        .as_ref()
        .ok_or(ReviewCommitError::ReadOnly)?;
    let _guard = writer.gate.lock().map_err(|_| ReviewCommitError::Io)?;
    let directory = repository
        .checked_directory()?
        .ok_or(ReviewCommitError::Integrity)?;
    let envelope = &request.envelope;
    if envelope.context.project_id != project
        || envelope.expected_snapshot_id.is_some()
        || super::ContinuousReviewCommandCodec
            .digest(envelope)
            .map_err(|_| ReviewCommitError::Integrity)?
            != envelope.payload_digest
    {
        return Err(ReviewCommitError::CommandConflict);
    }
    let ReviewWorkspaceCommand::Migrate(plan) = &envelope.command else {
        return Err(ReviewCommitError::Integrity);
    };
    let Some(inspected) = migration_inspect::scan(&directory, project)? else {
        let view = repository.view()?.ok_or(ReviewCommitError::Integrity)?;
        return match history::find_command(&view, envelope.context.stream_id, envelope.command_id)?
        {
            CommandLookup::Found(receipt) if receipt.payload_digest == envelope.payload_digest => {
                Ok(receipt)
            }
            CommandLookup::Found(_) => Err(ReviewCommitError::CommandConflict),
            CommandLookup::Absent => Err(ReviewCommitError::StaleSnapshot),
            CommandLookup::Unavailable => Err(ReviewCommitError::LookupUnavailable),
        };
    };
    if !inspected.has_data() || inspected.inspection.inspection_digest != plan.inspection_digest {
        return Err(ReviewCommitError::StaleSnapshot);
    }
    let expected = viewer_application::review_workspace::prepare_migration_state(
        &inspected.inspection,
        envelope,
        &request.next.state.assets,
    )
    .map_err(|_| ReviewCommitError::Integrity)?;
    if expected != request.next.state
        || request.next.command_id != envelope.command_id
        || request.next.payload_digest != envelope.payload_digest
    {
        return Err(ReviewCommitError::Integrity);
    }
    validate_evidence(plan, &request.next)?;
    let mut view = View {
        directory,
        index: inspected.index,
        index_bytes: Some(inspected.index_bytes.clone()),
        ancestry: Default::default(),
    };
    let prepared = super::prepare::prepare(
        &mut view,
        ReviewCommitRequest {
            expected: None,
            production: envelope.context.production.clone(),
            next: request.next,
            archives: vec![],
            adopted_usage: vec![],
            staged_evidence: request.staged_evidence,
        },
    )?;
    save_backup(&view.directory, &inspected.index_bytes)?;
    repository
        .faults
        .check(ReviewCommitFaultPoint::AfterRecovery)?;
    commit::install(&repository, &view, &prepared)?;
    commit::validate_installed(&view, &prepared)?;
    repository
        .faults
        .check(ReviewCommitFaultPoint::BeforeIndex)?;
    let observed = repository
        .checked_directory()?
        .ok_or(ReviewCommitError::Integrity)?;
    let latest =
        migration_inspect::scan(&observed, project)?.ok_or(ReviewCommitError::StaleSnapshot)?;
    if latest.inspection.inspection_digest != plan.inspection_digest {
        return Err(ReviewCommitError::StaleSnapshot);
    }
    atomic_replace_at_with_barrier(
        &view.directory.file,
        "index.json",
        &prepared.index_bytes,
        || {
            repository
                .faults
                .check(ReviewCommitFaultPoint::AfterIndex)
                .map_err(|_| std::io::Error::other("migration durability failure"))
        },
    )
    .map_err(|_| ReviewCommitError::OutcomeUnknown)?;
    repository
        .view()
        .map_err(|_| ReviewCommitError::OutcomeUnknown)?;
    Ok(ReviewCommitReceipt {
        command_id: envelope.command_id,
        payload_digest: envelope.payload_digest,
        snapshot: prepared.reference,
    })
}

fn validate_evidence(
    plan: &MigrationPlan,
    next: &PreparedContinuousSnapshot,
) -> Result<(), ReviewCommitError> {
    let fresh: std::collections::HashSet<_> = match &plan.choice {
        MigrationChoice::KeepHistoryOnly => Default::default(),
        MigrationChoice::ContinueSelected { bindings, .. } => {
            bindings.iter().map(|b| b.new_asset_version_id).collect()
        }
    };
    for asset in &next.state.assets {
        let binding = next
            .evidence
            .iter()
            .find(|b| b.asset_version_id == asset.id)
            .ok_or(ReviewCommitError::Integrity)?;
        let allowed = matches!(
            (&asset.media, fresh.contains(&asset.id), &binding.capability),
            (
                viewer_domain::review::ReviewMedia::Image { .. },
                true,
                EvidenceCapability::Image { .. },
            ) | (
                viewer_domain::review::ReviewMedia::Image { .. },
                false,
                EvidenceCapability::LegacyAbsent,
            ) | (
                viewer_domain::review::ReviewMedia::Video { .. },
                _,
                EvidenceCapability::NotImage
            )
        );
        if !allowed {
            return Err(ReviewCommitError::Integrity);
        }
    }
    Ok(())
}

pub(super) fn backup_name(hash: &[u8; 32]) -> String {
    format!(
        "legacy-index-{}.json",
        blake3::Hash::from_bytes(*hash).to_hex()
    )
}
pub(super) fn save_backup(directory: &Directory, bytes: &[u8]) -> Result<(), ReviewCommitError> {
    let recovery = directory
        .child("recovery", true)?
        .ok_or(ReviewCommitError::Integrity)?;
    let name = backup_name(blake3::hash(bytes).as_bytes());
    let mut count = 1;
    let mut total = bytes.len() as u64;
    for entry in recovery.entries(10_128)? {
        if entry.starts_with("legacy-index-") && entry != name {
            count += 1;
            let file = recovery
                .regular(&entry, false)?
                .ok_or(ReviewCommitError::Integrity)?;
            total = total
                .checked_add(file.metadata().map_err(|_| ReviewCommitError::Io)?.len())
                .ok_or(ReviewCommitError::LimitExceeded)?;
        }
    }
    if count > 64 || total > crate::review::MAX_REVIEW_DOCUMENT_BYTES {
        return Err(ReviewCommitError::LimitExceeded);
    }
    commit::create_record(&recovery, &name, bytes)
}
pub(super) fn verify_backup(
    directory: &Directory,
    reference: &v3::LegacyIndexRef,
    project: ProjectId,
) -> Result<(), ReviewCommitError> {
    let backup = directory
        .required_child("recovery")?
        .read(&backup_name(&reference.blake3), MAX_REVIEW_INDEX_BYTES)?
        .ok_or(ReviewCommitError::Integrity)?;
    history::verify_digest(&backup, &reference.blake3)?;
    let decoded =
        crate::review::protocol::decode_catalog_versioned(&backup).map_err(protocol_error)?;
    if decoded.value.project_id != project {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}
