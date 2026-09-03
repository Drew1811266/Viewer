use super::ReviewArchiveRecord;
use super::wire;
use crate::review::protocol::ContinuousReviewProtocol;
use serde::{Deserialize, Serialize};
use viewer_domain::review::continuous::*;
use viewer_domain::{ProjectId, ReviewArchiveId, ReviewSnapshotId, ReviewStreamId, ReviewUsageId};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Archive {
    protocol_version: ContinuousReviewProtocol,
    kind: ArchiveKind,
    #[serde(deserialize_with = "wire::canonical_id")]
    project_id: ProjectId,
    #[serde(deserialize_with = "wire::canonical_id")]
    review_stream_id: ReviewStreamId,
    #[serde(deserialize_with = "wire::canonical_id")]
    archive_id: ReviewArchiveId,
    created_at_ms: i64,
    #[serde(with = "wire::snapshot")]
    before_ref: SnapshotRef,
    #[serde(deserialize_with = "wire::canonical_id")]
    result_snapshot_id: ReviewSnapshotId,
    groups: Vec<Group>,
    #[serde(with = "wire::keys")]
    removed: Vec<TargetVersionKey>,
    retained: Vec<Retention>,
}
#[derive(Serialize, Deserialize)]
enum ArchiveKind {
    #[serde(rename = "archive")]
    Archive,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Group {
    #[serde(deserialize_with = "wire::required_option")]
    usage_basis: Option<Basis>,
    #[serde(with = "wire::keys")]
    targets: Vec<TargetVersionKey>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Basis {
    #[serde(with = "wire::snapshot")]
    snapshot: SnapshotRef,
    source: BasisSource,
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum BasisSource {
    UserSelected {},
    AgentDeclared { usage_id: ReviewUsageId },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Retention {
    #[serde(with = "wire::key")]
    basis: TargetVersionKey,
    #[serde(deserialize_with = "wire::required_option")]
    current: Option<wire::Key>,
    disposition: Disposition,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Disposition {
    RemoveCurrent,
    RetainLaterEdit,
    AlreadyAbsent,
}

impl Archive {
    pub fn from_record(record: &ReviewArchiveRecord, protocol: ContinuousReviewProtocol) -> Self {
        let v = &record.checkpoint;
        Self {
            protocol_version: protocol,
            kind: ArchiveKind::Archive,
            project_id: v.project_id,
            review_stream_id: v.stream_id,
            archive_id: v.archive_id,
            created_at_ms: v.created_at_ms,
            before_ref: v.before,
            result_snapshot_id: record.result_snapshot_id,
            groups: v
                .groups
                .iter()
                .map(|g| Group {
                    usage_basis: match &g.basis {
                        ArchiveBasis::Unknown => None,
                        ArchiveBasis::Known { snapshot, source } => Some(Basis {
                            snapshot: *snapshot,
                            source: match source {
                                ArchiveBasisSource::UserSelected => BasisSource::UserSelected {},
                                ArchiveBasisSource::AgentDeclared { usage_id } => {
                                    BasisSource::AgentDeclared {
                                        usage_id: *usage_id,
                                    }
                                }
                            },
                        }),
                    },
                    targets: g.targets.clone(),
                })
                .collect(),
            removed: v.removed.clone(),
            retained: v
                .retained
                .iter()
                .map(|r| Retention {
                    basis: r.basis,
                    current: r.current.as_ref().map(Into::into),
                    disposition: match r.disposition {
                        ArchiveDisposition::RemoveCurrent => Disposition::RemoveCurrent,
                        ArchiveDisposition::RetainLaterEdit => Disposition::RetainLaterEdit,
                        ArchiveDisposition::AlreadyAbsent => Disposition::AlreadyAbsent,
                    },
                })
                .collect(),
        }
    }

    pub fn into_record(self) -> ReviewArchiveRecord {
        ReviewArchiveRecord {
            result_snapshot_id: self.result_snapshot_id,
            checkpoint: ArchiveCheckpoint {
                project_id: self.project_id,
                stream_id: self.review_stream_id,
                archive_id: self.archive_id,
                created_at_ms: self.created_at_ms,
                before: self.before_ref,
                groups: self
                    .groups
                    .into_iter()
                    .map(|g| ArchiveGroup {
                        basis: match g.usage_basis {
                            None => ArchiveBasis::Unknown,
                            Some(b) => ArchiveBasis::Known {
                                snapshot: b.snapshot,
                                source: match b.source {
                                    BasisSource::UserSelected {} => {
                                        ArchiveBasisSource::UserSelected
                                    }
                                    BasisSource::AgentDeclared { usage_id } => {
                                        ArchiveBasisSource::AgentDeclared { usage_id }
                                    }
                                },
                            },
                        },
                        targets: g.targets,
                    })
                    .collect(),
                removed: self.removed,
                retained: self
                    .retained
                    .into_iter()
                    .map(|r| ArchiveRetention {
                        basis: r.basis,
                        current: r.current.map(Into::into),
                        disposition: match r.disposition {
                            Disposition::RemoveCurrent => ArchiveDisposition::RemoveCurrent,
                            Disposition::RetainLaterEdit => ArchiveDisposition::RetainLaterEdit,
                            Disposition::AlreadyAbsent => ArchiveDisposition::AlreadyAbsent,
                        },
                    })
                    .collect(),
            },
        }
    }
}
