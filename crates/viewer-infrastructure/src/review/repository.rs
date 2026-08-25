use super::atomic::{AtomicCreateOnceError, atomic_create_once, atomic_replace, sync_directory};
use super::lease::ProjectReviewLease;
use super::{
    MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError, decode_catalog,
    decode_completed, decode_draft, encode_catalog, encode_completed, encode_draft,
};
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use viewer_application::{
    MAX_COMPLETED_ROUNDS_PER_STREAM, MAX_REVIEW_STREAMS, ReviewCatalog, ReviewRepositoryError,
    ReviewRepositoryPort, ReviewStreamHead,
};
use viewer_domain::review::{ReviewDraft, ReviewSnapshot};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewRepositoryFaultPoint {
    AfterRoundDurableBeforeIndex,
    BeforeIndexReplace,
}

pub trait ReviewRepositoryFaultInjector: Send + Sync {
    fn check(&self, point: ReviewRepositoryFaultPoint) -> Result<(), ReviewRepositoryError>;
}

pub struct NoReviewRepositoryFaults;

impl ReviewRepositoryFaultInjector for NoReviewRepositoryFaults {
    fn check(&self, _point: ReviewRepositoryFaultPoint) -> Result<(), ReviewRepositoryError> {
        Ok(())
    }
}

pub struct ProjectReviewRepository {
    _project_root: PathBuf,
    reviews_root: PathBuf,
    project_id: ProjectId,
    access: ReviewRepositoryAccess,
    _lease: Option<ProjectReviewLease>,
    faults: Arc<dyn ReviewRepositoryFaultInjector>,
    write_guard: Mutex<()>,
}

impl ProjectReviewRepository {
    pub fn open(
        project_root: &Path,
        project_id: ProjectId,
        access: ReviewRepositoryAccess,
    ) -> Result<Self, ReviewRepositoryError> {
        Self::open_with_faults(
            project_root,
            project_id,
            access,
            Arc::new(NoReviewRepositoryFaults),
        )
    }

