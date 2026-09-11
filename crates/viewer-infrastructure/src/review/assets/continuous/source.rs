use super::*;
use std::io;

/// Result of one source check. On `Match`, `current_entity` is the verified
/// current session entity identity of the source file (see `check_file`).
#[derive(Debug, Eq, PartialEq)]
pub(super) struct SourceCheckOutcome {
    pub status: SourceCheckStatus,
    pub current_entity: Option<EntityId>,
}

impl SourceCheckOutcome {
    pub(super) fn unchanged(status: SourceCheckStatus) -> Self {
        Self {
            status,
            current_entity: None,
        }
    }
}

pub(super) async fn check(
    root: PathBuf,
    locator: ReviewSourceLocator,
    asset: AssetVersion,
    cancellation: ReviewTaskCancellation,
) -> Result<SourceCheckOutcome, ReviewAssetError> {
    tokio::task::spawn_blocking(move || check_file(&root, &locator, &asset, &cancellation, || {}))
        .await
        .map_err(|_| ReviewAssetError::Unavailable)?
}

fn check_file(
    root: &Path,
    locator: &ReviewSourceLocator,
    asset: &AssetVersion,
    cancellation: &ReviewTaskCancellation,
    after_hash: impl FnOnce(),
) -> Result<SourceCheckOutcome, ReviewAssetError> {
    if cancellation.is_cancelled() {
        return Err(ReviewAssetError::Cancelled);
    }
    let Some(expected) = asset.evidence.blake3 else {
        return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Unverified));
    };
    let path = locator.relative_path.as_str();
    if path.contains('\\') || path.as_bytes().get(1) == Some(&b':') {
        return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Unverified));
    }
    let mut file = match open_owned_source(root, &locator.relative_path) {
        Ok(file) => file,
        Err(error) => return Ok(SourceCheckOutcome::unchanged(io_status(error))),
    };
    let before = match file.metadata() {
        Ok(m) => m,
        Err(_) => return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Unreadable)),
    };
    if !before.is_file() {
        return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Unreadable));
    }
    // Device/inode identity is session-scoped (ADR 0003): macOS reassigns APFS
    // volume device ids across reboots, so an identity mismatch alone must not
    // fail the check. The content digest below is the durable identity; the
    // verified current identity is returned so callers can rebind.
    let current_entity = entity_id(&before, &locator.relative_path);
    if before.len() != asset.evidence.size_bytes {
        return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Changed));
    }
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        if cancellation.is_cancelled() {
            return Err(ReviewAssetError::Cancelled);
        }
        let count = match file.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => {
                return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Unreadable));
            }
        };
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > before.len() {
            return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Changed));
        }
        hasher.update(&buffer[..count]);
    }
    after_hash();
    if cancellation.is_cancelled() {
        return Err(ReviewAssetError::Cancelled);
    }
    let after = match file.metadata() {
        Ok(m) => m,
        Err(_) => return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Unreadable)),
    };
    let located = match open_owned_source(root, &locator.relative_path).and_then(|f| f.metadata()) {
        Ok(m) => m,
        Err(error) => return Ok(SourceCheckOutcome::unchanged(io_status(error))),
    };
    if total != before.len()
        || !same_metadata(&before, &after)
        || !same_metadata(&after, &located)
        || *hasher.finalize().as_bytes() != expected
    {
        return Ok(SourceCheckOutcome::unchanged(SourceCheckStatus::Changed));
    }
    Ok(SourceCheckOutcome {
        status: SourceCheckStatus::Match,
        current_entity: Some(current_entity),
    })
}
fn io_status(error: io::Error) -> SourceCheckStatus {
    if error.kind() == io::ErrorKind::NotFound {
        SourceCheckStatus::Missing
    } else {
        SourceCheckStatus::Unreadable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(root: &Path) -> (ReviewSourceLocator, AssetVersion) {
        let path = root.join("a.png");
        fs::write(&path, b"original").unwrap();
        let metadata = fs::metadata(path).unwrap();
        let relative_path = RelativePath::parse("a.png").unwrap();
        let id = entity_id(&metadata, &relative_path);
        (
            ReviewSourceLocator {
                entity_id: id,
                relative_path: relative_path.clone(),
            },
            AssetVersion {
                id: AssetVersionId::new(),
                source_entity_id: Some(id),
                relative_path,
                evidence: AssetEvidence {
                    size_bytes: metadata.len(),
                    modified_ns: modified_ns(&metadata),
                    blake3: Some(*blake3::hash(b"original").as_bytes()),
                },
                media: ReviewMedia::Image {
                    width: Some(1),
                    height: Some(1),
                },
                producer_asset_id: None,
                parent_asset_version_id: None,
            },
        )
    }
    #[test]
    fn source_changed_after_hash_is_never_reported_as_match() {
        let root = tempfile::tempdir().unwrap();
        let (locator, asset) = source(root.path());
        let result = check_file(
            root.path(),
            &locator,
            &asset,
            &ReviewTaskCancellation::default(),
            || {
                fs::write(root.path().join("a.png"), b"modified").unwrap();
            },
        );
        assert_eq!(result.unwrap(), SourceCheckOutcome::unchanged(SourceCheckStatus::Changed));
    }
    #[test]
    fn cancellation_after_hash_cannot_return_a_stale_match() {
        let root = tempfile::tempdir().unwrap();
        let (locator, asset) = source(root.path());
        let cancellation = ReviewTaskCancellation::default();
        let result = check_file(root.path(), &locator, &asset, &cancellation, || {
            cancellation.cancel()
        });
        assert_eq!(result, Err(ReviewAssetError::Cancelled));
    }
    #[test]
    fn volume_device_reassignment_with_identical_content_matches_and_rebinds() {
        let root = tempfile::tempdir().unwrap();
        let (locator, asset) = source(root.path());
        // Simulate a reboot that reassigned the APFS volume device id: the
        // locator no longer matches the file's current derived identity while
        // the inode (and the content) stayed the same.
        let drifted_locator = ReviewSourceLocator {
            entity_id: EntityId::from_u128((u128::from(0x99_u32) << 64) | 0xDEAD),
            relative_path: locator.relative_path.clone(),
        };
        assert_ne!(drifted_locator.entity_id, locator.entity_id);
        let result = check_file(
            root.path(),
            &drifted_locator,
            &asset,
            &ReviewTaskCancellation::default(),
            || {},
        )
        .unwrap();
        assert_eq!(result.status, SourceCheckStatus::Match);
        assert_eq!(result.current_entity, Some(locator.entity_id));
    }
    #[test]
    fn identity_drift_with_modified_content_is_still_a_change() {
        let root = tempfile::tempdir().unwrap();
        let (locator, asset) = source(root.path());
        let drifted_locator = ReviewSourceLocator {
            entity_id: EntityId::from_u128((u128::from(0x99_u32) << 64) | 0xDEAD),
            relative_path: locator.relative_path.clone(),
        };
        fs::write(root.path().join("a.png"), b"replaced").unwrap();
        let result = check_file(
            root.path(),
            &drifted_locator,
            &asset,
            &ReviewTaskCancellation::default(),
            || {},
        )
        .unwrap();
        assert_eq!(result, SourceCheckOutcome::unchanged(SourceCheckStatus::Changed));
    }
}
