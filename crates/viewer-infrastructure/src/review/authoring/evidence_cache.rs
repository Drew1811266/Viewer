use super::SqliteContinuousReviewAuthoringStore;
use crate::review::continuous::verify_cached_evidence;
use rusqlite::{OptionalExtension, params};
use std::time::{SystemTime, UNIX_EPOCH};
use viewer_application::{
    MAX_REVIEW_ARTIFACT_BYTES, MAX_REVIEW_ARTIFACT_PIXELS,
    review_workspace::{
        CachedReviewEvidenceAction, EvidenceRef, ReviewCommitError, ReviewEvidenceActionCachePort,
        ReviewEvidenceActionKey,
    },
};

const ENCODED_REFERENCE_BYTES: usize = 48;

impl ReviewEvidenceActionCachePort for SqliteContinuousReviewAuthoringStore {
    fn load_verified(
        &self,
        key: ReviewEvidenceActionKey,
    ) -> Result<Option<CachedReviewEvidenceAction>, ReviewCommitError> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                "SELECT base_evidence, annotated_evidence, renderer_version,
                        output_policy_version
                 FROM review_evidence_action_cache WHERE action_key = ?1",
                [key.0.as_slice()],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(super::store::map_database_error)?;
        let Some((base, annotated, renderer, output)) = row else {
            return Ok(None);
        };
        let decoded = (|| {
            let base = decode_reference(&base)?;
            let annotated = match annotated.as_deref() {
                Some(bytes) => Some(decode_reference(bytes)?),
                None => None,
            };
            Some((
                base,
                annotated,
                parse_version(&renderer)?,
                parse_version(&output)?,
            ))
        })();
        let valid = decoded.as_ref().is_some_and(|(base, annotated, _, _)| {
            verify_cached_evidence(&self.project_root, base).is_ok()
                && annotated.as_ref().is_none_or(|reference| {
                    verify_cached_evidence(&self.project_root, reference).is_ok()
                })
        });
        if !valid {
            if !self.writable {
                return Err(ReviewCommitError::ReadOnly);
            }
            connection
                .execute(
                    "DELETE FROM review_evidence_action_cache WHERE action_key = ?1",
                    [key.0.as_slice()],
                )
                .map_err(super::store::map_database_error)?;
            return Ok(None);
        }
        let (base, annotated, renderer_version, output_policy_version) =
            decoded.ok_or(ReviewCommitError::Integrity)?;
        Ok(Some(CachedReviewEvidenceAction {
            action_key: key,
            base,
            annotated,
            renderer_version,
            output_policy_version,
        }))
    }

    fn store_verified(&self, value: &CachedReviewEvidenceAction) -> Result<(), ReviewCommitError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly);
        }
        if value.renderer_version == 0 || value.output_policy_version == 0 {
            return Err(ReviewCommitError::Integrity);
        }
        verify_cached_evidence(&self.project_root, &value.base)?;
        if let Some(annotated) = &value.annotated {
            verify_cached_evidence(&self.project_root, annotated)?;
        }
        let base = encode_reference(&value.base)?;
        let annotated = value.annotated.as_ref().map(encode_reference).transpose()?;
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO review_evidence_action_cache(
                    action_key, base_evidence, annotated_evidence, renderer_version,
                    output_policy_version, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(action_key) DO UPDATE SET
                    base_evidence = excluded.base_evidence,
                    annotated_evidence = excluded.annotated_evidence,
                    renderer_version = excluded.renderer_version,
                    output_policy_version = excluded.output_policy_version,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    value.action_key.0.as_slice(),
                    base,
                    annotated,
                    value.renderer_version.to_string(),
                    value.output_policy_version.to_string(),
                    now_ms(),
                ],
            )
            .map_err(super::store::map_database_error)?;
        Ok(())
    }

    fn remove(&self, key: ReviewEvidenceActionKey) -> Result<(), ReviewCommitError> {
        if !self.writable {
            return Err(ReviewCommitError::ReadOnly);
        }
        self.connection()?
            .execute(
                "DELETE FROM review_evidence_action_cache WHERE action_key = ?1",
                [key.0.as_slice()],
            )
            .map_err(super::store::map_database_error)?;
        Ok(())
    }
}

fn encode_reference(reference: &EvidenceRef) -> Result<Vec<u8>, ReviewCommitError> {
    validate_reference(reference)?;
    let mut bytes = Vec::with_capacity(ENCODED_REFERENCE_BYTES);
    bytes.extend_from_slice(&reference.blake3);
    bytes.extend_from_slice(&reference.size_bytes.to_be_bytes());
    bytes.extend_from_slice(&reference.width.to_be_bytes());
    bytes.extend_from_slice(&reference.height.to_be_bytes());
    Ok(bytes)
}

fn decode_reference(bytes: &[u8]) -> Option<EvidenceRef> {
    if bytes.len() != ENCODED_REFERENCE_BYTES {
        return None;
    }
    let reference = EvidenceRef {
        blake3: bytes[0..32].try_into().ok()?,
        size_bytes: u64::from_be_bytes(bytes[32..40].try_into().ok()?),
        width: u32::from_be_bytes(bytes[40..44].try_into().ok()?),
        height: u32::from_be_bytes(bytes[44..48].try_into().ok()?),
    };
    validate_reference(&reference).ok()?;
    Some(reference)
}

fn validate_reference(reference: &EvidenceRef) -> Result<(), ReviewCommitError> {
    if reference.size_bytes == 0
        || reference.size_bytes > MAX_REVIEW_ARTIFACT_BYTES
        || reference.width == 0
        || reference.height == 0
        || u64::from(reference.width) * u64::from(reference.height) > MAX_REVIEW_ARTIFACT_PIXELS
    {
        return Err(ReviewCommitError::Integrity);
    }
    Ok(())
}

fn parse_version(value: &str) -> Option<u32> {
    let parsed = value.parse::<u32>().ok().filter(|value| *value > 0)?;
    (parsed.to_string() == value).then_some(parsed)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_millis()).ok())
        .unwrap_or(i64::MAX)
}
