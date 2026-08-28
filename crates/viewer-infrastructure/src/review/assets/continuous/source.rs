use super::*;
use std::io;

pub(super) async fn check(
    root: PathBuf,
    locator: ReviewSourceLocator,
    asset: AssetVersion,
    cancellation: ReviewTaskCancellation,
) -> Result<SourceCheckStatus, ReviewAssetError> {
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
) -> Result<SourceCheckStatus, ReviewAssetError> {
    if cancellation.is_cancelled() {
        return Err(ReviewAssetError::Cancelled);
    }
    let Some(expected) = asset.evidence.blake3 else {
        return Ok(SourceCheckStatus::Unverified);
    };
    let path = locator.relative_path.as_str();
    if path.contains('\\') || path.as_bytes().get(1) == Some(&b':') {
        return Ok(SourceCheckStatus::Unverified);
    }
    let mut file = match open_owned_source(root, &locator.relative_path) {
        Ok(file) => file,
        Err(error) => return Ok(io_status(error)),
    };
    let before = match file.metadata() {
        Ok(m) => m,
        Err(_) => return Ok(SourceCheckStatus::Unreadable),
    };
    if !before.is_file() {
        return Ok(SourceCheckStatus::Unreadable);
    }
    if entity_id(&before, &locator.relative_path) != locator.entity_id
        || before.len() != asset.evidence.size_bytes
    {
        return Ok(SourceCheckStatus::Changed);
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
            Err(_) => return Ok(SourceCheckStatus::Unreadable),
        };
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > before.len() {
            return Ok(SourceCheckStatus::Changed);
        }
        hasher.update(&buffer[..count]);
    }
    after_hash();
    if cancellation.is_cancelled() {
        return Err(ReviewAssetError::Cancelled);
    }
    let after = match file.metadata() {
        Ok(m) => m,
        Err(_) => return Ok(SourceCheckStatus::Unreadable),
    };
    let located = match open_owned_source(root, &locator.relative_path).and_then(|f| f.metadata()) {
        Ok(m) => m,
        Err(error) => return Ok(io_status(error)),
    };
    if total != before.len()
        || !same_metadata(&before, &after)
        || !same_metadata(&after, &located)
        || *hasher.finalize().as_bytes() != expected
    {
        return Ok(SourceCheckStatus::Changed);
    }
    Ok(SourceCheckStatus::Match)
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
        assert_eq!(result.unwrap(), SourceCheckStatus::Changed);
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
}
