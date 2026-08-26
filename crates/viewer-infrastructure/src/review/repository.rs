use super::atomic::{AtomicCreateOnceError, atomic_create_once, atomic_replace, sync_directory};
use super::bundle;
use super::catalog::{legacy_round_location, prepare_v2_catalog_migration};
use super::lease::ProjectReviewLease;
use super::{
    MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError,
    decode_catalog_versioned, decode_completed_versioned, decode_draft_versioned, encode_catalog,
    encode_completed, encode_draft, encode_draft_v2,
};
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use viewer_application::{
    DecodedReview, MAX_COMPLETED_ROUNDS_PER_STREAM, MAX_REVIEW_STREAMS, PersistedReviewDraft,
    ReviewCatalog, ReviewProtocolVersion, ReviewPublication, ReviewRecordLocation,
    ReviewRepositoryError, ReviewRepositoryPort, ReviewRoundRecord, ReviewStreamHead,
};
use viewer_domain::review::ReviewSnapshot;
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
    AfterArtifactFileSync,
    AfterRoundManifestSync,
    AfterBundleDirectorySync,
    AfterBundleDurableBeforeIndex,
    AfterRoundDurableBeforeIndex,
    BeforeIndexReplace,
    AfterIndexReplace,
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
            if let Some(catalog) = read_catalog_document(&reviews, project_id)? {
                validate_catalog_identity(&catalog.value, project_id)?;
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
        let existing_catalog = read_catalog_document(&reviews_root, project_id)?;
        if let Some(catalog) = &existing_catalog {
            validate_catalog_identity(&catalog.value, project_id)?;
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
        repository.cleanup_transaction_directories()?;
        repository.recover_orphans()?;
        repository.cleanup_indexed_drafts();
        Ok(repository)
    }

    pub fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError> {
        let Some(catalog) = read_catalog_document(&self.reviews_root, self.project_id)? else {
            return Ok(empty_catalog(self.project_id));
        };
        validate_catalog_identity(&catalog.value, self.project_id)?;
        Ok(catalog.value)
    }

    pub fn load_active_draft(&self) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        let catalog = self.load_catalog()?;
        discover_active_draft(self, &catalog)
    }

    pub fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        let catalog = self.load_catalog()?;
        if let Some(indexed_stream) = catalog
            .streams
            .iter()
            .find(|stream| stream.completed_round_ids().any(|id| id == round_id))
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
        let decoded = decode_draft_versioned(&bytes).map_err(map_protocol_error)?;
        if decoded.value.project_id != self.project_id
            || decoded.value.review_stream_id != stream_id
            || decoded.value.review_round_id != round_id
        {
            return Err(ReviewRepositoryError::InvalidData);
        }
        Ok(Some(PersistedReviewDraft {
            protocol_version: decoded.version,
            draft: decoded.value,
        }))
    }

    pub fn save_draft(
        &self,
        persisted: &PersistedReviewDraft,
    ) -> Result<(), ReviewRepositoryError> {
        if self.access != ReviewRepositoryAccess::ReadWrite {
            return Err(ReviewRepositoryError::ReadOnly);
        }
        let _guard = self
            .write_guard
            .lock()
            .map_err(|_| ReviewRepositoryError::Unavailable)?;
        let draft = &persisted.draft;
        if draft.project_id != self.project_id {
            return Err(ReviewRepositoryError::InvalidData);
        }
        if self.load_catalog()?.streams.iter().any(|stream| {
            stream
                .completed_round_ids()
                .any(|round_id| round_id == draft.review_round_id)
        }) {
            return Err(ReviewRepositoryError::Conflict);
        }
        match safe_file_kind(&self.completed_path(draft.review_round_id))? {
            Some(OwnedPathKind::File) => return Err(ReviewRepositoryError::Conflict),
            Some(_) => return Err(ReviewRepositoryError::InvalidData),
            None => {}
        }
        let bytes = match persisted.protocol_version {
            ReviewProtocolVersion::V1 => encode_draft(draft),
            ReviewProtocolVersion::V2 => encode_draft_v2(draft),
        }
        .map_err(map_protocol_error)?;
        replace_checked(&self.draft_path(draft.review_round_id), &bytes)
    }

    pub fn delete_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<(), ReviewRepositoryError> {
        if self.access != ReviewRepositoryAccess::ReadWrite {
            return Err(ReviewRepositoryError::ReadOnly);
        }
        let _guard = self
            .write_guard
            .lock()
            .map_err(|_| ReviewRepositoryError::Unavailable)?;
        let path = self.draft_path(round_id);
        let bytes = read_bounded(&path, MAX_REVIEW_DOCUMENT_BYTES)?
            .ok_or(ReviewRepositoryError::NotFound)?;
        let draft = decode_draft_versioned(&bytes)
            .map_err(map_protocol_error)?
            .value;
        if draft.project_id != self.project_id
            || draft.review_stream_id != stream_id
            || draft.review_round_id != round_id
        {
            return Err(ReviewRepositoryError::InvalidData);
        }
        if self.load_catalog()?.streams.iter().any(|stream| {
            stream.review_stream_id == stream_id
                && stream.completed_round_ids().any(|id| id == round_id)
        }) {
            return Err(ReviewRepositoryError::Conflict);
        }
        fs::remove_file(&path).map_err(map_open_error)?;
        sync_directory(path.parent().ok_or(ReviewRepositoryError::InvalidData)?)
            .map_err(|_| ReviewRepositoryError::Unavailable)
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
        let Some(record) = stream
            .completed_rounds
            .iter()
            .find(|record| record.review_round_id == round_id)
        else {
            return Ok(None);
        };
        let bytes = read_bounded(
            &self.reviews_root.join(record.location.as_str()),
            MAX_REVIEW_DOCUMENT_BYTES,
        )?
        .ok_or(ReviewRepositoryError::InvalidData)?;
        if *blake3::hash(&bytes).as_bytes() != record.blake3 {
            return Err(ReviewRepositoryError::InvalidData);
        }
        let decoded = decode_completed_versioned(&bytes).map_err(map_protocol_error)?;
        if decoded.version != record.protocol_version {
            return Err(ReviewRepositoryError::InvalidData);
        }
        let snapshot = decoded.value;
        if snapshot.project_id != self.project_id
            || snapshot.review_stream_id != stream_id
            || snapshot.review_round_id != round_id
        {
            return Err(ReviewRepositoryError::InvalidData);
        }
        Ok(Some(snapshot))
    }

    pub fn publish(&self, publication: &ReviewPublication) -> Result<(), ReviewRepositoryError> {
        if self.access != ReviewRepositoryAccess::ReadWrite {
            return Err(ReviewRepositoryError::ReadOnly);
        }
        let _guard = self
            .write_guard
            .lock()
            .map_err(|_| ReviewRepositoryError::Unavailable)?;
        let snapshot = &publication.snapshot;
        if snapshot.project_id != self.project_id {
            return Err(ReviewRepositoryError::InvalidData);
        }
        match publication.protocol_version {
            ReviewProtocolVersion::V1 => self.publish_v1(publication),
            ReviewProtocolVersion::V2 => self.publish_v2(publication),
        }
    }

    fn publish_v1(&self, publication: &ReviewPublication) -> Result<(), ReviewRepositoryError> {
        if !publication.artifacts.is_empty() {
            return Err(ReviewRepositoryError::InvalidData);
        }
        let snapshot = &publication.snapshot;
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
        append_snapshot(
            &mut catalog,
            snapshot,
            ReviewRoundRecord {
                review_round_id: snapshot.review_round_id,
                protocol_version: ReviewProtocolVersion::V1,
                location: ReviewRecordLocation::new(legacy_round_location(
                    snapshot.review_round_id,
                ))
                .map_err(|_| ReviewRepositoryError::InvalidData)?,
                blake3: *blake3::hash(&round_bytes).as_bytes(),
            },
        );
        let catalog_bytes = encode_catalog(&catalog).map_err(map_protocol_error)?;
        self.faults
            .check(ReviewRepositoryFaultPoint::BeforeIndexReplace)?;
        replace_checked(&self.reviews_root.join(INDEX_FILE), &catalog_bytes)?;
        self.remove_stale_draft(snapshot.review_round_id);
        Ok(())
    }

    fn publish_v2(&self, publication: &ReviewPublication) -> Result<(), ReviewRepositoryError> {
        let catalog = self.load_catalog()?;
        prepare_catalog_append(&catalog, &publication.snapshot)?;
        let prepared = self.prepare_v2_catalog_migration(&catalog)?;
        bundle::publish_v2(
            &self.reviews_root,
            prepared,
            publication,
            self.faults.as_ref(),
        )?;
        self.remove_stale_draft(publication.snapshot.review_round_id);
        Ok(())
    }

    pub fn prepare_v2_catalog_migration(
        &self,
        catalog_document: &ReviewCatalog,
    ) -> Result<ReviewCatalog, ReviewRepositoryError> {
        prepare_v2_catalog_migration(catalog_document.clone(), |_, round_id| {
            read_bounded(&self.completed_path(round_id), MAX_REVIEW_DOCUMENT_BYTES)?
                .ok_or(ReviewRepositoryError::RecoveryRequired)
        })
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
        let document = self.load_catalog_document()?;
        let catalog = document.value;
        let orphans = inspect_unindexed_rounds(self, &catalog)?;
        if orphans.is_empty() {
            return Ok(());
        }
        let recovered = plan_orphan_recovery(catalog, orphans)?;
        let uses_v2 = document.version == ReviewProtocolVersion::V2
            || recovered.streams.iter().any(|stream| {
                stream
                    .completed_rounds
                    .iter()
                    .any(|record| record.protocol_version == ReviewProtocolVersion::V2)
            });
        let bytes = if uses_v2 {
            super::encode_catalog_v2(&recovered)
        } else {
            encode_catalog(&recovered)
        }
        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        replace_checked(&self.reviews_root.join(INDEX_FILE), &bytes)?;
        Ok(())
    }

    fn cleanup_transaction_directories(&self) -> Result<(), ReviewRepositoryError> {
        let catalog = self.load_catalog()?;
        bundle::cleanup_transaction_directories(&self.reviews_root.join(ROUNDS_DIRECTORY), &catalog)
    }

    fn load_catalog_document(&self) -> Result<DecodedReview<ReviewCatalog>, ReviewRepositoryError> {
        read_catalog_document(&self.reviews_root, self.project_id)?
            .ok_or(ReviewRepositoryError::InvalidData)
    }

    fn cleanup_indexed_drafts(&self) {
        let Ok(catalog) = self.load_catalog() else {
            return;
        };
        for round_id in catalog
            .streams
            .iter()
            .flat_map(|stream| stream.completed_round_ids())
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

    fn load_active_draft(&self) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        ProjectReviewRepository::load_active_draft(self)
    }

    fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        ProjectReviewRepository::load_draft(self, stream_id, round_id)
    }

    fn save_draft(&self, draft: &PersistedReviewDraft) -> Result<(), ReviewRepositoryError> {
        ProjectReviewRepository::save_draft(self, draft)
    }

    fn delete_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<(), ReviewRepositoryError> {
        ProjectReviewRepository::delete_draft(self, stream_id, round_id)
    }

    fn load_completed(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError> {
        ProjectReviewRepository::load_completed(self, stream_id, round_id)
    }

    fn publish(&self, publication: &ReviewPublication) -> Result<(), ReviewRepositoryError> {
        ProjectReviewRepository::publish(self, publication)
    }
}

