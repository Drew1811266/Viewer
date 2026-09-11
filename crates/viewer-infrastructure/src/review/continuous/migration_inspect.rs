use super::{legacy, owned_io::Directory, repository::protocol_error};
use crate::review::{MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, protocol, v3};
use std::{collections::HashSet, path::Path};
use viewer_application::{PersistedReviewDraft, ReviewProtocolVersion, review_workspace::*};
use viewer_domain::ProjectId;

pub(super) struct InspectedLegacy {
    pub index_bytes: Vec<u8>,
    pub index: v3::ReviewIndexV3,
    pub inspection: MigrationInspection,
}
impl InspectedLegacy {
    pub fn has_data(&self) -> bool {
        !self.inspection.legacy_records.is_empty()
    }
}

pub(super) fn inspect(
    path: &Path,
    project: ProjectId,
) -> Result<Option<MigrationInspection>, ReviewCommitError> {
    let root = Directory::open(path)?;
    let Some(viewer) = root.child(".viewer", false)? else {
        return Ok(None);
    };
    let Some(reviews) = viewer.child("reviews", false)? else {
        return Ok(None);
    };
    Ok(scan(&reviews, project)?
        .filter(InspectedLegacy::has_data)
        .map(|v| v.inspection))
}

pub(super) fn scan(
    directory: &Directory,
    project: ProjectId,
) -> Result<Option<InspectedLegacy>, ReviewCommitError> {
    scan_index(
        directory,
        project,
        directory.read("index.json", MAX_REVIEW_INDEX_BYTES)?,
    )
}

pub(super) fn scan_index(
    directory: &Directory,
    project: ProjectId,
    bytes: Option<Vec<u8>>,
) -> Result<Option<InspectedLegacy>, ReviewCommitError> {
    let Some(bytes) = bytes else {
        verify_without_index(directory)?;
        return Ok(None);
    };
    match protocol::detect_continuous_review_protocol(&bytes, MAX_REVIEW_INDEX_BYTES) {
        Ok(protocol::ContinuousReviewProtocol::V3) => {
            v3::decode_index_v3(&bytes).map_err(protocol_error)?;
            return Ok(None);
        }
        Ok(protocol::ContinuousReviewProtocol::V4) => {
            protocol::v4::decode_index_v4(&bytes).map_err(protocol_error)?;
            return Ok(None);
        }
        Err(protocol::ReviewProtocolError::UnsupportedVersion) => {}
        Err(error) => return Err(protocol_error(error)),
    }
    // A valid continuous index (including the current v4 protocol) is already the
    // authoritative format.  Do not send it through the legacy catalog decoder;
    // doing so reports a false unsupported-protocol migration error to the UI.
    if super::super::detect_continuous_review_protocol(&bytes, MAX_REVIEW_INDEX_BYTES).is_ok() {
        return Ok(None);
    }
    let decoded = protocol::decode_catalog_versioned(&bytes).map_err(protocol_error)?;
    if decoded.value.project_id != project {
        return Err(ReviewCommitError::Integrity);
    }
    let mut index = v3::ReviewIndexV3 {
        project_id: project,
        streams: vec![],
        legacy_index: Some(v3::LegacyIndexRef {
            blake3: *blake3::hash(&bytes).as_bytes(),
        }),
    };
    let mut inspection = MigrationInspection {
        legacy_protocol: decoded.version,
        index_digest: *blake3::hash(&bytes).as_bytes(),
        inspection_digest: [0; 32],
        active_draft: None,
        completed_candidates: vec![],
        legacy_records: vec![],
        limitations: vec![
            ReviewHistoryLimitation::BackgroundOnly,
            ReviewHistoryLimitation::LegacyEvidenceAbsent,
            ReviewHistoryLimitation::UsageUnconfirmed,
            ReviewHistoryLimitation::ExternalCopiesCannotBeRevoked,
        ],
    };
    let mut retained = 0_u64;
    scan_completed(
        directory,
        decoded.version,
        decoded.value.streams,
        &mut index,
        &mut inspection,
        &mut retained,
    )?;
    scan_drafts(directory, &mut index, &mut inspection, &mut retained)?;
    let encoded = v3::encode_index_v3(&index).map_err(protocol_error)?;
    let mut digest = blake3::Hasher::new();
    digest.update(b"viewer.review.migration-inspection/1\0");
    digest.update(&inspection.index_digest);
    digest.update(&encoded);
    inspection.inspection_digest = *digest.finalize().as_bytes();
    directory.verify()?;
    Ok(Some(InspectedLegacy {
        index_bytes: bytes,
        index,
        inspection,
    }))
}

