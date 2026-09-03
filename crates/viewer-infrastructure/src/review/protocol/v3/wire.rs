//! Private DTO adapters: Domain types remain independent of JSON representation.
use super::super::common::{ReviewProtocolError, encode_digest, parse_digest};
use super::{EvidenceBinding, ReviewStateRecord};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use viewer_domain::review::continuous::*;
use viewer_domain::review::*;
use viewer_domain::*;

fn parse_id<T>(text: &str) -> Result<T, ReviewProtocolError>
where
    T: std::str::FromStr + std::fmt::Display,
{
    let id: T = text.parse().map_err(|_| ReviewProtocolError::InvalidData)?;
    if !id.to_string().eq_ignore_ascii_case(text) {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(id)
}
pub(super) fn canonical_id<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: std::str::FromStr + std::fmt::Display,
{
    parse_id(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}
pub(super) fn optional_id<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: std::str::FromStr + std::fmt::Display,
{
    Option::<String>::deserialize(deserializer)?
        .as_deref()
        .map(parse_id)
        .transpose()
        .map_err(serde::de::Error::custom)
}
pub(super) fn id_list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: std::str::FromStr + std::fmt::Display,
{
    Vec::<String>::deserialize(deserializer)?
        .iter()
        .map(|value| parse_id(value).map_err(serde::de::Error::custom))
        .collect()
}

pub(super) fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

macro_rules! adapter {
    ($name:ident, $domain:ty, $wire:ty) => {
        pub(in crate::review::protocol::v3) mod $name {
            use super::*;
            pub fn serialize<S: Serializer>(
                value: &$domain,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                <$wire>::from(value).serialize(serializer)
            }
            pub fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<$domain, D::Error> {
                <$domain>::try_from(<$wire>::deserialize(deserializer)?)
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

pub(super) mod digest {
    use super::*;
    pub fn serialize<S: Serializer>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        encode_digest(*value).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        parse_digest(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Snapshot {
    #[serde(deserialize_with = "canonical_id")]
    snapshot_id: ReviewSnapshotId,
    #[serde(with = "digest")]
    blake3: [u8; 32],
}
impl From<&SnapshotRef> for Snapshot {
    fn from(v: &SnapshotRef) -> Self {
        Self {
            snapshot_id: v.snapshot_id,
            blake3: v.blake3,
        }
    }
}
impl From<Snapshot> for SnapshotRef {
    fn from(v: Snapshot) -> Self {
        Self {
            snapshot_id: v.snapshot_id,
            blake3: v.blake3,
        }
    }
}
adapter!(snapshot, SnapshotRef, Snapshot);

pub(super) mod optional_snapshot {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &Option<SnapshotRef>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_ref().map(Snapshot::from).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<SnapshotRef>, D::Error> {
        Ok(Option::<Snapshot>::deserialize(deserializer)?.map(Into::into))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Key {
    #[serde(deserialize_with = "canonical_id")]
    feedback_id: FeedbackId,
    #[serde(deserialize_with = "canonical_id")]
    text_revision_id: ReviewTextRevisionId,
    #[serde(deserialize_with = "canonical_id")]
    target_id: ReviewTargetId,
    #[serde(deserialize_with = "canonical_id")]
    target_revision_id: ReviewTargetRevisionId,
}
impl From<&TargetVersionKey> for Key {
    fn from(v: &TargetVersionKey) -> Self {
        Self {
            feedback_id: v.feedback_id,
            text_revision_id: v.text_revision_id,
            target_id: v.target_id,
            target_revision_id: v.target_revision_id,
        }
    }
}
impl From<Key> for TargetVersionKey {
    fn from(v: Key) -> Self {
        Self {
            feedback_id: v.feedback_id,
            text_revision_id: v.text_revision_id,
            target_id: v.target_id,
            target_revision_id: v.target_revision_id,
        }
    }
}
adapter!(key, TargetVersionKey, Key);

pub(super) mod keys {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &[TargetVersionKey],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value
            .iter()
            .map(Key::from)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<TargetVersionKey>, D::Error> {
        Ok(Vec::<Key>::deserialize(deserializer)?
            .into_iter()
            .map(Into::into)
            .collect())
    }
}

macro_rules! vector_adapter {
    ($name:ident, $domain:ty, $wire:ty) => {
        pub(in crate::review::protocol::v3) mod $name {
            use super::*;
            pub fn serialize<S: Serializer>(
                value: &[$domain],
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeSeq;
                let mut sequence = serializer.serialize_seq(Some(value.len()))?;
                for item in value {
                    sequence.serialize_element(&<$wire>::from(item))?;
                }
                sequence.end()
            }
            pub fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Vec<$domain>, D::Error> {
                Vec::<$wire>::deserialize(deserializer)?
                    .into_iter()
                    .map(|v| <$domain>::try_from(v).map_err(serde::de::Error::custom))
                    .collect()
            }
        }
    };
}
mod asset;
mod change;
mod feedback_types;
mod legacy;
mod state;

pub(super) use asset::assets;
pub(super) use change::{changes, deltas};
use feedback_types::Anchor;
adapter!(anchor, FeedbackAnchor, Anchor);
pub(super) use feedback_types::{feedback, histories, optional_history, targets};
pub(super) use legacy::legacy_feedback;
pub(super) use state::State;
