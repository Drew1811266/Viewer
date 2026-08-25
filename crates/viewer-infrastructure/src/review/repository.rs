use super::atomic::{atomic_replace, sync_directory};
use super::lease::ProjectReviewLease;
use super::{
    MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError, decode_catalog,
    decode_draft, encode_catalog, encode_draft,
};
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use viewer_application::{ReviewCatalog, ReviewRepositoryError};
use viewer_domain::review::ReviewDraft;
use viewer_domain::{ProjectId, ReviewRoundId, ReviewStreamId};

const VIEWER_DIRECTORY: &str = ".viewer";
const REVIEWS_DIRECTORY: &str = "reviews";
const DRAFTS_DIRECTORY: &str = "drafts";
const ROUNDS_DIRECTORY: &str = "rounds";
const INDEX_FILE: &str = "index.json";
const LOCK_FILE: &str = "write.lock";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewRepositoryAccess {
    ReadOnly,
    ReadWrite,
}

pub struct ProjectReviewRepository {
    _project_root: PathBuf,
    reviews_root: PathBuf,
    project_id: ProjectId,
    access: ReviewRepositoryAccess,
    _lease: Option<ProjectReviewLease>,
}

impl ProjectReviewRepository {
    pub fn open(
        project_root: &Path,
        project_id: ProjectId,
        access: ReviewRepositoryAccess,
    ) -> Result<Self, ReviewRepositoryError> {
        validate_directory(project_root)?;
        let viewer = locate_child(project_root, VIEWER_DIRECTORY)?
            .ok_or(ReviewRepositoryError::InvalidData)?;
        validate_directory(&viewer)?;
        let reviews_root = viewer.join(REVIEWS_DIRECTORY);
        let existing_reviews = locate_child(&viewer, REVIEWS_DIRECTORY)?;

        if existing_reviews.is_none() && access == ReviewRepositoryAccess::ReadOnly {
            return Ok(Self {
                _project_root: project_root.to_path_buf(),
                reviews_root,
                project_id,
                access,
                _lease: None,
            });
        }

        if let Some(reviews) = existing_reviews {
            validate_directory(&reviews)?;
            validate_owned_children(&reviews)?;
            if let Some(catalog) = read_catalog_file(&reviews.join(INDEX_FILE))? {
                validate_catalog_identity(&catalog, project_id)?;
            }
        } else {
            create_directory(&reviews_root)?;
        }

        if access == ReviewRepositoryAccess::ReadOnly {
            return Ok(Self {
                _project_root: project_root.to_path_buf(),
                reviews_root,
                project_id,
                access,
                _lease: None,
            });
        }

        let lease = ProjectReviewLease::acquire(&reviews_root.join(LOCK_FILE))?;
        validate_owned_children(&reviews_root)?;
        let existing_catalog = read_catalog_file(&reviews_root.join(INDEX_FILE))?;
        if let Some(catalog) = &existing_catalog {
            validate_catalog_identity(catalog, project_id)?;
        }
        ensure_owned_directory(&reviews_root, DRAFTS_DIRECTORY)?;
        ensure_owned_directory(&reviews_root, ROUNDS_DIRECTORY)?;
        if existing_catalog.is_none() {
            let catalog = empty_catalog(project_id);
            replace_checked(
                &reviews_root.join(INDEX_FILE),
                &encode_catalog(&catalog).map_err(map_protocol_error)?,
            )?;
        }

        Ok(Self {
            _project_root: project_root.to_path_buf(),
            reviews_root,
            project_id,
            access,
            _lease: Some(lease),
        })
    }

    pub fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError> {
        let Some(catalog) = read_catalog_file(&self.reviews_root.join(INDEX_FILE))? else {
            return Ok(empty_catalog(self.project_id));
        };
        validate_catalog_identity(&catalog, self.project_id)?;
        Ok(catalog)
    }

    pub fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewDraft>, ReviewRepositoryError> {
        let path = self.draft_path(round_id);
        let Some(bytes) = read_bounded(&path, MAX_REVIEW_DOCUMENT_BYTES)? else {
            return Ok(None);
        };
        let draft = decode_draft(&bytes).map_err(map_protocol_error)?;
        if draft.project_id != self.project_id
            || draft.review_stream_id != stream_id
            || draft.review_round_id != round_id
        {
            return Err(ReviewRepositoryError::InvalidData);
        }
        Ok(Some(draft))
    }

    pub fn save_draft(&self, draft: &ReviewDraft) -> Result<(), ReviewRepositoryError> {
        if self.access != ReviewRepositoryAccess::ReadWrite {
            return Err(ReviewRepositoryError::ReadOnly);
        }
        if draft.project_id != self.project_id {
            return Err(ReviewRepositoryError::InvalidData);
        }
        match safe_file_kind(&self.completed_path(draft.review_round_id))? {
            Some(OwnedPathKind::File) => return Err(ReviewRepositoryError::Conflict),
            Some(_) => return Err(ReviewRepositoryError::InvalidData),
            None => {}
        }
        let bytes = encode_draft(draft).map_err(map_protocol_error)?;
        replace_checked(&self.draft_path(draft.review_round_id), &bytes)
    }

    fn draft_path(&self, round_id: ReviewRoundId) -> PathBuf {
        self.reviews_root
            .join(DRAFTS_DIRECTORY)
            .join(format!("{round_id}.json"))
    }