/// Shared no-index policy; callers that already pinned absence must not reopen a newer index.
pub(super) fn verify_without_index(directory: &Directory) -> Result<(), ReviewCommitError> {
    if let Some(drafts) = directory.child("drafts", false)?
        && !drafts.entries(1)?.is_empty()
    {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}

fn scan_completed(
    directory: &Directory,
    version: ReviewProtocolVersion,
    streams: Vec<viewer_application::ReviewStreamHead>,
    index: &mut v3::ReviewIndexV3,
    inspection: &mut MigrationInspection,
    retained: &mut u64,
) -> Result<(), ReviewCommitError> {
    let project = index.project_id;
    let mut ids = HashSet::new();
    for stream in streams {
        let mut next = v3::ReviewStreamV3 {
            review_stream_id: stream.review_stream_id,
            task_id: stream
                .production
                .as_ref()
                .map(|s| s.task_id.as_str().to_owned()),
            batch_id: stream
                .production
                .as_ref()
                .map(|s| s.batch_id.as_str().to_owned()),
            current_ref: None,
            archive_refs: vec![],
            usage_refs: vec![],
            legacy_refs: vec![],
        };
        for old in stream.completed_rounds {
            if inspection.legacy_records.len() >= 10_000 {
                return Err(ReviewCommitError::LimitExceeded);
            }
            if !ids.insert(old.review_round_id) {
                return Err(ReviewCommitError::Integrity);
            }
            let protocol_version = match old.protocol_version {
                ReviewProtocolVersion::V1 => protocol::REVIEW_PROTOCOL_V1,
                ReviewProtocolVersion::V2 => protocol::REVIEW_PROTOCOL_V2,
            };
            let mut reference = v3::LegacyRecordRef {
                kind: v3::LegacyRecordKind::Completed,
                round_id: old.review_round_id,
                protocol_version: protocol_version.into(),
                location: old.location.as_str().into(),
                blake3: old.blake3,
            };
            let (location, name) = legacy::location(directory, &reference)?;
            let body = location
                .read(&name, MAX_REVIEW_DOCUMENT_BYTES)?
                .ok_or(ReviewCommitError::Integrity)?;
            if version == ReviewProtocolVersion::V1 {
                reference.blake3 = *blake3::hash(&body).as_bytes();
            }
            let record = legacy::read(
                directory,
                project,
                stream.review_stream_id,
                stream.production.clone(),
                &reference,
            )?;
            inspection.legacy_records.push(record.reference);
            if stream.latest_completed_round_id == Some(old.review_round_id) {
                *retained = retained
                    .checked_add(body.len() as u64)
                    .filter(|v| *v <= MAX_REVIEW_DOCUMENT_BYTES)
                    .ok_or(ReviewCommitError::LimitExceeded)?;
                if let LegacyReviewContents::Completed(snapshot) = record.contents {
                    inspection.completed_candidates.push(snapshot);
                }
            }
            next.legacy_refs.push(reference);
        }
        index.streams.push(next);
    }
    Ok(())
}

fn scan_drafts(
    directory: &Directory,
    index: &mut v3::ReviewIndexV3,
    inspection: &mut MigrationInspection,
    retained: &mut u64,
) -> Result<(), ReviewCommitError> {
    let project = index.project_id;
    let ids: HashSet<_> = index
        .streams
        .iter()
        .flat_map(|s| &s.legacy_refs)
        .map(|r| r.round_id)
        .collect();
    if let Some(drafts) = directory.child("drafts", false)? {
        let mut names = drafts.entries(10_064)?;
        names.sort();
        let mut temporary = 0;
        for name in names {
            if name.starts_with(".viewer-review-") && name.ends_with(".tmp") {
                temporary += 1;
                if temporary > 64 {
                    return Err(ReviewCommitError::LimitExceeded);
                }
                continue;
            }
            let body = drafts
                .read(&name, MAX_REVIEW_DOCUMENT_BYTES)?
                .ok_or(ReviewCommitError::Integrity)?;
            let draft = protocol::decode_draft_versioned(&body).map_err(protocol_error)?;
            let d = &draft.value;
            if name != format!("{}.json", d.review_round_id) || d.project_id != project {
                return Err(ReviewCommitError::Integrity);
            }
            if ids.contains(&d.review_round_id) {
                if !index.streams.iter().any(|s| {
                    s.review_stream_id == d.review_stream_id
                        && super::mapping::scope(s).ok().as_ref() == Some(&d.production)
                        && s.legacy_refs
                            .iter()
                            .any(|r| r.round_id == d.review_round_id)
                }) {
                    return Err(ReviewCommitError::Integrity);
                }
                continue;
            }
            if inspection.active_draft.is_some() {
                return Err(ReviewCommitError::Integrity);
            }
            *retained = retained
                .checked_add(body.len() as u64)
                .filter(|v| *v <= MAX_REVIEW_DOCUMENT_BYTES)
                .ok_or(ReviewCommitError::LimitExceeded)?;
            let reference = v3::LegacyRecordRef {
                kind: v3::LegacyRecordKind::Draft,
                round_id: d.review_round_id,
                protocol_version: match draft.version {
                    ReviewProtocolVersion::V1 => protocol::REVIEW_PROTOCOL_V1,
                    ReviewProtocolVersion::V2 => protocol::REVIEW_PROTOCOL_V2,
                }
                .into(),
                location: format!("drafts/{}.json", d.review_round_id),
                blake3: *blake3::hash(&body).as_bytes(),
            };
            if !index
                .streams
                .iter()
                .any(|s| s.review_stream_id == d.review_stream_id)
            {
                index.streams.push(v3::ReviewStreamV3 {
                    review_stream_id: d.review_stream_id,
                    task_id: d.production.as_ref().map(|s| s.task_id.as_str().to_owned()),
                    batch_id: d
                        .production
                        .as_ref()
                        .map(|s| s.batch_id.as_str().to_owned()),
                    current_ref: None,
                    archive_refs: vec![],
                    legacy_refs: vec![],
                    usage_refs: vec![],
                });
            }
            let stream = index
                .streams
                .iter_mut()
                .find(|s| s.review_stream_id == d.review_stream_id)
                .ok_or(ReviewCommitError::Integrity)?;
            if super::mapping::scope(stream)? != d.production {
                return Err(ReviewCommitError::Integrity);
            }
            inspection.legacy_records.push(LegacyReviewReference {
                stream_id: d.review_stream_id,
                round_id: d.review_round_id,
                protocol: draft.version,
                is_draft: true,
                blake3: reference.blake3,
            });
            stream.legacy_refs.push(reference);
            inspection.active_draft = Some(PersistedReviewDraft {
                protocol_version: draft.version,
                draft: draft.value,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::inspect;
    use std::fs;
    use viewer_domain::ProjectId;

    #[test]
    fn current_v4_index_is_not_misclassified_as_legacy_migration() {
        let root = tempfile::tempdir().unwrap();
        let reviews = root.path().join(".viewer/reviews");
        fs::create_dir_all(&reviews).unwrap();
        let project_id = ProjectId::from_u128(1);
        fs::write(
            reviews.join("index.json"),
            format!(
                r#"{{"protocolVersion":"viewer.review/4","kind":"index","projectId":"{}","streams":[]}}"#,
                project_id
            ),
        )
        .unwrap();

        assert!(inspect(root.path(), project_id).unwrap().is_none());
    }
}