fn discover_active_draft(
    repository: &ProjectReviewRepository,
    catalog: &ReviewCatalog,
) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
    let drafts_directory = repository.reviews_root.join(DRAFTS_DIRECTORY);
    match safe_file_kind(&drafts_directory)? {
        None => return Ok(None),
        Some(OwnedPathKind::Directory) => {}
        Some(_) => return Err(ReviewRepositoryError::RecoveryRequired),
    }
    let entries =
        fs::read_dir(&drafts_directory).map_err(|_| ReviewRepositoryError::Unavailable)?;
    let mut active = None;
    for entry in entries {
        let entry = entry.map_err(|_| ReviewRepositoryError::Unavailable)?;
        let path = entry.path();
        if safe_file_kind(&path)? != Some(OwnedPathKind::File) {
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
            .map_err(map_draft_discovery_repository_error)?
            .ok_or(ReviewRepositoryError::RecoveryRequired)?;
        let decoded = decode_draft_versioned(&bytes).map_err(map_draft_discovery_protocol_error)?;
        let draft = &decoded.value;
        if draft.project_id != repository.project_id || draft.review_round_id != filename_round_id {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }

        let indexed_owners = catalog
            .streams
            .iter()
            .filter(|stream| {
                stream
                    .completed_round_ids()
                    .any(|round_id| round_id == filename_round_id)
            })
            .collect::<Vec<_>>();
        match indexed_owners.as_slice() {
            [] => {}
            [owner]
                if owner.review_stream_id == draft.review_stream_id
                    && owner.production == draft.production =>
            {
                continue;
            }
            _ => return Err(ReviewRepositoryError::RecoveryRequired),
        }

        match catalog
            .streams
            .iter()
            .find(|stream| stream.review_stream_id == draft.review_stream_id)
        {
            Some(stream) if stream.production != draft.production => {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            _ => {}
        }
        if active
            .replace(PersistedReviewDraft {
                protocol_version: decoded.version,
                draft: decoded.value,
            })
            .is_some()
        {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }
    }
    Ok(active)
}

fn map_draft_discovery_repository_error(error: ReviewRepositoryError) -> ReviewRepositoryError {
    match error {
        ReviewRepositoryError::Unavailable | ReviewRepositoryError::LimitExceeded => error,
        _ => ReviewRepositoryError::RecoveryRequired,
    }
}

fn map_draft_discovery_protocol_error(error: ReviewProtocolError) -> ReviewRepositoryError {
    match map_protocol_error(error) {
        error @ (ReviewRepositoryError::UnsupportedVersion
        | ReviewRepositoryError::LimitExceeded) => error,
        _ => ReviewRepositoryError::RecoveryRequired,
    }
}

fn prepare_catalog_append(
    catalog: &ReviewCatalog,
    snapshot: &ReviewSnapshot,
) -> Result<(), ReviewRepositoryError> {
    if catalog.streams.iter().any(|stream| {
        stream
            .completed_round_ids()
            .any(|round_id| round_id == snapshot.review_round_id)
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
        if stream.completed_rounds.len() >= MAX_COMPLETED_ROUNDS_PER_STREAM {
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

pub(super) fn append_snapshot(
    catalog: &mut ReviewCatalog,
    snapshot: &ReviewSnapshot,
    record: ReviewRoundRecord,
) {
    if let Some(stream) = catalog
        .streams
        .iter_mut()
        .find(|stream| stream.review_stream_id == snapshot.review_stream_id)
    {
        stream.completed_rounds.push(record);
        stream.latest_completed_round_id = Some(snapshot.review_round_id);
        return;
    }
    catalog.streams.push(ReviewStreamHead {
        review_stream_id: snapshot.review_stream_id,
        production: snapshot.production.clone(),
        completed_rounds: vec![record],
        latest_completed_round_id: Some(snapshot.review_round_id),
    });
}

#[derive(Clone)]
struct UnindexedRound {
    snapshot: ReviewSnapshot,
    record: ReviewRoundRecord,
}

fn inspect_unindexed_rounds(
    repository: &ProjectReviewRepository,
    catalog: &ReviewCatalog,
) -> Result<Vec<UnindexedRound>, ReviewRepositoryError> {
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
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
        let discovered =
            match safe_file_kind(&path).map_err(|_| ReviewRepositoryError::RecoveryRequired)? {
                Some(OwnedPathKind::File) => {
                    let encoded_id = filename
                        .strip_suffix(".json")
                        .ok_or(ReviewRepositoryError::RecoveryRequired)?;
                    let round_id = ReviewRoundId::from_str(encoded_id)
                        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
                    if filename != format!("{round_id}.json") {
                        return Err(ReviewRepositoryError::RecoveryRequired);
                    }
                    let bytes = read_bounded(&path, MAX_REVIEW_DOCUMENT_BYTES)
                        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?
                        .ok_or(ReviewRepositoryError::RecoveryRequired)?;
                    let decoded = decode_completed_versioned(&bytes)
                        .map_err(|_| ReviewRepositoryError::RecoveryRequired)?;
                    if decoded.version != ReviewProtocolVersion::V1
                        || decoded.value.project_id != repository.project_id
                        || decoded.value.review_round_id != round_id
                    {
                        return Err(ReviewRepositoryError::RecoveryRequired);
                    }
                    UnindexedRound {
                        snapshot: decoded.value,
                        record: ReviewRoundRecord {
                            review_round_id: round_id,
                            protocol_version: ReviewProtocolVersion::V1,
                            location: ReviewRecordLocation::new(legacy_round_location(round_id))
                                .map_err(|_| ReviewRepositoryError::RecoveryRequired)?,
                            blake3: *blake3::hash(&bytes).as_bytes(),
                        },
                    }
                }
                Some(OwnedPathKind::Directory) => {
                    if bundle::temporary_round_id(&filename).is_some() {
                        if !bundle::temporary_contents_are_owned(&path)? {
                            return Err(ReviewRepositoryError::RecoveryRequired);
                        }
                        continue;
                    }
                    let round_id = bundle::committed_round_id(&filename)
                        .ok_or(ReviewRepositoryError::RecoveryRequired)?;
                    let validated = bundle::validate_bundle_directory(
                        &repository.reviews_root,
                        repository.project_id,
                        round_id,
                    )?;
                    UnindexedRound {
                        snapshot: validated.document.snapshot,
                        record: validated.record,
                    }
                }
                _ => return Err(ReviewRepositoryError::RecoveryRequired),
            };

        let indexed_owners = catalog
            .streams
            .iter()
            .filter_map(|stream| {
                stream
                    .completed_rounds
                    .iter()
                    .find(|record| record.review_round_id == discovered.record.review_round_id)
                    .map(|record| (stream, record))
            })
            .collect::<Vec<_>>();
        match indexed_owners.as_slice() {
            [] => unindexed.push(discovered),
            [(owner, record)]
                if owner.review_stream_id == discovered.snapshot.review_stream_id
                    && owner.production == discovered.snapshot.production
                    && *record == &discovered.record => {}
            _ => return Err(ReviewRepositoryError::RecoveryRequired),
        }
    }
    Ok(unindexed)
}

fn plan_orphan_recovery(
    mut catalog: ReviewCatalog,
    mut orphans: Vec<UnindexedRound>,
) -> Result<ReviewCatalog, ReviewRepositoryError> {
    orphans.sort_by_key(|round| {
        (
            round.snapshot.review_stream_id.to_string(),
            round.snapshot.review_round_id.to_string(),
        )
    });
    let mut stream_ids = Vec::new();
    for orphan in &orphans {
        if !stream_ids.contains(&orphan.snapshot.review_stream_id) {
            stream_ids.push(orphan.snapshot.review_stream_id);
        }
    }

    for stream_id in stream_ids {
        let mut pending = orphans
            .iter()
            .filter(|round| round.snapshot.review_stream_id == stream_id)
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
                .snapshot
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
                completed_rounds: Vec::new(),
                latest_completed_round_id: None,
            });
            catalog.streams.len() - 1
        };
        if pending
            .iter()
            .any(|round| round.snapshot.production != catalog.streams[stream_index].production)
        {
            return Err(ReviewRepositoryError::RecoveryRequired);
        }

        while !pending.is_empty() {
            let head = catalog.streams[stream_index].latest_completed_round_id;
            let candidates = pending
                .iter()
                .enumerate()
                .filter(|(_, round)| round.snapshot.previous_completed_round_id == head)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            let round = pending.remove(candidates[0]);
            if catalog.streams[stream_index].completed_rounds.len()
                >= MAX_COMPLETED_ROUNDS_PER_STREAM
            {
                return Err(ReviewRepositoryError::RecoveryRequired);
            }
            catalog.streams[stream_index]
                .completed_rounds
                .push(round.record);
            catalog.streams[stream_index].latest_completed_round_id =
                Some(round.snapshot.review_round_id);
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

fn read_catalog_document(
    reviews_root: &Path,
    project_id: ProjectId,
) -> Result<Option<DecodedReview<ReviewCatalog>>, ReviewRepositoryError> {
    let Some(bytes) = read_bounded(&reviews_root.join(INDEX_FILE), MAX_REVIEW_INDEX_BYTES)? else {
        return Ok(None);
    };
    let mut decoded = decode_catalog_versioned(&bytes).map_err(map_protocol_error)?;
    validate_catalog_identity(&decoded.value, project_id)?;
    decoded.value = prepare_v2_catalog_migration(decoded.value, |_, round_id| {
        read_bounded(
            &reviews_root.join(legacy_round_location(round_id)),
            MAX_REVIEW_DOCUMENT_BYTES,
        )?
        .ok_or(ReviewRepositoryError::RecoveryRequired)
    })?;
    if decoded.version == ReviewProtocolVersion::V2 {
        bundle::validate_indexed_bundles(reviews_root, &decoded.value)?;
    }
    Ok(Some(decoded))
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

pub(super) fn map_protocol_error(error: ReviewProtocolError) -> ReviewRepositoryError {
    match error {
        ReviewProtocolError::UnsupportedVersion => ReviewRepositoryError::UnsupportedVersion,
        ReviewProtocolError::InvalidData => ReviewRepositoryError::InvalidData,
        ReviewProtocolError::LimitExceeded => ReviewRepositoryError::LimitExceeded,
    }
}
