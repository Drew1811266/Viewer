use super::super::{
    atomic::{AtomicCreateOnceError, atomic_create_once_with},
    bundle::validate_png_header,
    v3,
};
use super::{
    owned_io::{Directory, map_io, same_contents},
    repository::View,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Seek, Write},
};
use viewer_application::{
    MAX_REVIEW_ARTIFACT_BYTES,
    review_workspace::{PreparedEvidenceFile, ReviewCommitError},
};

fn references(bindings: &[v3::EvidenceBinding]) -> HashMap<[u8; 32], &v3::EvidenceRef> {
    let mut refs = HashMap::new();
    for binding in bindings {
        if let v3::EvidenceCapability::Image {
            base, annotated, ..
        } = &binding.capability
        {
            refs.insert(base.blake3, base);
            if let Some(annotated) = annotated {
                refs.insert(annotated.blake3, annotated);
            }
        }
    }
    refs
}

fn name(reference: &v3::EvidenceRef) -> String {
    format!(
        "{}.png",
        blake3::Hash::from_bytes(reference.blake3).to_hex()
    )
}

pub(super) fn verify(
    view: &View,
    bindings: &[v3::EvidenceBinding],
) -> Result<(), ReviewCommitError> {
    let references = references(bindings);
    if references.is_empty() {
        return Ok(());
    }
    let directory = view.directory.required_child("evidence")?;
    for reference in references.values() {
        verify_file(&directory, reference)?;
    }
    Ok(())
}

fn verify_file(
    directory: &Directory,
    reference: &v3::EvidenceRef,
) -> Result<(), ReviewCommitError> {
    let mut file = directory
        .regular(&name(reference), false)?
        .ok_or(ReviewCommitError::Integrity)?;
    copy_png(&mut file, reference, &mut io::sink())?;
    directory.verify()
}

pub(super) fn install(
    view: &View,
    bindings: &[v3::EvidenceBinding],
    staged: &[PreparedEvidenceFile],
) -> Result<(), ReviewCommitError> {
    let references = references(bindings);
    let mut sources = HashMap::new();
    for prepared in staged {
        let reference: v3::EvidenceRef = prepared.reference.clone().into();
        if references.get(&reference.blake3).copied() != Some(&reference)
            || sources.insert(reference.blake3, prepared).is_some()
        {
            return Err(ReviewCommitError::Integrity);
        }
    }
    if references.is_empty() {
        return Ok(());
    }
    let directory = view
        .directory
        .child("evidence", true)?
        .ok_or(ReviewCommitError::Integrity)?;
    for reference in references.values() {
        if directory.regular(&name(reference), false)?.is_some() {
            verify_file(&directory, reference)?;
            continue;
        }
        let prepared = sources
            .get(&reference.blake3)
            .ok_or(ReviewCommitError::Integrity)?;
        let parent = Directory::open(prepared.path.parent().ok_or(ReviewCommitError::Integrity)?)?;
        let leaf = prepared
            .path
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(ReviewCommitError::Integrity)?;
        let mut input = parent
            .regular(leaf, false)?
            .ok_or(ReviewCommitError::Integrity)?;
        copy_png(&mut input, reference, &mut io::sink())?;
        input.rewind().map_err(map_io)?;
        let publication = atomic_create_once_with(&directory.file, &name(reference), |output| {
            copy_png(&mut input, reference, output)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        });
        match publication {
            Ok(()) | Err(AtomicCreateOnceError::AlreadyExists) => {
                verify_file(&directory, reference)?
            }
            Err(AtomicCreateOnceError::Io(error)) => return Err(map_io(error)),
        }
        parent.verify()?;
    }
    Ok(())
}

fn copy_png(
    input: &mut File,
    reference: &v3::EvidenceRef,
    output: &mut impl Write,
) -> Result<(), ReviewCommitError> {
    let before = input.metadata().map_err(map_io)?;
    if reference.size_bytes > MAX_REVIEW_ARTIFACT_BYTES || before.len() > MAX_REVIEW_ARTIFACT_BYTES
    {
        return Err(ReviewCommitError::LimitExceeded);
    }
    if before.len() != reference.size_bytes || !before.is_file() {
        return Err(ReviewCommitError::Integrity);
    }
    let mut header = Vec::with_capacity(24);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0; 64 * 1024];
    let mut total = 0_u64;
    let mut bounded = (&mut *input).take(reference.size_bytes + 1);
    loop {
        let count = bounded.read(&mut buffer).map_err(map_io)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > reference.size_bytes {
            return Err(ReviewCommitError::Integrity);
        }
        header.extend_from_slice(&buffer[..count.min(24 - header.len())]);
        hasher.update(&buffer[..count]);
        output.write_all(&buffer[..count]).map_err(map_io)?;
    }
    if total != reference.size_bytes
        || hasher.finalize().as_bytes() != &reference.blake3
        || !same_contents(&before, &input.metadata().map_err(map_io)?)
    {
        return Err(ReviewCommitError::Integrity);
    }
    validate_png_header(&header, reference.width, reference.height)
        .map_err(|_| ReviewCommitError::Integrity)
}
