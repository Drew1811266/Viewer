use crate::review::continuous::owned_io::{Directory, same_contents};
use crate::review::v3::{ReadSourceCheck, ReadSourceStatus};
use std::fs::File;
use std::io::Read;
use viewer_domain::review::AssetVersion;

pub(super) fn check(root: &Directory, assets: &[AssetVersion]) -> Vec<ReadSourceCheck> {
    assets
        .iter()
        .map(|asset| ReadSourceCheck {
            asset_version_id: asset.id,
            status: check_file(root, asset, || {}),
            checked_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|t| t.as_millis().min(9_007_199_254_740_991) as i64)
                .unwrap_or(0),
        })
        .collect()
}

fn open_source(root: &Directory, path: &str) -> Result<(Vec<Directory>, File), ReadSourceStatus> {
    let mut directories: Vec<Directory> = vec![];
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        let parent = directories.last().unwrap_or(root);
        if parts.peek().is_none() {
            let file = parent
                .regular(part, false)
                .map_err(|_| ReadSourceStatus::Unreadable)?
                .ok_or(ReadSourceStatus::Missing)?;
            return Ok((directories, file));
        }
        let child = parent
            .child(part, false)
            .map_err(|_| ReadSourceStatus::Unreadable)?
            .ok_or(ReadSourceStatus::Missing)?;
        directories.push(child);
    }
    Err(ReadSourceStatus::Unverified)
}

fn check_file(
    root: &Directory,
    asset: &AssetVersion,
    after_hash: impl FnOnce(),
) -> ReadSourceStatus {
    use ReadSourceStatus::*;
    let path = asset.relative_path.as_str();
    if !source_path_is_safe(path) {
        return Unverified;
    }
    let Some(expected) = asset.evidence.blake3 else {
        return Unverified;
    };
    if root.verify().is_err() {
        return Unverified;
    }
    let (directories, mut file) = match open_source(root, path) {
        Ok(v) => v,
        Err(status) => return status,
    };
    let Ok(before) = file.metadata() else {
        return Unreadable;
    };
    if before.len() != asset.evidence.size_bytes
        || asset
            .source_entity_id
            .is_some_and(|id| id != crate::review::assets::entity_id(&before, &asset.relative_path))
    {
        return Changed;
    }
    let digest = match hash_source(&mut file, before.len()) {
        Ok(v) => v,
        Err(status) => return status,
    };
    after_hash();
    if root.verify().is_err() || directories.iter().any(|d| d.verify().is_err()) {
        return Unverified;
    }
    let Ok(after) = file.metadata() else {
        return Unreadable;
    };
    let (_, reopened) = match open_source(root, path) {
        Ok(v) => v,
        Err(status) => return status,
    };
    let Ok(located) = reopened.metadata() else {
        return Unreadable;
    };
    if !same_contents(&before, &after) || !same_contents(&after, &located) || digest != expected {
        return Changed;
    }
    Match
}

fn source_path_is_safe(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', '\0'])
        && path.as_bytes().get(1) != Some(&b':')
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.eq_ignore_ascii_case(".viewer"))
}

fn hash_source(input: &mut impl Read, expected_size: u64) -> Result<[u8; 32], ReadSourceStatus> {
    let mut buffer = [0; 64 * 1024];
    let mut hasher = blake3::Hasher::new();
    let mut total = 0_u64;
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|_| ReadSourceStatus::Unreadable)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|v| *v <= expected_size)
            .ok_or(ReadSourceStatus::Changed)?;
        hasher.update(&buffer[..count]);
    }
    if total != expected_size {
        return Err(ReadSourceStatus::Changed);
    }
    Ok(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use viewer_domain::review::{AssetEvidence, ReviewMedia};
    use viewer_domain::{AssetVersionId, RelativePath};

    fn asset(path: &str) -> AssetVersion {
        AssetVersion {
            id: AssetVersionId::from_u128(1),
            source_entity_id: None,
            relative_path: RelativePath::parse(path).unwrap(),
            evidence: AssetEvidence {
                size_bytes: 3,
                modified_ns: 0,
                blake3: Some(*blake3::hash(b"abc").as_bytes()),
            },
            media: ReviewMedia::Image {
                width: Some(1),
                height: Some(1),
            },
            producer_asset_id: None,
            parent_asset_version_id: None,
        }
    }
    #[test]
    fn source_checks_use_content_and_never_follow_links_or_metadata_paths() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("a"), b"abc").unwrap();
        let root = Directory::open_anchored(temp.path()).unwrap();
        assert_eq!(
            check_file(&root, &asset("a"), || {}),
            ReadSourceStatus::Match
        );
        fs::write(temp.path().join("a"), b"xyz").unwrap();
        assert_eq!(
            check_file(&root, &asset("a"), || {}),
            ReadSourceStatus::Changed
        );
        assert_eq!(
            check_file(&root, &asset("missing"), || {}),
            ReadSourceStatus::Missing
        );
        let mut unhashed = asset("a");
        unhashed.evidence.blake3 = None;
        assert_eq!(
            check_file(&root, &unhashed, || {}),
            ReadSourceStatus::Unverified
        );
        std::os::unix::fs::symlink(temp.path().join("a"), temp.path().join("link")).unwrap();
        fs::hard_link(temp.path().join("a"), temp.path().join("hard")).unwrap();
        fs::create_dir(temp.path().join("directory")).unwrap();
        for name in ["link", "hard", "directory"] {
            assert_eq!(
                check_file(&root, &asset(name), || {}),
                ReadSourceStatus::Unreadable
            );
        }
        for name in [
            ".viewer/index.json",
            ".Viewer/index.json",
            "nested\\a",
            "C:a",
        ] {
            assert!(!source_path_is_safe(name));
        }
    }
    #[test]
    fn source_replacement_after_hash_and_parent_swap_cannot_report_match() {
        for parent_swap in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            fs::create_dir(temp.path().join("dir")).unwrap();
            fs::write(temp.path().join("dir/a"), b"abc").unwrap();
            let root = Directory::open_anchored(temp.path()).unwrap();
            let result = check_file(&root, &asset("dir/a"), || {
                if parent_swap {
                    fs::rename(temp.path().join("dir"), temp.path().join("old")).unwrap();
                    fs::create_dir(temp.path().join("dir")).unwrap();
                }
                fs::write(temp.path().join("dir/new"), b"abc").unwrap();
                fs::rename(temp.path().join("dir/new"), temp.path().join("dir/a")).unwrap();
            });
            assert_ne!(result, ReadSourceStatus::Match);
        }
    }
    #[test]
    fn large_source_hash_has_a_fixed_64_kib_read_buffer() {
        struct Counted {
            file: File,
            largest: usize,
        }
        impl Read for Counted {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.largest = self.largest.max(buf.len());
                self.file.read(buf)
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large");
        File::create(&path)
            .unwrap()
            .set_len(32 * 1024 * 1024)
            .unwrap();
        let mut input = Counted {
            file: File::open(path).unwrap(),
            largest: 0,
        };
        let digest = hash_source(&mut input, 32 * 1024 * 1024).unwrap();
        let mut expected = blake3::Hasher::new();
        for _ in 0..512 {
            expected.update(&[0; 65536]);
        }
        assert_eq!(digest, *expected.finalize().as_bytes());
        assert_eq!(input.largest, 65536);
    }
}
