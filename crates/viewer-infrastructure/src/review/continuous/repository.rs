use super::super::{MAX_REVIEW_INDEX_BYTES, ReviewProtocolError, lease::ProjectReviewLease, v3};
use super::{
    history, mapping,
    owned_io::{Directory, map_io, same_identity},
};
use std::{path::Path, sync::Mutex};
use viewer_application::{ReviewRepositoryError, review_workspace::*};
use viewer_domain::review::continuous::{ArchiveCheckpoint, SnapshotRef};
use viewer_domain::{ProjectId, ReviewArchiveId, ReviewCommandId, ReviewStreamId};

pub(super) struct Writer {
    pub directory: Directory,
    pub _lease: ProjectReviewLease,
    pub gate: Mutex<()>,
}

pub(in crate::review) struct ContinuousReviewRepository {
    pub(super) root: Directory,
    pub(super) project_id: ProjectId,
    pub(super) writer: Option<Writer>,
}

pub(super) struct View {
    pub directory: Directory,
    pub index: v3::ReviewIndexV3,
    pub index_bytes: Option<Vec<u8>>,
}

impl ContinuousReviewRepository {
    pub fn open(
        path: &Path,
        project_id: ProjectId,
        writable: bool,
    ) -> Result<Self, ReviewCommitError> {
        let mut repository = Self {
            root: Directory::open(path)?,
            project_id,
            writer: None,
        };
        if let Some(directory) = repository.directory(writable)? {
            if writable {
                let file = directory
                    .regular("write.lock", true)?
                    .ok_or(ReviewCommitError::Integrity)?;
                let lease =
                    ProjectReviewLease::acquire_file(file).map_err(|error| match error {
                        ReviewRepositoryError::Busy => ReviewCommitError::LeaseBusy,
                        ReviewRepositoryError::InvalidData => ReviewCommitError::Integrity,
                        _ => ReviewCommitError::Io,
                    })?;
                repository.writer = Some(Writer {
                    directory,
                    _lease: lease,
                    gate: Mutex::new(()),
                });
            }
            repository.view()?;
        }
        Ok(repository)
    }

    fn directory(&self, create: bool) -> Result<Option<Directory>, ReviewCommitError> {
        let Some(viewer) = self.root.child(".viewer", create)? else {
            return Ok(None);
        };
        viewer.child("reviews", create)
    }

    pub(super) fn view(&self) -> Result<Option<View>, ReviewCommitError> {
        let Some(directory) = self.directory(false)? else {
            return Ok(None);
        };
        if let Some(writer) = &self.writer {
            if !same_identity(
                &directory.file.metadata().map_err(map_io)?,
                &writer.directory.file.metadata().map_err(map_io)?,
            ) {
                return Err(ReviewCommitError::Integrity);
            }
            let lock = directory
                .regular("write.lock", false)?
                .ok_or(ReviewCommitError::Integrity)?;
            if !writer
                ._lease
                .matches_file(&lock)
                .map_err(|_| ReviewCommitError::Io)?
            {
                return Err(ReviewCommitError::Integrity);
            }
        }
        let index_bytes = directory.read("index.json", MAX_REVIEW_INDEX_BYTES)?;
        let index = match &index_bytes {
            Some(bytes) => v3::decode_index_v3(bytes).map_err(protocol_error)?,
            None => v3::ReviewIndexV3 {
                project_id: self.project_id,
                streams: vec![],
            },
        };
        if index.project_id != self.project_id {
            return Err(ReviewCommitError::Integrity);
        }
        Ok(Some(View {
            directory,
            index,
            index_bytes,
        }))
    }
}

impl ContinuousReviewRepositoryPort for ContinuousReviewRepository {
    fn load_current(
        &self,
        stream_id: ReviewStreamId,
    ) -> Result<Option<StoredContinuousSnapshot>, ReviewCommitError> {
        let Some(view) = self.view()? else {
            return Ok(None);
        };
        let Some(stream) = view
            .index
            .streams
            .iter()
            .find(|v| v.review_stream_id == stream_id)
        else {
            return Ok(None);
        };
        let Some(reference) = stream.current_ref else {
            return Ok(None);
        };
        let record = history::read_state(&view, stream_id, &reference)?;
        super::references::feedback_origins(&view, &record)?;
        Ok(Some(mapping::stored(record, reference, stream)?))
    }

    fn load_snapshot(
        &self,
        stream_id: ReviewStreamId,
        reference: &SnapshotRef,
    ) -> Result<StoredContinuousSnapshot, ReviewCommitError> {
        let view = self.view()?.ok_or(ReviewCommitError::Integrity)?;
        let stream = history::stream(&view, stream_id)?;
        let record = history::reachable(&view, stream_id, reference)?;
        super::references::feedback_origins(&view, &record)?;
        mapping::stored(record, *reference, stream)
    }

    fn load_archive(
        &self,
        stream_id: ReviewStreamId,
        archive_id: ReviewArchiveId,
    ) -> Result<ArchiveCheckpoint, ReviewCommitError> {
        let view = self.view()?.ok_or(ReviewCommitError::Integrity)?;
        history::archive(&view, stream_id, archive_id).map(|v| v.checkpoint)
    }

    fn find_command(
        &self,
        stream_id: ReviewStreamId,
        command_id: ReviewCommandId,
    ) -> Result<CommandLookup, ReviewCommitError> {
        let Some(view) = self.view()? else {
            return Ok(CommandLookup::Absent);
        };
        history::find_command(&view, stream_id, command_id)
    }

    fn commit(
        &self,
        request: ReviewCommitRequest,
    ) -> Result<ReviewCommitReceipt, ReviewCommitError> {
        super::commit::commit(self, request)
    }
}

pub(super) fn protocol_error(error: ReviewProtocolError) -> ReviewCommitError {
    match error {
        ReviewProtocolError::LimitExceeded => ReviewCommitError::LimitExceeded,
        _ => ReviewCommitError::Integrity,
    }
}
