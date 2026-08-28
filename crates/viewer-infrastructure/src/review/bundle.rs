use super::atomic::{atomic_replace, sync_directory};
use super::protocol::v2::{
    V2ArtifactAnnotation, V2ArtifactRecord, V2CompletedDocument, decode_completed_document,
    encode_completed_with_artifacts,
};
use super::repository::{
    ReviewRepositoryFaultInjector, ReviewRepositoryFaultPoint, append_snapshot,
};
use super::{MAX_REVIEW_DOCUMENT_BYTES, encode_catalog_v2};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use viewer_application::{
    MAX_REVIEW_ARTIFACT_BYTES, MAX_REVIEW_ARTIFACT_PIXELS, MAX_REVIEW_BUNDLE_BYTES, ReviewCatalog,
    ReviewProtocolVersion, ReviewPublication, ReviewRecordLocation, ReviewRepositoryError,
    ReviewRoundRecord,
};
use viewer_domain::review::{FeedbackAnchor, ReviewSnapshot};
use viewer_domain::{AssetVersionId, FeedbackId, ProjectId, ReviewRoundId};

const ROUNDS_DIRECTORY: &str = "rounds";
const ARTIFACTS_DIRECTORY: &str = "artifacts";
const ROUND_MANIFEST: &str = "round.json";
const COPY_BUFFER_BYTES: usize = 64 * 1024;

static BUNDLE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct ValidatedBundle {
    pub document: V2CompletedDocument,
    pub record: ReviewRoundRecord,
}

pub(crate) fn publish_v2(
    reviews_root: &Path,
    mut catalog: ReviewCatalog,
    publication: &ReviewPublication,
    faults: &dyn ReviewRepositoryFaultInjector,
) -> Result<(), ReviewRepositoryError> {
    let records = validate_publication(publication)?;
    // Validate complete references and canonical ordinals before any bundle write.
    let round_bytes = encode_completed_with_artifacts(&publication.snapshot, &records)
        .map_err(super::repository::map_protocol_error)?;
    for artifact in &publication.artifacts {
        validate_source_artifact(artifact)?;
    }
    let rounds_root = reviews_root.join(ROUNDS_DIRECTORY);
    let round_id = publication.snapshot.review_round_id;
    let final_directory = rounds_root.join(round_id.to_string());
    if path_kind(&final_directory)?.is_some() {
        return Err(ReviewRepositoryError::Conflict);
    }
    let temporary_directory = create_temporary_bundle(&rounds_root, round_id)?;
    let artifacts_directory = temporary_directory.join(ARTIFACTS_DIRECTORY);
    fs::create_dir(&artifacts_directory).map_err(map_io_error)?;

    for (source, record) in publication.artifacts.iter().zip(&records) {
        copy_artifact(source, &temporary_directory.join(&record.relative_path))?;
        faults.check(ReviewRepositoryFaultPoint::AfterArtifactFileSync)?;
    }

    write_synced_new(&temporary_directory.join(ROUND_MANIFEST), &round_bytes)?;
    faults.check(ReviewRepositoryFaultPoint::AfterRoundManifestSync)?;
    sync_directory(&artifacts_directory).map_err(map_io_error)?;
    sync_directory(&temporary_directory).map_err(map_io_error)?;
    faults.check(ReviewRepositoryFaultPoint::AfterBundleDirectorySync)?;

    fs::rename(&temporary_directory, &final_directory).map_err(map_io_error)?;
    sync_directory(&rounds_root).map_err(map_io_error)?;
    faults.check(ReviewRepositoryFaultPoint::AfterBundleDurableBeforeIndex)?;

    let record = ReviewRoundRecord {
        review_round_id: round_id,
        protocol_version: ReviewProtocolVersion::V2,
        location: ReviewRecordLocation::new(format!("rounds/{round_id}/{ROUND_MANIFEST}"))
            .map_err(|_| ReviewRepositoryError::InvalidData)?,
        blake3: *blake3::hash(&round_bytes).as_bytes(),
    };
    append_snapshot(&mut catalog, &publication.snapshot, record);
    let catalog_bytes =
        encode_catalog_v2(&catalog).map_err(super::repository::map_protocol_error)?;
    faults.check(ReviewRepositoryFaultPoint::BeforeIndexReplace)?;
    atomic_replace(&reviews_root.join("index.json"), &catalog_bytes).map_err(map_io_error)?;
    faults.check(ReviewRepositoryFaultPoint::AfterIndexReplace)?;
    Ok(())
}