    pub fn open_with_faults(
        project_root: &Path,
        project_id: ProjectId,
        access: ReviewRepositoryAccess,
        faults: Arc<dyn ReviewRepositoryFaultInjector>,
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
                faults,
                write_guard: Mutex::new(()),
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
            let repository = Self {
                _project_root: project_root.to_path_buf(),
                reviews_root,
                project_id,
                access,
                _lease: None,
                faults,
                write_guard: Mutex::new(()),
            };
            repository.require_no_orphans()?;
            return Ok(repository);
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

        let repository = Self {
            _project_root: project_root.to_path_buf(),
            reviews_root,
            project_id,
            access,
            _lease: Some(lease),
            faults,
            write_guard: Mutex::new(()),
        };
        repository.recover_orphans()?;
        repository.cleanup_indexed_drafts();
        Ok(repository)
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
        let catalog = self.load_catalog()?;
        if let Some(indexed_stream) = catalog
            .streams
            .iter()
            .find(|stream| stream.completed_round_ids.contains(&round_id))
        {
            return if indexed_stream.review_stream_id == stream_id {
                Ok(None)
            } else {
                Err(ReviewRepositoryError::InvalidData)
            };
        }
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
        let _guard = self
            .write_guard
            .lock()
            .map_err(|_| ReviewRepositoryError::Unavailable)?;
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

    pub fn load_completed(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError> {
        let catalog = self.load_catalog()?;
        let Some(stream) = catalog
            .streams
            .iter()
            .find(|stream| stream.review_stream_id == stream_id)
        else {
            return Ok(None);
        };
        if !stream.completed_round_ids.contains(&round_id) {
            return Ok(None);
        }
        let bytes = read_bounded(&self.completed_path(round_id), MAX_REVIEW_DOCUMENT_BYTES)?
            .ok_or(ReviewRepositoryError::InvalidData)?;
        let snapshot = decode_completed(&bytes).map_err(map_protocol_error)?;
        if snapshot.project_id != self.project_id
            || snapshot.review_stream_id != stream_id
            || snapshot.review_round_id != round_id
        {
            return Err(ReviewRepositoryError::InvalidData);
        }
        Ok(Some(snapshot))
    }

    pub fn publish(&self, snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError> {
        if self.access != ReviewRepositoryAccess::ReadWrite {
            return Err(ReviewRepositoryError::ReadOnly);
        }
        let _guard = self
            .write_guard
            .lock()
            .map_err(|_| ReviewRepositoryError::Unavailable)?;
        if snapshot.project_id != self.project_id {
            return Err(ReviewRepositoryError::InvalidData);
        }
        let round_bytes = encode_completed(snapshot).map_err(map_protocol_error)?;
        let mut catalog = self.load_catalog()?;
        prepare_catalog_append(&catalog, snapshot)?;
        match safe_file_kind(&self.completed_path(snapshot.review_round_id))? {
            Some(OwnedPathKind::File) => return Err(ReviewRepositoryError::Conflict),
            Some(_) => return Err(ReviewRepositoryError::InvalidData),
            None => {}
        }

        atomic_create_once(&self.completed_path(snapshot.review_round_id), &round_bytes)
            .map_err(map_create_once_error)?;
        self.faults
            .check(ReviewRepositoryFaultPoint::AfterRoundDurableBeforeIndex)?;
        append_snapshot(&mut catalog, snapshot);
        let catalog_bytes = encode_catalog(&catalog).map_err(map_protocol_error)?;
        self.faults
            .check(ReviewRepositoryFaultPoint::BeforeIndexReplace)?;
        replace_checked(&self.reviews_root.join(INDEX_FILE), &catalog_bytes)?;
        self.remove_stale_draft(snapshot.review_round_id);
        Ok(())
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

    fn require_no_orphans(&self) -> Result<(), ReviewRepositoryError> {
        let catalog = self.load_catalog()?;
        if inspect_unindexed_rounds(self, &catalog)?.is_empty() {
            Ok(())
        } else {
            Err(ReviewRepositoryError::RecoveryRequired)
        }
    }

    fn recover_orphans(&self) -> Result<(), ReviewRepositoryError> {
        let catalog = self.load_catalog()?;
        let orphans = inspect_unindexed_rounds(self, &catalog)?;
        if orphans.is_empty() {
            return Ok(());
        }
        let recovered = plan_orphan_recovery(catalog, orphans)?;
        let bytes =
            encode_catalog(&recovered).map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        replace_checked(&self.reviews_root.join(INDEX_FILE), &bytes)?;
        Ok(())
    }

    fn cleanup_indexed_drafts(&self) {
        let Ok(catalog) = self.load_catalog() else {
            return;
        };
        for round_id in catalog
            .streams
            .iter()
            .flat_map(|stream| stream.completed_round_ids.iter().copied())
        {
            self.remove_stale_draft(round_id);
        }
    }

    fn remove_stale_draft(&self, round_id: ReviewRoundId) {
        let path = self.draft_path(round_id);
        if safe_file_kind(&path).ok() == Some(Some(OwnedPathKind::File)) {
            let _ = fs::remove_file(path);
        }
    }
}

impl ReviewRepositoryPort for ProjectReviewRepository {
    fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError> {
        ProjectReviewRepository::load_catalog(self)
    }

    fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewDraft>, ReviewRepositoryError> {
        ProjectReviewRepository::load_draft(self, stream_id, round_id)
    }

    fn save_draft(&self, draft: &ReviewDraft) -> Result<(), ReviewRepositoryError> {
        ProjectReviewRepository::save_draft(self, draft)
    }

    fn load_completed(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError> {
        ProjectReviewRepository::load_completed(self, stream_id, round_id)
    }

    fn publish(&self, snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError> {
        ProjectReviewRepository::publish(self, snapshot)
    }
}

fn prepare_catalog_append(
    catalog: &ReviewCatalog,
    snapshot: &ReviewSnapshot,
) -> Result<(), ReviewRepositoryError> {
    if catalog.streams.iter().any(|stream| {
        stream
            .completed_round_ids
            .contains(&snapshot.review_round_id)
    }) {
        return Err(ReviewRepositoryError::Conflict);
    }
    if let Some(stream) = catalog
        .streams
        .iter()
        .find(|stream| stream.review_stream_id == snapshot.review_stream_id)
    {
        if stream.production != snapshot.production
            || stream.latest_completed_round_id != snapshot.previous_completed_round_id
        {
            return Err(ReviewRepositoryError::Conflict);
        }
        if stream.completed_round_ids.len() >= MAX_COMPLETED_ROUNDS_PER_STREAM {
            return Err(ReviewRepositoryError::LimitExceeded);
        }
        return Ok(());
    }

    if catalog.streams.len() >= MAX_REVIEW_STREAMS {
        return Err(ReviewRepositoryError::LimitExceeded);
    }
    if snapshot.previous_completed_round_id.is_some()
        || snapshot.production.is_some()
            && catalog
                .streams
                .iter()
                .any(|stream| stream.production == snapshot.production)
    {
        return Err(ReviewRepositoryError::Conflict);
    }
    Ok(())
}

fn append_snapshot(catalog: &mut ReviewCatalog, snapshot: &ReviewSnapshot) {
    if let Some(stream) = catalog
        .streams
        .iter_mut()
        .find(|stream| stream.review_stream_id == snapshot.review_stream_id)
    {
        stream.completed_round_ids.push(snapshot.review_round_id);
        stream.latest_completed_round_id = Some(snapshot.review_round_id);
        return;
    }
    catalog.streams.push(ReviewStreamHead {
        review_stream_id: snapshot.review_stream_id,
        production: snapshot.production.clone(),
        completed_round_ids: vec![snapshot.review_round_id],
        latest_completed_round_id: Some(snapshot.review_round_id),
    });
}

fn inspect_unindexed_rounds(
    repository: &ProjectReviewRepository,
    catalog: &ReviewCatalog,
) -> Result<Vec<ReviewSnapshot>, ReviewRepositoryError> {
    let rounds_directory = repository.reviews_root.join(ROUNDS_DIRECTORY);
    match safe_file_kind(&rounds_directory)? {
        None => return Ok(Vec::new()),
        Some(OwnedPathKind::Directory) => {}
        Some(_) => return Err(ReviewRepositoryError::RecoveryRequired),
    }
    let entries =
        fs::read_dir(&rounds_directory).map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
    let mut unindexed = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        let path = entry.path();
        if safe_file_kind(&path).map_err(|_| ReviewRepositoryError::RecoveryRequired)?
            != Some(OwnedPathKind::File)
        {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        let encoded_id = filename
            .strip_suffix(".json")
            .ok_or(ReviewRepositoryError::RecoveryRequired)?;
        let filename_round_id = ReviewRoundId::from_str(encoded_id)
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        if filename != format!("{filename_round_id}.json") {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
        let bytes = read_bounded(&path, MAX_REVIEW_DOCUMENT_BYTES)
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?
            .ok_or(ReviewRepositoryError::RecoveryRequired)?;
        let snapshot =
            decode_completed(&bytes).map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        if snapshot.project_id != repository.project_id
            || snapshot.review_round_id != filename_round_id
        {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }

        let indexed_owners = catalog
            .streams
            .iter()
            .filter(|stream| stream.completed_round_ids.contains(&filename_round_id))
            .collect::<Vec<_>>();
        match indexed_owners.as_slice() {
            [] => unindexed.push(snapshot),
            [owner]
                if owner.review_stream_id == snapshot.review_stream_id
                    && owner.production == snapshot.production => {}
            _ => return Err(ReviewRepositoryError::RecoveryRequired),
        }
    }
    Ok(unindexed)
}

fn plan_orphan_recovery(
    mut catalog: ReviewCatalog,
    mut orphans: Vec<ReviewSnapshot>,
) -> Result<ReviewCatalog, ReviewRepositoryError> {
    orphans.sort_by_key(|snapshot| {
        (
            snapshot.review_stream_id.to_string(),
            snapshot.review_round_id.to_string(),
        )
    });
    let mut stream_ids = Vec::new();
    for orphan in &orphans {
        if !stream_ids.contains(&orphan.review_stream_id) {
            stream_ids.push(orphan.review_stream_id);
        }
    }

    for stream_id in stream_ids {
        let mut pending = orphans
            .iter()
            .filter(|snapshot| snapshot.review_stream_id == stream_id)
            .cloned()
            .collect::<Vec<_>>();
        let stream_index = if let Some(index) = catalog
            .streams
            .iter()
            .position(|stream| stream.review_stream_id == stream_id)
        {
            index
        } else {
            let production = pending
                .first()
                .ok_or(ReviewRepositoryError::RecoveryRequired)?
                .production
                .clone();
            if production.is_some()
                && catalog
                    .streams
                    .iter()
                    .any(|stream| stream.production == production)
            {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            catalog.streams.push(ReviewStreamHead {
                review_stream_id: stream_id,
                production,
                completed_round_ids: Vec::new(),
                latest_completed_round_id: None,
            });
            catalog.streams.len() - 1
        };
        if pending
            .iter()
            .any(|snapshot| snapshot.production != catalog.streams[stream_index].production)
        {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }

        while !pending.is_empty() {
            let head = catalog.streams[stream_index].latest_completed_round_id;
            let candidates = pending
                .iter()
                .enumerate()
                .filter(|(_, snapshot)| snapshot.previous_completed_round_id == head)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            let snapshot = pending.remove(candidates[0]);
            if catalog.streams[stream_index].completed_round_ids.len()
                >= MAX_COMPLETED_ROUNDS_PER_STREAM
            {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            catalog.streams[stream_index]
                .completed_round_ids
                .push(snapshot.review_round_id);
            catalog.streams[stream_index].latest_completed_round_id =
                Some(snapshot.review_round_id);
        }
    }
    if catalog.streams.len() > MAX_REVIEW_STREAMS {
        return Err(ReviewRepositoryError::RecoveryRequired);
    }
    Ok(catalog)
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

fn map_create_once_error(error: AtomicCreateOnceError) -> ReviewRepositoryError {
    match error {
        AtomicCreateOnceError::AlreadyExists => ReviewRepositoryError::Conflict,
        AtomicCreateOnceError::Io(error) => map_open_error(error),
    }
}

fn map_protocol_error(error: ReviewProtocolError) -> ReviewRepositoryError {
    match error {
        ReviewProtocolError::UnsupportedVersion => ReviewRepositoryError::UnsupportedVersion,
        ReviewProtocolError::InvalidData => ReviewRepositoryError::InvalidData,
        ReviewProtocolError::LimitExceeded => ReviewRepositoryError::LimitExceeded,
    }
}
