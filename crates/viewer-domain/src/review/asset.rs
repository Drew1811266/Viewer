use super::ReviewValueError;
use crate::{AssetVersionId, EntityId, RelativePath};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProductionId(String);

impl ProductionId {
    pub fn parse(value: &str) -> Result<Self, ReviewValueError> {
        let bytes = value.as_bytes();
        let valid = (1..=128).contains(&bytes.len())
            && bytes[0].is_ascii_alphanumeric()
            && bytes[1..].iter().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
            });

        valid
            .then(|| Self(value.to_owned()))
            .ok_or(ReviewValueError::InvalidFormat)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProductionScope {
    pub task_id: ProductionId,
    pub batch_id: ProductionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetEvidence {
    pub size_bytes: u64,
    pub modified_ns: i128,
    pub blake3: Option<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewAssetKind {
    Image,
    Video,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewMedia {
    Image {
        width: Option<u32>,
        height: Option<u32>,
    },
    Video {
        duration_us: Option<u64>,
        display_width: Option<u32>,
        display_height: Option<u32>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetVersion {
    pub id: AssetVersionId,
    pub source_entity_id: Option<EntityId>,
    pub relative_path: RelativePath,
    pub evidence: AssetEvidence,
    pub media: ReviewMedia,
    pub producer_asset_id: Option<ProductionId>,
    pub parent_asset_version_id: Option<AssetVersionId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_ids_are_bounded_portable_and_exact() {
        assert_eq!(
            ProductionId::parse("task:2026-08-25.alpha")
                .unwrap()
                .as_str(),
            "task:2026-08-25.alpha"
        );
        for invalid in ["", " contains-space", "任务一", "../task", "a/b"] {
            assert!(
                ProductionId::parse(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(ProductionId::parse(&"a".repeat(129)).is_err());
    }
}
