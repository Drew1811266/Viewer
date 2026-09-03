use super::super::{MAX_REVIEW_DOCUMENT_BYTES, protocol, v3};
use super::{
    history,
    owned_io::Directory,
    repository::{View, protocol_error},
};
use viewer_application::{
    MAX_REVIEW_ARTIFACT_BYTES, ReviewArtifactError,
    review_evidence::{BoundReviewImage, EvidenceRole, HistorySelector},
    review_workspace::ReviewCommitError as Error,
};
use viewer_domain::{
    AssetVersionId, ReviewRoundId, ReviewStreamId,
    review::{AssetVersion, continuous::ArchiveBasis},
};

pub(super) fn load(
    view: &View,
    stream: ReviewStreamId,
    selector: &HistorySelector,
    asset: AssetVersionId,
    role: EvidenceRole,
) -> Result<BoundReviewImage, Error> {
    let binding = match *selector {
        HistorySelector::Snapshot(reference) => select(
            &history::reachable(view, stream, &reference)?.record,
            asset,
            role,
        )?,
        HistorySelector::Archive(id) => {
            let archive = history::archive(view, stream, id)?;
            let mut selected = None;
            for group in &archive.checkpoint.groups {
                let reference = match group.basis {
                    ArchiveBasis::Known { snapshot, .. } => snapshot,
                    ArchiveBasis::Unknown => archive.checkpoint.before,
                };
                let state = history::reachable(view, stream, &reference)?;
                let relevant = state.state.feedback.iter().any(|f| {
                    f.targets.iter().any(|t| {
                        t.asset_version_id == asset
                            && state
                                .state
                                .target_key(t.id)
                                .is_some_and(|k| group.targets.contains(&k))
                    })
                });
                if !relevant {
                    continue;
                }
                let binding = select(&state.record, asset, role)?;
                if selected.as_ref().is_some_and(|old| old != &binding) {
                    return Err(Error::AmbiguousEvidence);
                }
                selected = Some(binding);
            }
            selected.ok_or(Error::Integrity)?
        }
        HistorySelector::Legacy(id) => return legacy(view, stream, id, asset, role),
    };
    let directory = view.directory.required_child("evidence")?;
    let name = format!(
        "{}.png",
        blake3::Hash::from_bytes(binding.1.blake3).to_hex()
    );
    bound(&directory, &name, binding.0, binding.1, role)
}

fn select(
    record: &v3::ReviewStateRecord,
    id: AssetVersionId,
    role: EvidenceRole,
) -> Result<(AssetVersion, v3::EvidenceRef), Error> {
    let asset = record
        .state
        .assets
        .iter()
        .find(|a| a.id == id)
        .ok_or(Error::Integrity)?;
    let capability = &record
        .evidence
        .iter()
        .find(|e| e.asset_version_id == id)
        .ok_or(Error::Integrity)?
        .capability;
    let reference = match capability {
        v3::EvidenceCapability::Image {
            base, annotated, ..
        } => match role {
            EvidenceRole::Base => base,
            EvidenceRole::Annotated => annotated.as_ref().ok_or(Error::EvidenceAbsent)?,
        },
        v3::EvidenceCapability::LegacyAbsent {} | v3::EvidenceCapability::NotImage {} => {
            return Err(Error::EvidenceAbsent);
        }
    };
    Ok((asset.clone(), reference.clone()))
}

fn bound(
    directory: &Directory,
    name: &str,
    asset: AssetVersion,
    reference: v3::EvidenceRef,
    role: EvidenceRole,
) -> Result<BoundReviewImage, Error> {
    let mut file = directory.regular(name, false)?.ok_or(Error::Integrity)?;
    let mut bytes = Vec::new();
    super::evidence::copy_png(&mut file, &reference, &mut bytes)?;
    directory.verify()?;
    BoundReviewImage::from_verified_png(asset, reference.into(), role, bytes).map_err(|error| {
        match error {
            ReviewArtifactError::LimitExceeded => Error::LimitExceeded,
            _ => Error::Integrity,
        }
    })
}

fn legacy(
    view: &View,
    stream_id: ReviewStreamId,
    id: ReviewRoundId,
    asset_id: AssetVersionId,
    role: EvidenceRole,
) -> Result<BoundReviewImage, Error> {
    let stream = history::stream(view, stream_id)?;
    let reference = stream
        .legacy_refs
        .iter()
        .find(|r| r.round_id == id)
        .ok_or(Error::Integrity)?;
    if reference.kind == v3::LegacyRecordKind::Draft {
        let verified = super::legacy::load(view, stream_id, id)?;
        if let viewer_application::review_workspace::LegacyReviewContents::Draft(draft) =
            verified.contents
        {
            return if draft.assets.iter().any(|a| a.id == asset_id) {
                Err(Error::EvidenceAbsent)
            } else {
                Err(Error::Integrity)
            };
        }
        return Err(Error::Integrity);
    }
    let rounds = view.directory.required_child("rounds")?;
    let (directory, name) = match reference.protocol_version.as_str() {
        protocol::REVIEW_PROTOCOL_V1 => (rounds, format!("{id}.json")),
        protocol::REVIEW_PROTOCOL_V2 => {
            (rounds.required_child(&id.to_string())?, "round.json".into())
        }
        _ => return Err(Error::Integrity),
    };
    let bytes = directory
        .read(&name, MAX_REVIEW_DOCUMENT_BYTES)?
        .ok_or(Error::Integrity)?;
    history::verify_digest(&bytes, &reference.blake3)?;
    if protocol::detect_review_protocol(&bytes).map_err(protocol_error)?
        != reference.protocol_version
    {
        return Err(Error::Integrity);
    }
    let (snapshot, artifacts) = if reference.protocol_version == protocol::REVIEW_PROTOCOL_V1 {
        (
            protocol::decode_completed(&bytes).map_err(protocol_error)?,
            vec![],
        )
    } else {
        let document = protocol::v2::decode_completed_document(&bytes).map_err(protocol_error)?;
        (document.snapshot, document.artifacts)
    };
    if snapshot.project_id != view.index.project_id
        || snapshot.review_stream_id != stream_id
        || snapshot.review_round_id != id
        || snapshot.production != super::mapping::scope(stream)?
    {
        return Err(Error::Integrity);
    }
    let asset = snapshot
        .assets
        .iter()
        .find(|a| a.id == asset_id)
        .ok_or(Error::Integrity)?;
    // Legacy previews contain drawn marks: they can never authorize a clean base.
    if role == EvidenceRole::Base {
        return Err(Error::EvidenceAbsent);
    }
    let artifact = artifacts
        .iter()
        .find(|a| a.asset_version_id == asset_id)
        .ok_or(Error::EvidenceAbsent)?;
    let directory = directory.required_child("artifacts")?;
    let name = format!("{asset_id}-annotation.png");
    let file = directory.regular(&name, false)?.ok_or(Error::Integrity)?;
    let size_bytes = file.metadata().map_err(|_| Error::Io)?.len();
    if size_bytes > MAX_REVIEW_ARTIFACT_BYTES {
        return Err(Error::LimitExceeded);
    }
    let reference = v3::EvidenceRef {
        blake3: artifact.blake3,
        size_bytes,
        width: artifact.width,
        height: artifact.height,
    };
    bound(&directory, &name, asset.clone(), reference, role)
}
