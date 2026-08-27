use super::*;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Asset {
    #[serde(deserialize_with = "canonical_id")]
    asset_version_id: AssetVersionId,
    #[serde(deserialize_with = "optional_id")]
    source_entity_id: Option<EntityId>,
    relative_path: RelativePath,
    evidence: AssetProof,
    media: Media,
    #[serde(deserialize_with = "required_option")]
    producer_asset_id: Option<String>,
    #[serde(deserialize_with = "optional_id")]
    parent_asset_version_id: Option<AssetVersionId>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetProof {
    size_bytes: u64,
    modified_ns: String,
    #[serde(deserialize_with = "required_option")]
    blake3: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Media {
    Image {
        #[serde(deserialize_with = "required_option")]
        width: Option<u32>,
        #[serde(deserialize_with = "required_option")]
        height: Option<u32>,
    },
    Video {
        #[serde(deserialize_with = "required_option")]
        duration_us: Option<u64>,
        #[serde(deserialize_with = "required_option")]
        display_width: Option<u32>,
        #[serde(deserialize_with = "required_option")]
        display_height: Option<u32>,
    },
}
impl From<&ReviewMedia> for Media {
    fn from(v: &ReviewMedia) -> Self {
        match *v {
            ReviewMedia::Image { width, height } => Self::Image { width, height },
            ReviewMedia::Video {
                duration_us,
                display_width,
                display_height,
            } => Self::Video {
                duration_us,
                display_width,
                display_height,
            },
        }
    }
}
impl From<Media> for ReviewMedia {
    fn from(v: Media) -> Self {
        match v {
            Media::Image { width, height } => Self::Image { width, height },
            Media::Video {
                duration_us,
                display_width,
                display_height,
            } => Self::Video {
                duration_us,
                display_width,
                display_height,
            },
        }
    }
}
impl From<&AssetVersion> for Asset {
    fn from(v: &AssetVersion) -> Self {
        Self {
            asset_version_id: v.id,
            source_entity_id: v.source_entity_id,
            relative_path: v.relative_path.clone(),
            evidence: AssetProof {
                size_bytes: v.evidence.size_bytes,
                modified_ns: v.evidence.modified_ns.to_string(),
                blake3: v.evidence.blake3.map(encode_digest),
            },
            media: (&v.media).into(),
            producer_asset_id: v
                .producer_asset_id
                .as_ref()
                .map(|id| id.as_str().to_owned()),
            parent_asset_version_id: v.parent_asset_version_id,
        }
    }
}
impl TryFrom<Asset> for AssetVersion {
    type Error = ReviewProtocolError;
    fn try_from(v: Asset) -> Result<Self, Self::Error> {
        let modified_ns: i128 = v
            .evidence
            .modified_ns
            .parse()
            .map_err(|_| ReviewProtocolError::InvalidData)?;
        if modified_ns.to_string() != v.evidence.modified_ns {
            return Err(ReviewProtocolError::InvalidData);
        }
        Ok(Self {
            id: v.asset_version_id,
            source_entity_id: v.source_entity_id,
            relative_path: v.relative_path,
            evidence: AssetEvidence {
                size_bytes: v.evidence.size_bytes,
                modified_ns,
                blake3: v.evidence.blake3.as_deref().map(parse_digest).transpose()?,
            },
            media: v.media.into(),
            producer_asset_id: v
                .producer_asset_id
                .as_deref()
                .map(ProductionId::parse)
                .transpose()
                .map_err(|_| ReviewProtocolError::InvalidData)?,
            parent_asset_version_id: v.parent_asset_version_id,
        })
    }
}

vector_adapter!(assets, AssetVersion, Asset);
