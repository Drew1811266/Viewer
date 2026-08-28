//! Verified legacy records. Annotated legacy PNGs never authorize a clean base image.
use super::{
    history,
    owned_io::Directory,
    repository::{View, protocol_error},
};
use crate::review::{MAX_REVIEW_DOCUMENT_BYTES, protocol, v3};
use viewer_application::{ReviewProtocolVersion, review_workspace::*};
use viewer_domain::{ProjectId, ReviewStreamId, review::ProductionScope};

pub(super) fn location(
    root: &Directory,
    reference: &v3::LegacyRecordRef,
) -> Result<(Directory, String), ReviewCommitError> {
    if reference.kind == v3::LegacyRecordKind::Draft {
        return Ok((
            root.required_child("drafts")?,
            format!("{}.json", reference.round_id),
        ));
    }
    let rounds = root.required_child("rounds")?;
    match reference.protocol_version.as_str() {
        protocol::REVIEW_PROTOCOL_V1 => Ok((rounds, format!("{}.json", reference.round_id))),
        protocol::REVIEW_PROTOCOL_V2 => Ok((
            rounds.required_child(&reference.round_id.to_string())?,
            "round.json".into(),
        )),
        _ => Err(ReviewCommitError::UnsupportedProtocol),
    }
}

pub(super) fn read(
    root: &Directory,
    project: ProjectId,
    stream: ReviewStreamId,
    scope: Option<ProductionScope>,
    reference: &v3::LegacyRecordRef,
) -> Result<LegacyReviewRecord, ReviewCommitError> {
    read_with_evidence(root, project, stream, scope, reference).map(|(record, _)| record)
}

pub(super) fn read_with_evidence(
    root: &Directory,
    project: ProjectId,
    stream: ReviewStreamId,
    scope: Option<ProductionScope>,
    reference: &v3::LegacyRecordRef,
) -> Result<(LegacyReviewRecord, Vec<v3::LegacyEvidence>), ReviewCommitError> {
    #[cfg(test)]
    LEGACY_READS.with(|c| c.set(c.get() + 1));
    let (directory, name) = location(root, reference)?;
    let bytes = directory
        .read(&name, MAX_REVIEW_DOCUMENT_BYTES)?
        .ok_or(ReviewCommitError::Integrity)?;
    history::verify_digest(&bytes, &reference.blake3)?;
    if protocol::detect_review_protocol(&bytes).map_err(protocol_error)?
        != reference.protocol_version
    {
        return Err(ReviewCommitError::Integrity);
    }
    let protocol = match reference.protocol_version.as_str() {
        protocol::REVIEW_PROTOCOL_V1 => ReviewProtocolVersion::V1,
        protocol::REVIEW_PROTOCOL_V2 => ReviewProtocolVersion::V2,
        _ => return Err(ReviewCommitError::UnsupportedProtocol),
    };
    let mut images = vec![];
    let contents = if reference.kind == v3::LegacyRecordKind::Draft {
        LegacyReviewContents::Draft(
            protocol::decode_draft_versioned(&bytes)
                .map_err(protocol_error)?
                .value,
        )
    } else if protocol == ReviewProtocolVersion::V1 {
        LegacyReviewContents::Completed(protocol::decode_completed(&bytes).map_err(protocol_error)?)
    } else {
        let document = protocol::v2::decode_completed_document(&bytes).map_err(protocol_error)?;
        images = verify_artifacts(&directory, &document.artifacts)?;
        LegacyReviewContents::Completed(document.snapshot)
    };
    let (actual_project, actual_stream, round, production) = match &contents {
        LegacyReviewContents::Draft(d) => (
            d.project_id,
            d.review_stream_id,
            d.review_round_id,
            &d.production,
        ),
        LegacyReviewContents::Completed(d) => (
            d.project_id,
            d.review_stream_id,
            d.review_round_id,
            &d.production,
        ),
    };
    if actual_project != project
        || actual_stream != stream
        || round != reference.round_id
        || *production != scope
    {
        return Err(ReviewCommitError::Integrity);
    }
    Ok((
        LegacyReviewRecord {
            reference: LegacyReviewReference {
                stream_id: stream,
                round_id: round,
                protocol,
                is_draft: reference.kind == v3::LegacyRecordKind::Draft,
                blake3: reference.blake3,
            },
            contents,
        },
        images,
    ))
}

pub(super) fn verify_artifacts(
    directory: &Directory,
    records: &[protocol::v2::V2ArtifactRecord],
) -> Result<Vec<v3::LegacyEvidence>, ReviewCommitError> {
    let mut bundle_names = directory.entries(2)?;
    bundle_names.sort();
    if bundle_names != ["artifacts", "round.json"] {
        return Err(ReviewCommitError::Integrity);
    }
    let artifacts = directory.required_child("artifacts")?;
    let expected: std::collections::HashSet<_> = records
        .iter()
        .map(|a| format!("{}-annotation.png", a.asset_version_id))
        .collect();
    let names = artifacts.entries(50_000)?;
    if names.len() != expected.len() || names.iter().any(|n| !expected.contains(n)) {
        return Err(ReviewCommitError::Integrity);
    }
    let mut total = 0_u64;
    let mut images = Vec::with_capacity(records.len());
    for artifact in records {
        let mut file = artifacts
            .regular(
                &format!("{}-annotation.png", artifact.asset_version_id),
                false,
            )?
            .ok_or(ReviewCommitError::Integrity)?;
        let size_bytes = file.metadata().map_err(|_| ReviewCommitError::Io)?.len();
        total = total
            .checked_add(size_bytes)
            .filter(|v| *v <= 4 * 1024 * 1024 * 1024)
            .ok_or(ReviewCommitError::LimitExceeded)?;
        super::evidence::copy_png(
            &mut file,
            &v3::EvidenceRef {
                blake3: artifact.blake3,
                size_bytes,
                width: artifact.width,
                height: artifact.height,
            },
            &mut std::io::sink(),
        )?;
        artifacts.verify()?;
        images.push(v3::LegacyEvidence {
            asset_version_id: artifact.asset_version_id,
            relative_path: artifact.relative_path.clone(),
            image: v3::EvidenceRef {
                blake3: artifact.blake3,
                size_bytes,
                width: artifact.width,
                height: artifact.height,
            },
            annotations: artifact
                .annotations
                .iter()
                .map(|a| v3::LegacyAnnotation {
                    ordinal: a.ordinal,
                    feedback_id: a.feedback_id,
                })
                .collect(),
        });
    }
    directory.verify()?;
    Ok(images)
}

#[cfg(test)]
thread_local! {static LEGACY_READS:std::cell::Cell<usize>=const {std::cell::Cell::new(0)};}
#[cfg(test)]
#[path = "legacy_tests.rs"]
mod tests;

pub(super) fn load(
    view: &View,
    stream: ReviewStreamId,
    round: viewer_domain::ReviewRoundId,
) -> Result<LegacyReviewRecord, ReviewCommitError> {
    let stream = history::stream(view, stream)?;
    let reference = stream
        .legacy_refs
        .iter()
        .find(|r| r.round_id == round)
        .ok_or(ReviewCommitError::Integrity)?;
    read(
        &view.directory,
        view.index.project_id,
        stream.review_stream_id,
        super::mapping::scope(stream)?,
        reference,
    )
}