pub(crate) fn validate_indexed_bundles(
    reviews_root: &Path,
    catalog: &ReviewCatalog,
) -> Result<(), ReviewRepositoryError> {
    for stream in &catalog.streams {
        for record in &stream.completed_rounds {
            if record.protocol_version != ReviewProtocolVersion::V2 {
                continue;
            }
            let validated = validate_bundle_directory(
                reviews_root,
                catalog.project_id,
                record.review_round_id,
            )?;
            if &validated.record != record
                || validated.document.snapshot.review_stream_id != stream.review_stream_id
                || validated.document.snapshot.production != stream.production
            {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_bundle_directory(
    reviews_root: &Path,
    project_id: ProjectId,
    round_id: ReviewRoundId,
) -> Result<ValidatedBundle, ReviewRepositoryError> {
    let round_directory = reviews_root
        .join(ROUNDS_DIRECTORY)
        .join(round_id.to_string());
    if path_kind(&round_directory)? != Some(PathKind::Directory) {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    let manifest_path = round_directory.join(ROUND_MANIFEST);
    let bytes = read_regular_bounded(&manifest_path, MAX_REVIEW_DOCUMENT_BYTES)?;
    let document =
        decode_completed_document(&bytes).map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
    if document.snapshot.project_id != project_id || document.snapshot.review_round_id != round_id {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    validate_bundle_contents(&round_directory, &document)?;
    let record = ReviewRoundRecord {
        review_round_id: round_id,
        protocol_version: ReviewProtocolVersion::V2,
        location: ReviewRecordLocation::new(format!("rounds/{round_id}/{ROUND_MANIFEST}"))
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?,
        blake3: *blake3::hash(&bytes).as_bytes(),
    };
    Ok(ValidatedBundle { document, record })
}

pub(crate) fn cleanup_transaction_directories(
    rounds_root: &Path,
    catalog: &ReviewCatalog,
) -> Result<(), ReviewRepositoryError> {
    if path_kind(rounds_root)? != Some(PathKind::Directory) {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    let indexed = catalog
        .streams
        .iter()
        .flat_map(|stream| stream.completed_round_ids())
        .collect::<HashSet<_>>();
    let mut removed = false;
    for entry in fs::read_dir(rounds_root).map_err(|_| ReviewRepositoryError::Unavailable)? {
        let entry = entry.map_err(|_| ReviewRepositoryError::Unavailable)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        let Some(round_id) = temporary_round_id(&name) else {
            continue;
        };
        if indexed.contains(&round_id)
            || path_kind(&entry.path())? != Some(PathKind::Directory)
            || !temporary_contents_are_owned(&entry.path())?
        {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
        fs::remove_dir_all(entry.path()).map_err(|_| ReviewRepositoryError::Unavailable)?;
        removed = true;
    }
    if removed {
        sync_directory(rounds_root).map_err(map_io_error)?;
    }
    Ok(())
}

pub(crate) fn temporary_round_id(name: &str) -> Option<ReviewRoundId> {
    let rest = name.strip_prefix('.')?;
    let (round, nonce) = rest.split_once(".tmp-")?;
    if nonce.len() != 16
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let round_id = ReviewRoundId::from_str(round).ok()?;
    (round == round_id.to_string()).then_some(round_id)
}

pub(crate) fn committed_round_id(name: &str) -> Option<ReviewRoundId> {
    let round_id = ReviewRoundId::from_str(name).ok()?;
    (name == round_id.to_string()).then_some(round_id)
}

fn validate_publication(
    publication: &ReviewPublication,
) -> Result<Vec<V2ArtifactRecord>, ReviewRepositoryError> {
    if publication.protocol_version != ReviewProtocolVersion::V2
        || publication.artifacts.len() > publication.snapshot.assets.len()
    {
        return Err(ReviewRepositoryError::InvalidData);
    }
    let expected = expected_image_annotations(&publication.snapshot)?;
    if publication.artifacts.len() != expected.len() {
        return Err(ReviewRepositoryError::InvalidData);
    }
    let asset_ids = publication
        .snapshot
        .assets
        .iter()
        .map(|asset| asset.id)
        .collect::<HashSet<_>>();
    let mut seen_assets = HashSet::new();
    let mut bundle_bytes = 0_u64;
    let mut records = Vec::with_capacity(publication.artifacts.len());
    for artifact in &publication.artifacts {
        if !asset_ids.contains(&artifact.asset_version_id)
            || !seen_assets.insert(artifact.asset_version_id)
            || artifact.media_type != "image/png"
            || artifact.width == 0
            || artifact.height == 0
            || artifact.size_bytes > MAX_REVIEW_ARTIFACT_BYTES
            || u64::from(artifact.width)
                .checked_mul(u64::from(artifact.height))
                .is_none_or(|pixels| pixels > MAX_REVIEW_ARTIFACT_PIXELS)
        {
            return Err(ReviewRepositoryError::InvalidData);
        }
        bundle_bytes = bundle_bytes
            .checked_add(artifact.size_bytes)
            .ok_or(ReviewRepositoryError::LimitExceeded)?;
        if bundle_bytes > MAX_REVIEW_BUNDLE_BYTES {
            return Err(ReviewRepositoryError::LimitExceeded);
        }
        let expected_feedback = expected
            .get(&artifact.asset_version_id)
            .ok_or(ReviewRepositoryError::InvalidData)?;
        validate_annotations(&artifact.annotations, expected_feedback)?;
        records.push(V2ArtifactRecord {
            asset_version_id: artifact.asset_version_id,
            relative_path: format!(
                "{ARTIFACTS_DIRECTORY}/{}-annotation.png",
                artifact.asset_version_id
            ),
            blake3: artifact.blake3,
            media_type: artifact.media_type.clone(),
            width: artifact.width,
            height: artifact.height,
            annotations: artifact
                .annotations
                .iter()
                .map(|annotation| V2ArtifactAnnotation {
                    ordinal: annotation.ordinal,
                    feedback_id: annotation.feedback_id,
                })
                .collect(),
        });
    }
    if seen_assets != expected.keys().copied().collect::<HashSet<_>>() {
        return Err(ReviewRepositoryError::InvalidData);
    }
    Ok(records)
}

fn expected_image_annotations(
    snapshot: &ReviewSnapshot,
) -> Result<HashMap<AssetVersionId, HashSet<FeedbackId>>, ReviewRepositoryError> {
    let mut expected = HashMap::<AssetVersionId, HashSet<FeedbackId>>::new();
    for feedback in &snapshot.feedback {
        for target in &feedback.targets {
            if matches!(
                target.anchor,
                FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
            ) && !expected
                .entry(target.asset_version_id)
                .or_default()
                .insert(feedback.id)
            {
                return Err(ReviewRepositoryError::InvalidData);
            }
        }
    }
    Ok(expected)
}

fn validate_annotations(
    annotations: &[viewer_application::ReviewArtifactAnnotation],
    expected_feedback: &HashSet<FeedbackId>,
) -> Result<(), ReviewRepositoryError> {
    if annotations.len() != expected_feedback.len() {
        return Err(ReviewRepositoryError::InvalidData);
    }
    let mut ordinals = HashSet::new();
    let mut feedback_ids = HashSet::new();
    for annotation in annotations {
        if annotation.ordinal == 0
            || !ordinals.insert(annotation.ordinal)
            || !feedback_ids.insert(annotation.feedback_id)
        {
            return Err(ReviewRepositoryError::InvalidData);
        }
    }
    if feedback_ids != *expected_feedback
        || (1..=annotations.len() as u32).any(|ordinal| !ordinals.contains(&ordinal))
    {
        return Err(ReviewRepositoryError::InvalidData);
    }
    Ok(())
}

fn copy_artifact(
    artifact: &viewer_application::ReviewRenderedArtifact,
    destination: &Path,
) -> Result<(), ReviewRepositoryError> {
    if path_kind(&artifact.temporary_path)? != Some(PathKind::File) {
        return Err(ReviewRepositoryError::InvalidData);
    }
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&artifact.temporary_path)
        .map_err(map_io_error)?;
    let metadata = source.metadata().map_err(map_io_error)?;
    if !metadata.is_file()
        || metadata.len() != artifact.size_bytes
        || metadata.len() > MAX_REVIEW_ARTIFACT_BYTES
    {
        return Err(ReviewRepositoryError::InvalidData);
    }
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(destination)
        .map_err(map_io_error)?;
    let mut header = [0_u8; 24];
    source
        .read_exact(&mut header)
        .map_err(|_| ReviewRepositoryError::InvalidData)?;
    validate_png_header(&header, artifact.width, artifact.height)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&header);
    target.write_all(&header).map_err(map_io_error)?;
    let mut copied = header.len() as u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = source.read(&mut buffer).map_err(map_io_error)?;
        if read == 0 {
            break;
        }
        copied = copied
            .checked_add(read as u64)
            .ok_or(ReviewRepositoryError::LimitExceeded)?;
        if copied > artifact.size_bytes || copied > MAX_REVIEW_ARTIFACT_BYTES {
            return Err(ReviewRepositoryError::InvalidData);
        }
        hasher.update(&buffer[..read]);
        target.write_all(&buffer[..read]).map_err(map_io_error)?;
    }
    if copied != artifact.size_bytes || hasher.finalize().as_bytes() != &artifact.blake3 {
        return Err(ReviewRepositoryError::InvalidData);
    }
    target.sync_all().map_err(map_io_error)
}

fn validate_source_artifact(
    artifact: &viewer_application::ReviewRenderedArtifact,
) -> Result<(), ReviewRepositoryError> {
    if path_kind(&artifact.temporary_path)? != Some(PathKind::File) {
        return Err(ReviewRepositoryError::InvalidData);
    }
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&artifact.temporary_path)
        .map_err(|_| ReviewRepositoryError::InvalidData)?;
    let metadata = source
        .metadata()
        .map_err(|_| ReviewRepositoryError::InvalidData)?;
    if !metadata.is_file()
        || metadata.len() != artifact.size_bytes
        || metadata.len() > MAX_REVIEW_ARTIFACT_BYTES
    {
        return Err(ReviewRepositoryError::InvalidData);
    }
    let mut header = [0_u8; 24];
    source
        .read_exact(&mut header)
        .map_err(|_| ReviewRepositoryError::InvalidData)?;
    validate_png_header(&header, artifact.width, artifact.height)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&header);
    let mut read_total = header.len() as u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| ReviewRepositoryError::InvalidData)?;
        if read == 0 {
            break;
        }
        read_total = read_total
            .checked_add(read as u64)
            .ok_or(ReviewRepositoryError::LimitExceeded)?;
        if read_total > artifact.size_bytes || read_total > MAX_REVIEW_ARTIFACT_BYTES {
            return Err(ReviewRepositoryError::InvalidData);
        }
        hasher.update(&buffer[..read]);
    }
    if read_total != artifact.size_bytes || hasher.finalize().as_bytes() != &artifact.blake3 {
        return Err(ReviewRepositoryError::InvalidData);
    }
    Ok(())
}

fn validate_bundle_contents(
    round_directory: &Path,
    document: &V2CompletedDocument,
) -> Result<(), ReviewRepositoryError> {
    let artifacts_directory = round_directory.join(ARTIFACTS_DIRECTORY);
    if path_kind(&artifacts_directory)? != Some(PathKind::Directory) {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    let expected_files = document
        .artifacts
        .iter()
        .map(|artifact| {
            let filename = Path::new(&artifact.relative_path)
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(ReviewRepositoryError::RecoveryRequired)?;
            Ok((filename.to_owned(), artifact))
        })
        .collect::<Result<HashMap<_, _>, ReviewRepositoryError>>()?;
    let root_entries = fs::read_dir(round_directory)
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
    if root_entries.len() != 2
        || root_entries.iter().any(|entry| {
            let name = entry.file_name();
            name != ROUND_MANIFEST && name != ARTIFACTS_DIRECTORY
        })
    {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    let entries = fs::read_dir(&artifacts_directory)
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
    let mut bundle_bytes = 0_u64;
    // Check aggregate sizes before reading/hashing potentially GiBs of data.
    for entry in &entries {
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
        if metadata.len() > MAX_REVIEW_ARTIFACT_BYTES {
            return Err(ReviewRepositoryError::LimitExceeded);
        }
        bundle_bytes = bundle_bytes
            .checked_add(metadata.len())
            .ok_or(ReviewRepositoryError::LimitExceeded)?;
        if bundle_bytes > MAX_REVIEW_BUNDLE_BYTES {
            return Err(ReviewRepositoryError::LimitExceeded);
        }
    }
    let mut seen = HashSet::new();
    let mut read_total = 0_u64;
    for entry in entries {
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        let artifact = expected_files
            .get(&filename)
            .ok_or(ReviewRepositoryError::RecoveryRequired)?;
        if !seen.insert(filename) || path_kind(&entry.path())? != Some(PathKind::File) {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
        let bytes = read_regular_bounded(
            &entry.path(),
            MAX_REVIEW_ARTIFACT_BYTES.min(MAX_REVIEW_BUNDLE_BYTES - read_total),
        )?;
        read_total += bytes.len() as u64;
        validate_png_header(&bytes, artifact.width, artifact.height)
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        if *blake3::hash(&bytes).as_bytes() != artifact.blake3 {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
    }
    if seen.len() != expected_files.len() {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    Ok(())
}

pub(super) fn validate_png_header(
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<(), ReviewRepositoryError> {
    if bytes.len() < 24
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) != width
        || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) != height
        || width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > MAX_REVIEW_ARTIFACT_PIXELS
    {
        return Err(ReviewRepositoryError::InvalidData);
    }
    Ok(())
}

pub(crate) fn temporary_contents_are_owned(path: &Path) -> Result<bool, ReviewRepositoryError> {
    for entry in fs::read_dir(path).map_err(|_| ReviewRepositoryError::RecoveryRequired)? {
        let entry = entry.map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        match name.as_str() {
            ROUND_MANIFEST => {
                if path_kind(&entry.path())? != Some(PathKind::File) {
                    return Ok(false);
                }
            }
            ARTIFACTS_DIRECTORY => {
                if path_kind(&entry.path())? != Some(PathKind::Directory) {
                    return Ok(false);
                }
                for artifact in fs::read_dir(entry.path())
                    .map_err(|_| ReviewRepositoryError::RecoveryRequired)?
                {
                    let artifact = artifact.map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
                    let name = artifact
                        .file_name()
                        .into_string()
                        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
                    let Some(encoded_id) = name.strip_suffix("-annotation.png") else {
                        return Ok(false);
                    };
                    let Ok(asset_version_id) = AssetVersionId::from_str(encoded_id) else {
                        return Ok(false);
                    };
                    if name != format!("{asset_version_id}-annotation.png")
                        || path_kind(&artifact.path())? != Some(PathKind::File)
                    {
                        return Ok(false);
                    }
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

fn create_temporary_bundle(
    rounds_root: &Path,
    round_id: ReviewRoundId,
) -> Result<PathBuf, ReviewRepositoryError> {
    for _ in 0..64 {
        let nonce = BUNDLE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = rounds_root.join(format!(".{round_id}.tmp-{nonce:016x}"));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(map_io_error(error)),
        }
    }
    Err(ReviewRepositoryError::Unavailable)
}

fn write_synced_new(path: &Path, bytes: &[u8]) -> Result<(), ReviewRepositoryError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(map_io_error)?;
    file.write_all(bytes).map_err(map_io_error)?;
    file.sync_all().map_err(map_io_error)
}

fn read_regular_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, ReviewRepositoryError> {
    if path_kind(path)? != Some(PathKind::File) {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
    let metadata = file
        .metadata()
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
    if metadata.len() > max_bytes {
        return Err(ReviewRepositoryError::LimitExceeded);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
    if bytes.len() as u64 > max_bytes {
        return Err(ReviewRepositoryError::LimitExceeded);
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PathKind {
    Directory,
    File,
    Symlink,
    Other,
}

fn path_kind(path: &Path) -> Result<Option<PathKind>, ReviewRepositoryError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ReviewRepositoryError::Unavailable),
    };
    let file_type = metadata.file_type();
    Ok(Some(if file_type.is_symlink() {
        PathKind::Symlink
    } else if file_type.is_dir() {
        PathKind::Directory
    } else if file_type.is_file() {
        PathKind::File
    } else {
        PathKind::Other
    }))
}

fn map_io_error(error: io::Error) -> ReviewRepositoryError {
    if error.raw_os_error() == Some(libc::ELOOP) {
        ReviewRepositoryError::InvalidData
    } else {
        ReviewRepositoryError::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_names_require_a_canonical_round_and_fixed_lowercase_nonce() {
        let round_id = ReviewRoundId::from_u128(7);
        assert_eq!(
            temporary_round_id(&format!(".{round_id}.tmp-000000000000000a")),
            Some(round_id)
        );
        assert_eq!(temporary_round_id(&format!(".{round_id}.tmp-a")), None);
        assert_eq!(
            temporary_round_id(&format!(".{round_id}.tmp-000000000000000A")),
            None
        );
        assert_eq!(
            temporary_round_id(".not-a-round.tmp-000000000000000a"),
            None
        );
    }
}
