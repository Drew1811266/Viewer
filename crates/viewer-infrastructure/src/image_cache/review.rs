use super::*;
use viewer_application::review_evidence::{BoundReviewImage, EvidenceRole};
use viewer_domain::AssetVersionId;

const MAX_REVIEW_TOKENS: usize = viewer_application::review_workspace::MAX_REVIEW_PREVIEW_BATCH;
const MAX_RETAINED_REVIEW_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct Registration {
    asset: AssetVersionId,
    digest: [u8; 32],
    role: EvidenceRole,
    sequence: u64,
}

/// Cache capabilities, not persistent evidence. A bounded session cache expires its oldest
/// registrations; callers can re-request a committed selector or prepare a fresh preview.
/// No eviction modifies a repository object, source file, or another media cache.
pub fn register_review_png(
    registry: &ImageArtifactRegistry,
    session_id: SessionId,
    entity_id: EntityId,
    image: &BoundReviewImage,
) -> Result<ImageArtifactToken, ImageArtifactRegistryError> {
    if blake3::hash(image.png()).as_bytes() != &image.blake3() {
        return Err(ImageArtifactRegistryError::UnverifiedReviewImage);
    }
    let mut entries = registry.entries.write().unwrap_or_else(|e| e.into_inner());
    let sequence = entries
        .values()
        .filter_map(|e| e.review.as_ref())
        .map(|r| r.sequence)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            ImageArtifactRegistryError::ArtifactUnavailable("review cache limit".into())
        })?;
    if let Some((token, entry)) = entries.iter_mut().find(|(_, e)| {
        e.session_id == session_id
            && e.artifact.entity_id == entity_id
            && e.review.as_ref().is_some_and(|r| {
                r.asset == image.asset().id && r.digest == image.blake3() && r.role == image.role()
            })
    }) {
        entry
            .review
            .as_mut()
            .expect("matched review entry")
            .sequence = sequence;
        return Ok(token.clone());
    }
    let mut retained = 0_usize;
    let mut count = 0_usize;
    for entry in entries
        .values()
        .filter(|e| e.session_id == session_id && e.review.is_some())
    {
        retained += entry.artifact.immutable_bytes().map_or(0, |b| b.len());
        count += 1;
    }
    while count >= MAX_REVIEW_TOKENS
        || retained.saturating_add(image.png().len()) > MAX_RETAINED_REVIEW_BYTES
    {
        let oldest = entries
            .iter()
            .filter(|(_, e)| e.session_id == session_id && e.review.is_some())
            .min_by_key(|(_, e)| e.review.as_ref().expect("filtered entry").sequence)
            .map(|(t, _)| t.clone())
            .ok_or_else(|| {
                ImageArtifactRegistryError::ArtifactUnavailable("review cache limit".into())
            })?;
        let entry = entries.remove(&oldest).expect("selected cache entry");
        retained -= entry.artifact.immutable_bytes().map_or(0, |b| b.len());
        count -= 1;
    }
    let token = loop {
        let token = random_token()?;
        if !entries.contains_key(&token) {
            break token;
        }
    };
    entries.insert(
        token.clone(),
        RegistryEntry {
            session_id,
            artifact: RegisteredImageArtifact {
                entity_id,
                cache_path: PathBuf::new(),
                mime: "image/png".into(),
                immutable_bytes: Some(Arc::from(image.png())),
                generation: None,
                video_request_id: None,
            },
            review: Some(Registration {
                asset: image.asset().id,
                digest: image.blake3(),
                role: image.role(),
                sequence,
            }),
        },
    );
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_application::review_workspace::EvidenceRef;
    use viewer_domain::review::{AssetEvidence, AssetVersion, ReviewMedia};

    fn image(id: u128, size: usize) -> BoundReviewImage {
        let mut png = std::fs::read(viewer_test_support::image_fixtures::image_fixture(
            "alpha.png",
        ))
        .unwrap();
        png.resize(size.max(png.len()), 0);
        let digest = *blake3::hash(&png).as_bytes();
        let asset = AssetVersion {
            id: AssetVersionId::from_u128(id),
            source_entity_id: None,
            relative_path: RelativePath::parse("image.png").unwrap(),
            evidence: AssetEvidence {
                size_bytes: png.len() as u64,
                modified_ns: 1,
                blake3: Some(digest),
            },
            media: ReviewMedia::Image {
                width: Some(640),
                height: Some(480),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        };
        BoundReviewImage::from_verified_png(
            asset,
            EvidenceRef {
                blake3: digest,
                size_bytes: png.len() as u64,
                width: 640,
                height: 480,
            },
            EvidenceRole::Base,
            png,
        )
        .unwrap()
    }

    #[test]
    fn review_workspace_cache_byte_budget_does_not_evict_another_session() {
        let registry = ImageArtifactRegistry::default();
        let session = SessionId::from_u128(1);
        let other = SessionId::from_u128(2);
        let entity = EntityId::from_u128(1);
        let other_token = register_review_png(&registry, other, entity, &image(1, 0)).unwrap();
        let first =
            register_review_png(&registry, session, entity, &image(10, 24 * 1024 * 1024)).unwrap();
        let second =
            register_review_png(&registry, session, entity, &image(11, 24 * 1024 * 1024)).unwrap();
        let third =
            register_review_png(&registry, session, entity, &image(12, 24 * 1024 * 1024)).unwrap();
        assert!(registry.resolve(session, &first).is_none());
        assert!(registry.resolve(session, &second).is_some());
        assert!(registry.resolve(session, &third).is_some());
        assert!(registry.resolve(other, &other_token).is_some());
    }

    #[test]
    fn review_workspace_registration_rechecks_the_bound_digest() {
        let valid = image(1, 0);
        let mut reference = valid.reference().clone();
        reference.blake3 = [0; 32];
        let forged = BoundReviewImage::from_verified_png(
            valid.asset().clone(),
            reference,
            EvidenceRole::Base,
            valid.png().to_vec(),
        )
        .unwrap();
        assert_eq!(
            register_review_png(
                &ImageArtifactRegistry::default(),
                SessionId::from_u128(1),
                EntityId::from_u128(1),
                &forged
            ),
            Err(ImageArtifactRegistryError::UnverifiedReviewImage)
        );
    }
}
