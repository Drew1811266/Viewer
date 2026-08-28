//! Producer-owned project files: descriptor-relative, no symlinks, bounded JSON, streaming media.
use super::owned_io::{Directory, same_contents};
use std::{fs::File, io::Read};
use viewer_application::review_workspace::{ReviewCommitError, UsageImportError};
use viewer_domain::RelativePath;

pub(super) fn open(
    root: &Directory,
    source: &RelativePath,
) -> Result<(Vec<Directory>, File), UsageImportError> {
    let path = source.as_str();
    if path.len() > 4096 || path.contains('\\') || path.as_bytes().get(1) == Some(&b':') {
        return Err(UsageImportError::UnsafePath);
    }
    root.verify().map_err(map)?;
    let mut parts: Vec<_> = path.split('/').collect();
    let name = parts.pop().ok_or(UsageImportError::UnsafePath)?;
    let mut parents: Vec<Directory> = vec![];
    for part in parts {
        let parent = parents.last().unwrap_or(root);
        parents.push(
            parent
                .child(part, false)
                .map_err(map)?
                .ok_or(UsageImportError::InvalidDeclaration)?,
        );
    }
    let parent = parents.last().unwrap_or(root);
    let file = parent
        .regular(name, false)
        .map_err(map)?
        .ok_or(UsageImportError::InvalidDeclaration)?;
    Ok((parents, file))
}

pub(super) fn read(
    root: &Directory,
    source: &RelativePath,
    retain: bool,
) -> Result<(Vec<u8>, [u8; 32]), UsageImportError> {
    let (parents, mut file) = open(root, source)?;
    let before = file
        .metadata()
        .map_err(|_| UsageImportError::InvalidDeclaration)?;
    if retain && before.len() > 64 * 1024 * 1024 {
        return Err(UsageImportError::LimitExceeded);
    }
    let mut bytes = Vec::new();
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| UsageImportError::InvalidDeclaration)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(UsageImportError::LimitExceeded)?;
        if total > before.len() {
            return Err(UsageImportError::SourceChanged);
        }
        hasher.update(&buffer[..count]);
        if retain {
            bytes.extend_from_slice(&buffer[..count]);
        }
    }
    let after = file
        .metadata()
        .map_err(|_| UsageImportError::SourceChanged)?;
    let (located_parents, located) = open(root, source)?;
    if total != before.len()
        || !same_contents(&before, &after)
        || !same_contents(
            &after,
            &located
                .metadata()
                .map_err(|_| UsageImportError::SourceChanged)?,
        )
    {
        return Err(UsageImportError::SourceChanged);
    }
    for parent in parents.iter().chain(&located_parents) {
        parent.verify().map_err(map)?;
    }
    root.verify().map_err(map)?;
    Ok((bytes, *hasher.finalize().as_bytes()))
}
pub(super) fn map(error: ReviewCommitError) -> UsageImportError {
    match error {
        ReviewCommitError::LimitExceeded => UsageImportError::LimitExceeded,
        ReviewCommitError::Integrity => UsageImportError::UnsafePath,
        _ => UsageImportError::InvalidDeclaration,
    }
}