    fn completed_path(&self, round_id: ReviewRoundId) -> PathBuf {
        self.reviews_root
            .join(ROUNDS_DIRECTORY)
            .join(format!("{round_id}.json"))
    }
}

fn empty_catalog(project_id: ProjectId) -> ReviewCatalog {
    ReviewCatalog {
        project_id,
        streams: Vec::new(),
    }
}

fn validate_catalog_identity(
    catalog: &ReviewCatalog,
    project_id: ProjectId,
) -> Result<(), ReviewRepositoryError> {
    if catalog.project_id == project_id {
        Ok(())
    } else {
        Err(ReviewRepositoryError::InvalidData)
    }
}

fn read_catalog_file(path: &Path) -> Result<Option<ReviewCatalog>, ReviewRepositoryError> {
    read_bounded(path, MAX_REVIEW_INDEX_BYTES)?
        .map(|bytes| decode_catalog(&bytes).map_err(map_protocol_error))
        .transpose()
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Option<Vec<u8>>, ReviewRepositoryError> {
    match safe_file_kind(path)? {
        None => return Ok(None),
        Some(OwnedPathKind::File) => {}
        Some(_) => return Err(ReviewRepositoryError::InvalidData),
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(map_open_error)?;
    let metadata = file
        .metadata()
        .map_err(|_| ReviewRepositoryError::Unavailable)?;
    if metadata.len() > max_bytes {
        return Err(ReviewRepositoryError::LimitExceeded);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReviewRepositoryError::Unavailable)?;
    if bytes.len() as u64 > max_bytes {
        return Err(ReviewRepositoryError::LimitExceeded);
    }
    Ok(Some(bytes))
}

fn validate_owned_children(reviews: &Path) -> Result<(), ReviewRepositoryError> {
    for (name, expected) in [
        (DRAFTS_DIRECTORY, OwnedPathKind::Directory),
        (ROUNDS_DIRECTORY, OwnedPathKind::Directory),
        (INDEX_FILE, OwnedPathKind::File),
        (LOCK_FILE, OwnedPathKind::File),
    ] {
        let Some(path) = locate_child(reviews, name)? else {
            continue;
        };
        if safe_file_kind(&path)? != Some(expected) {
            return Err(ReviewRepositoryError::InvalidData);
        }
    }
    Ok(())
}

fn ensure_owned_directory(parent: &Path, name: &str) -> Result<(), ReviewRepositoryError> {
    if let Some(path) = locate_child(parent, name)? {
        return validate_directory(&path);
    }
    create_directory(&parent.join(name))
}

fn create_directory(path: &Path) -> Result<(), ReviewRepositoryError> {
    fs::create_dir(path).map_err(|_| ReviewRepositoryError::Unavailable)?;
    sync_directory(path).map_err(|_| ReviewRepositoryError::Unavailable)?;
    sync_directory(path.parent().ok_or(ReviewRepositoryError::InvalidData)?)
        .map_err(|_| ReviewRepositoryError::Unavailable)
}

fn locate_child(parent: &Path, exact_name: &str) -> Result<Option<PathBuf>, ReviewRepositoryError> {
    let mut found = None;
    let entries = fs::read_dir(parent).map_err(|_| ReviewRepositoryError::Unavailable)?;
    for entry in entries {
        let entry = entry.map_err(|_| ReviewRepositoryError::Unavailable)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case(exact_name) {
            if name.as_ref() != exact_name || found.is_some() {
                return Err(ReviewRepositoryError::InvalidData);
            }
            found = Some(entry.path());
        }
    }
    Ok(found)
}

fn validate_directory(path: &Path) -> Result<(), ReviewRepositoryError> {
    if safe_file_kind(path)? == Some(OwnedPathKind::Directory) {
        Ok(())
    } else {
        Err(ReviewRepositoryError::InvalidData)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum OwnedPathKind {
    Directory,
    File,
    Symlink,
    Other,
}

fn safe_file_kind(path: &Path) -> Result<Option<OwnedPathKind>, ReviewRepositoryError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ReviewRepositoryError::Unavailable),
    };
    let file_type = metadata.file_type();
    Ok(Some(if file_type.is_symlink() {
        OwnedPathKind::Symlink
    } else if file_type.is_dir() {
        OwnedPathKind::Directory
    } else if file_type.is_file() {
        OwnedPathKind::File
    } else {
        OwnedPathKind::Other
    }))
}

fn replace_checked(path: &Path, bytes: &[u8]) -> Result<(), ReviewRepositoryError> {
    if matches!(safe_file_kind(path)?, Some(kind) if kind != OwnedPathKind::File) {
        return Err(ReviewRepositoryError::InvalidData);
    }
    atomic_replace(path, bytes).map_err(map_open_error)
}

fn map_open_error(error: io::Error) -> ReviewRepositoryError {
    if error.raw_os_error() == Some(libc::ELOOP) {
        ReviewRepositoryError::InvalidData
    } else {
        ReviewRepositoryError::Unavailable
    }
}

fn map_protocol_error(error: ReviewProtocolError) -> ReviewRepositoryError {
    match error {
        ReviewProtocolError::UnsupportedVersion => ReviewRepositoryError::UnsupportedVersion,
        ReviewProtocolError::InvalidData => ReviewRepositoryError::InvalidData,
        ReviewProtocolError::LimitExceeded => ReviewRepositoryError::LimitExceeded,
    }
}
