use super::super::{ContinuousReviewProtocol, v3};
use viewer_application::review_workspace as app;
use viewer_domain::review::{ProductionId, ProductionScope, continuous::SnapshotRef};

impl From<app::ReviewPublicationProtocol> for ContinuousReviewProtocol {
    fn from(value: app::ReviewPublicationProtocol) -> Self {
        match value {
            app::ReviewPublicationProtocol::V3 => Self::V3,
            app::ReviewPublicationProtocol::V4 => Self::V4,
        }
    }
}

impl From<ContinuousReviewProtocol> for app::ReviewPublicationProtocol {
    fn from(value: ContinuousReviewProtocol) -> Self {
        match value {
            ContinuousReviewProtocol::V3 => Self::V3,
            ContinuousReviewProtocol::V4 => Self::V4,
        }
    }
}

impl From<app::EvidenceRef> for v3::EvidenceRef {
    fn from(value: app::EvidenceRef) -> Self {
        Self {
            blake3: value.blake3,
            size_bytes: value.size_bytes,
            width: value.width,
            height: value.height,
        }
    }
}
impl From<v3::EvidenceRef> for app::EvidenceRef {
    fn from(value: v3::EvidenceRef) -> Self {
        Self {
            blake3: value.blake3,
            size_bytes: value.size_bytes,
            width: value.width,
            height: value.height,
        }
    }
}
impl From<app::ReviewEvidenceBinding> for v3::EvidenceBinding {
    fn from(value: app::ReviewEvidenceBinding) -> Self {
        let capability = match value.capability {
            app::EvidenceCapability::Image {
                base,
                annotated,
                annotations,
            } => v3::EvidenceCapability::Image {
                base: base.into(),
                annotated: annotated.map(Into::into),
                annotations: annotations
                    .into_iter()
                    .map(|a| v3::EvidenceAnnotation {
                        ordinal: a.ordinal,
                        key: a.key,
                    })
                    .collect(),
            },
            app::EvidenceCapability::LegacyAbsent => v3::EvidenceCapability::LegacyAbsent {},
            app::EvidenceCapability::NotImage => v3::EvidenceCapability::NotImage {},
        };
        Self {
            asset_version_id: value.asset_version_id,
            capability,
        }
    }
}
impl From<v3::EvidenceBinding> for app::ReviewEvidenceBinding {
    fn from(value: v3::EvidenceBinding) -> Self {
        let capability = match value.capability {
            v3::EvidenceCapability::Image {
                base,
                annotated,
                annotations,
            } => app::EvidenceCapability::Image {
                base: base.into(),
                annotated: annotated.map(Into::into),
                annotations: annotations
                    .into_iter()
                    .map(|a| app::EvidenceAnnotation {
                        ordinal: a.ordinal,
                        key: a.key,
                    })
                    .collect(),
            },
            v3::EvidenceCapability::LegacyAbsent {} => app::EvidenceCapability::LegacyAbsent,
            v3::EvidenceCapability::NotImage {} => app::EvidenceCapability::NotImage,
        };
        Self {
            asset_version_id: value.asset_version_id,
            capability,
        }
    }
}
impl From<app::PreparedContinuousSnapshot> for v3::ReviewStateRecord {
    fn from(value: app::PreparedContinuousSnapshot) -> Self {
        Self {
            state: value.state,
            command_id: value.command_id,
            payload_digest: value.payload_digest,
            changes: value.changes,
            evidence: value.evidence.into_iter().map(Into::into).collect(),
        }
    }
}

pub(super) fn stored(
    protocol: app::ReviewPublicationProtocol,
    record: v3::ReviewStateRecord,
    reference: SnapshotRef,
    stream: &v3::ReviewStreamV3,
) -> Result<app::StoredContinuousSnapshot, app::ReviewCommitError> {
    Ok(app::StoredContinuousSnapshot {
        reference,
        publication_protocol: protocol,
        production: scope(stream)?,
        state: record.state,
        command_id: record.command_id,
        payload_digest: record.payload_digest,
        changes: record.changes,
        evidence: record.evidence.into_iter().map(Into::into).collect(),
    })
}

pub(super) fn scope(
    stream: &v3::ReviewStreamV3,
) -> Result<Option<ProductionScope>, app::ReviewCommitError> {
    match (&stream.task_id, &stream.batch_id) {
        (None, None) => Ok(None),
        (Some(task), Some(batch)) => Ok(Some(ProductionScope {
            task_id: ProductionId::parse(task).map_err(|_| app::ReviewCommitError::Integrity)?,
            batch_id: ProductionId::parse(batch).map_err(|_| app::ReviewCommitError::Integrity)?,
        })),
        _ => Err(app::ReviewCommitError::Integrity),
    }
}
