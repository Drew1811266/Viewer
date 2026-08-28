use super::{ReviewWire, remote_output, remote_wire, wire};
use serde::{Deserialize, Serialize};
use viewer_application::{review_evidence::*, review_workspace::*};
use viewer_domain::{
    review::{AssetVersion, continuous::*},
    *,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedReviewAssetDto {
    pub asset: ReviewWire<AssetVersion>,
    pub preview: Option<ReviewEvidenceImageDto>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewEvidenceImageDto {
    #[serde(with = "wire")]
    pub asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    pub role: EvidenceRole,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub source_width: u32,
    pub source_height: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "EvidenceRole", rename_all = "snake_case")]
enum Role {
    Base,
    Annotated,
}
remote_wire!(EvidenceRole, Role);
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Selector {
    Snapshot {
        #[serde(with = "wire")]
        snapshot: SnapshotRef,
    },
    Archive {
        #[serde(with = "wire")]
        archive_id: ReviewArchiveId,
    },
    Legacy {
        #[serde(with = "wire")]
        round_id: ReviewRoundId,
    },
}
impl wire::WireValue for HistorySelector {
    fn encode<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Snapshot(snapshot) => Selector::Snapshot {
                snapshot: *snapshot,
            },
            Self::Archive(archive_id) => Selector::Archive {
                archive_id: *archive_id,
            },
            Self::Legacy(round_id) => Selector::Legacy {
                round_id: *round_id,
            },
        }
        .serialize(s)
    }
    fn decode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match Selector::deserialize(d)? {
            Selector::Snapshot { snapshot } => Self::Snapshot(snapshot),
            Selector::Archive { archive_id } => Self::Archive(archive_id),
            Selector::Legacy { round_id } => Self::Legacy(round_id),
        })
    }
}
#[derive(Serialize)]
#[serde(remote = "EvidenceRef", rename_all = "camelCase")]
struct Evidence {
    #[serde(with = "wire")]
    blake3: [u8; 32],
    #[serde(with = "wire")]
    size_bytes: u64,
    width: u32,
    height: u32,
}
remote_output!(EvidenceRef, Evidence);
#[derive(Serialize)]
#[serde(remote = "ReviewEvidenceBinding", rename_all = "camelCase")]
struct Binding {
    #[serde(with = "wire")]
    asset_version_id: AssetVersionId,
    #[serde(with = "wire")]
    capability: EvidenceCapability,
}
remote_output!(ReviewEvidenceBinding, Binding);
#[derive(Serialize)]
#[serde(remote = "EvidenceCapability", tag = "kind", rename_all = "snake_case")]
enum Capability {
    Image {
        #[serde(with = "wire")]
        base: EvidenceRef,
        #[serde(with = "wire")]
        annotated: Option<EvidenceRef>,
        #[serde(with = "wire")]
        annotations: Vec<EvidenceAnnotation>,
    },
    LegacyAbsent {},
    NotImage {},
}
remote_output!(EvidenceCapability, Capability);
#[derive(Serialize)]
#[serde(remote = "EvidenceAnnotation", rename_all = "camelCase")]
struct Annotation {
    ordinal: u32,
    #[serde(with = "wire")]
    key: TargetVersionKey,
}
remote_output!(EvidenceAnnotation, Annotation);
