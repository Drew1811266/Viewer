use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    path::{Path, PathBuf},
    sync::RwLock,
};
use viewer_domain::{EntityId, ProjectId, RelativePath, SessionId, image::ImageRepresentationKind};

const CACHE_KEY_SCHEMA: &[u8] = b"viewer-image-cache-key\0v1";

#[derive(Clone, Copy)]
pub struct ImageCacheKeyInput<'a> {
    pub project_id: ProjectId,
    pub relative_path: &'a RelativePath,
    pub source_size: u64,
    pub source_mtime_ns: i128,
    pub kind: ImageRepresentationKind,
    pub renderer_version: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImageCacheKey([u8; 32]);

impl ImageCacheKey {
    pub fn from_request(input: &ImageCacheKeyInput<'_>) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CACHE_KEY_SCHEMA);
        update_sized(&mut hasher, input.project_id.to_string().as_bytes());
        update_sized(&mut hasher, input.relative_path.as_str().as_bytes());
        hasher.update(&input.source_size.to_le_bytes());
        hasher.update(&input.source_mtime_ns.to_le_bytes());
        update_representation(&mut hasher, input.kind);
        hasher.update(&input.renderer_version.to_le_bytes());
        Self(*hasher.finalize().as_bytes())
    }

    pub fn to_hex(self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

fn update_sized(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn update_representation(hasher: &mut blake3::Hasher, kind: ImageRepresentationKind) {
    match kind {
        ImageRepresentationKind::Thumbnail {
            max_pixels,
            scale_milli,
        } => {
            hasher.update(&[0]);
            hasher.update(&max_pixels.to_le_bytes());
            hasher.update(&scale_milli.to_le_bytes());
        }
        ImageRepresentationKind::FitPreview {
            max_width,
            max_height,
            scale_milli,
        } => {
            hasher.update(&[1]);
            hasher.update(&max_width.to_le_bytes());
            hasher.update(&max_height.to_le_bytes());
            hasher.update(&scale_milli.to_le_bytes());
        }
        ImageRepresentationKind::Original100Percent => {
            hasher.update(&[2]);
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ImageArtifactToken(String);

impl ImageArtifactToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(value: &str) -> Option<Self> {
        let valid = value.len() == 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        valid.then(|| Self(value.to_owned()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredImageArtifact {
    entity_id: EntityId,
    cache_path: PathBuf,
    mime: String,
}

impl RegisteredImageArtifact {
    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    pub fn cache_path(&self) -> &Path {
        &self.cache_path
    }

    pub fn mime(&self) -> &str {
        &self.mime
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageArtifactLookup {
    Found(RegisteredImageArtifact),
    WrongSession,
    NotFound,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum ImageArtifactRegistryError {
    #[error("image artifact is unavailable: {0}")]
    ArtifactUnavailable(String),
    #[error("image artifact path is not a regular file")]
    NotAFile,
    #[error("system random source is unavailable: {0}")]
    RandomSourceUnavailable(String),
}

struct RegistryEntry {
    session_id: SessionId,
    artifact: RegisteredImageArtifact,
}

#[derive(Default)]
pub struct ImageArtifactRegistry {
    entries: RwLock<HashMap<ImageArtifactToken, RegistryEntry>>,
}

impl ImageArtifactRegistry {
    pub fn insert(
        &self,
        session_id: SessionId,
        entity_id: EntityId,
        cache_path: impl Into<PathBuf>,
        mime: impl Into<String>,
    ) -> Result<ImageArtifactToken, ImageArtifactRegistryError> {
        let cache_path = cache_path.into();
        let metadata = fs::metadata(&cache_path)
            .map_err(|error| ImageArtifactRegistryError::ArtifactUnavailable(error.to_string()))?;
        if !metadata.is_file() {
            return Err(ImageArtifactRegistryError::NotAFile);
        }
        let artifact = RegisteredImageArtifact {
            entity_id,
            cache_path,
            mime: mime.into(),
        };
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            let token = random_token()?;
            if let Entry::Vacant(entry) = entries.entry(token.clone()) {
                entry.insert(RegistryEntry {
                    session_id,
                    artifact,
                });
                return Ok(token);
            }
        }
    }

    pub fn resolve(
        &self,
        session_id: SessionId,
        token: &ImageArtifactToken,
    ) -> Option<RegisteredImageArtifact> {
        match self.lookup(session_id, token.as_str()) {
            ImageArtifactLookup::Found(artifact) => Some(artifact),
            ImageArtifactLookup::WrongSession | ImageArtifactLookup::NotFound => None,
        }
    }

    pub fn lookup(&self, session_id: SessionId, token: &str) -> ImageArtifactLookup {
        let Some(token) = ImageArtifactToken::parse(token) else {
            return ImageArtifactLookup::NotFound;
        };
        let entries = self
            .entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(entry) = entries.get(&token) else {
            return ImageArtifactLookup::NotFound;
        };
        if entry.session_id != session_id {
            return ImageArtifactLookup::WrongSession;
        }
        ImageArtifactLookup::Found(entry.artifact.clone())
    }

    pub fn remove_session(&self, session_id: SessionId) -> usize {
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous_len = entries.len();
        entries.retain(|_, entry| entry.session_id != session_id);
        previous_len - entries.len()
    }
}

fn random_token() -> Result<ImageArtifactToken, ImageArtifactRegistryError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random)
        .map_err(|error| ImageArtifactRegistryError::RandomSourceUnavailable(error.to_string()))?;
    let mut encoded = String::with_capacity(32);
    for byte in random {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(ImageArtifactToken(encoded))
}

#[cfg(test)]
mod tests {
    use super::{ImageArtifactLookup, ImageArtifactRegistry, ImageCacheKey, ImageCacheKeyInput};
    use std::str::FromStr;
    use viewer_domain::{
        EntityId, ProjectId, RelativePath, SessionId, image::ImageRepresentationKind,
    };
    use viewer_test_support::image_fixtures::image_fixture;

    fn thumbnail_kind(max_pixels: u32) -> ImageRepresentationKind {
        ImageRepresentationKind::Thumbnail {
            max_pixels,
            scale_milli: 2_000,
        }
    }

    #[test]
    fn source_change_changes_cache_key() {
        let project_id = ProjectId::new();
        let relative_path = RelativePath::parse("products/sku/image.jpg").unwrap();
        let first = ImageCacheKeyInput {
            project_id,
            relative_path: &relative_path,
            source_size: 100,
            source_mtime_ns: 1_000,
            kind: thumbnail_kind(256),
            renderer_version: 1,
        };
        let changed = ImageCacheKeyInput {
            source_size: 101,
            source_mtime_ns: 1_001,
            ..first
        };
        assert_ne!(
            ImageCacheKey::from_request(&first),
            ImageCacheKey::from_request(&changed)
        );
    }

    #[test]
    fn representation_change_changes_cache_key() {
        let project_id = ProjectId::new();
        let relative_path = RelativePath::parse("products/sku/image.jpg").unwrap();
        let first = ImageCacheKeyInput {
            project_id,
            relative_path: &relative_path,
            source_size: 100,
            source_mtime_ns: 1_000,
            kind: thumbnail_kind(256),
            renderer_version: 1,
        };
        let changed = ImageCacheKeyInput {
            kind: thumbnail_kind(512),
            ..first
        };
        assert_ne!(
            ImageCacheKey::from_request(&first),
            ImageCacheKey::from_request(&changed)
        );
    }

    #[test]
    fn cache_key_serialization_is_stable() {
        let project_id = ProjectId::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        let relative_path = RelativePath::parse("products/sku/image.jpg").unwrap();
        let input = ImageCacheKeyInput {
            project_id,
            relative_path: &relative_path,
            source_size: 10_485_760,
            source_mtime_ns: 1_723_456_789_123_456_789,
            kind: ImageRepresentationKind::FitPreview {
                max_width: 2_560,
                max_height: 1_600,
                scale_milli: 2_000,
            },
            renderer_version: 1,
        };
        assert_eq!(
            ImageCacheKey::from_request(&input).to_hex(),
            "ae2e9435edf543bdc0506edd30823847eeb84560371123711c4b1e66315b7702"
        );
    }

    #[test]
    fn registry_cannot_resolve_artifact_from_another_session() {
        let registry = ImageArtifactRegistry::default();
        let owner = SessionId::new();
        let other = SessionId::new();
        let token = registry
            .insert(
                owner,
                EntityId::new(),
                image_fixture("srgb.jpg"),
                "image/jpeg",
            )
            .unwrap();
        assert_eq!(
            registry.lookup(other, token.as_str()),
            ImageArtifactLookup::WrongSession
        );
        assert!(registry.resolve(other, &token).is_none());
    }

    #[test]
    fn closing_session_removes_all_registry_entries() {
        let registry = ImageArtifactRegistry::default();
        let session = SessionId::new();
        let token = registry
            .insert(
                session,
                EntityId::new(),
                image_fixture("srgb.jpg"),
                "image/jpeg",
            )
            .unwrap();
        assert_eq!(registry.remove_session(session), 1);
        assert!(registry.resolve(session, &token).is_none());
    }

    #[test]
    fn registry_tokens_are_random_128_bit_url_safe_values() {
        let registry = ImageArtifactRegistry::default();
        let session = SessionId::new();
        let first = registry
            .insert(
                session,
                EntityId::new(),
                image_fixture("srgb.jpg"),
                "image/jpeg",
            )
            .unwrap();
        let second = registry
            .insert(
                session,
                EntityId::new(),
                image_fixture("p3.jpg"),
                "image/jpeg",
            )
            .unwrap();
        assert_ne!(first, second);
        assert_eq!(first.as_str().len(), 32);
        assert!(first.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}
