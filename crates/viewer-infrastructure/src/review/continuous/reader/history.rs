use super::{
    Failure, ReadErrorCode,
    project::CurrentProject,
    request::{Request, parse_id},
};
use crate::review::{
    continuous::{evidence, history, legacy, mapping, references, repository::View},
    v3,
};
use viewer_application::review_workspace::LegacyReviewContents;
use viewer_domain::{
    ReviewSnapshotId, ReviewStreamId,
    review::{
        ReviewMedia,
        continuous::{ArchiveBasis, SnapshotRef, TargetVersionKey},
    },
};

pub(super) fn read(
    project: &CurrentProject,
    request: &Request,
) -> Result<serde_json::Value, Failure> {
    let view = project.view.as_ref().ok_or_else(|| {
        Failure::new(
            ReadErrorCode::UnknownHistory,
            "project has no review history",
        )
    })?;
    let stream = super::current::selected(view, request)?
        .ok_or_else(|| Failure::new(ReadErrorCode::UnknownStream, "review stream was not found"))?;
    let stream_id = stream.review_stream_id;
    let mut limitations = vec![];
    let (selector, entries) = if let Some(id) = &request.snapshot_id {
        let (reference, record) = snapshot(view, stream_id, parse_id(id)?)?;
        let keys = keys(&record);
        (
            v3::ReadHistorySelector::Snapshot {
                snapshot: reference,
            },
            vec![entry(reference, record, keys, &mut limitations)],
        )
    } else if let Some(id) = &request.archive_id {
        let id = parse_id(id)?;
        if !stream.archive_refs.iter().any(|r| r.archive_id == id) {
            return Err(Failure::new(
                ReadErrorCode::UnknownHistory,
                "archive is not indexed",
            ));
        }
        let archive = history::archive(view, stream_id, id)?;
        let mut groups: Vec<(SnapshotRef, Vec<TargetVersionKey>)> = vec![];
        for group in archive.checkpoint.groups {
            let reference = match group.basis {
                ArchiveBasis::Known { snapshot, .. } => snapshot,
                ArchiveBasis::Unknown => archive.checkpoint.before,
            };
            if let Some((_, keys)) = groups.iter_mut().find(|(r, _)| *r == reference) {
                keys.extend(group.targets);
            } else {
                groups.push((reference, group.targets));
            }
        }
        let mut retained_bytes = 0_u64;
        let mut entries = vec![];
        for (reference, keys) in groups {
            let record = history::reachable(view, stream_id, &reference)?;
            references::feedback_origins(view, &record)?;
            retained_bytes += v3::encode_state_v3(&record)?.len() as u64;
            if retained_bytes > crate::review::MAX_REVIEW_DOCUMENT_BYTES {
                return Err(Failure::new(
                    ReadErrorCode::LimitExceeded,
                    "history result exceeds retained document limit",
                ));
            }
            entries.push(entry(reference, record.record, keys, &mut limitations));
        }
        (v3::ReadHistorySelector::Archive { archive_id: id }, entries)
    } else {
        let id = parse_id(
            request
                .legacy_round_id
                .as_deref()
                .ok_or_else(|| Failure::integrity("missing explicit history selector"))?,
        )?;
        let reference = stream
            .legacy_refs
            .iter()
            .find(|r| r.round_id == id)
            .ok_or_else(|| {
                Failure::new(ReadErrorCode::UnknownHistory, "legacy round is not indexed")
            })?;
        let (record, images) = legacy::read_with_evidence(
            &view.directory,
            project.project_id,
            stream_id,
            mapping::scope(stream)?,
            reference,
        )?;
        let (assets, feedback) = match record.contents {
            LegacyReviewContents::Draft(d) => (d.assets, d.feedback),
            LegacyReviewContents::Completed(d) => (d.assets, d.feedback),
        };
        limitations.push(v3::HistoryLimitation::LegacyUsageUnknown);
        if assets
            .iter()
            .any(|a| matches!(a.media, ReviewMedia::Image { .. }))
        {
            limitations.push(v3::HistoryLimitation::LegacyEvidenceAbsent);
        }
        (
            v3::ReadHistorySelector::Legacy { round_id: id },
            vec![v3::HistoryEntry::Legacy {
                reference: reference.clone(),
                assets,
                feedback,
                evidence: images,
            }],
        )
    };
    project.root.verify()?;
    super::current::result(v3::ReviewReadResult::History(
        v3::HistoryReadResult::from_verified(
            project.project_id,
            stream_id,
            selector,
            entries,
            limitations,
        ),
    ))
}

fn snapshot(
    view: &View,
    stream: ReviewStreamId,
    id: ReviewSnapshotId,
) -> Result<(SnapshotRef, v3::ReviewStateRecord), Failure> {
    let mut found = None;
    history::walk(view, stream, |reference, record| {
        if reference.snapshot_id != id {
            return Ok(false);
        }
        evidence::verify(view, &record.evidence)?;
        references::feedback_origins(view, &record)?;
        found = Some((reference, record.record));
        Ok(true)
    })?;
    found.ok_or_else(|| {
        Failure::new(
            ReadErrorCode::UnknownHistory,
            "snapshot is not in committed history",
        )
    })
}
fn keys(record: &v3::ReviewStateRecord) -> Vec<TargetVersionKey> {
    record
        .state
        .feedback
        .iter()
        .flat_map(|f| {
            f.targets.iter().map(|t| TargetVersionKey {
                feedback_id: f.id,
                text_revision_id: f.text_revision_id,
                target_id: t.id,
                target_revision_id: t.revision_id,
            })
        })
        .collect()
}
fn entry(
    reference: SnapshotRef,
    record: v3::ReviewStateRecord,
    selected_targets: Vec<TargetVersionKey>,
    limitations: &mut Vec<v3::HistoryLimitation>,
) -> v3::HistoryEntry {
    if record
        .evidence
        .iter()
        .any(|e| matches!(e.capability, v3::EvidenceCapability::LegacyAbsent {}))
        && !limitations.contains(&v3::HistoryLimitation::LegacyEvidenceAbsent)
    {
        limitations.push(v3::HistoryLimitation::LegacyEvidenceAbsent);
    }
    v3::HistoryEntry::Snapshot {
        snapshot_ref: reference,
        assets: record.state.assets,
        feedback: record.state.feedback,
        evidence: record.evidence,
        selected_targets,
    }
}
