use super::{ProjectReviewRepository, ReviewRepositoryAccess};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use viewer_application::{
    ProjectAccess, ReviewCatalog, ReviewRepositoryError, ReviewRepositoryInspection,
    ReviewRepositoryPort, ReviewRepositoryProviderPort,
};
use viewer_domain::review::{ReviewDraft, ReviewSnapshot};
use viewer_domain::{ProjectId, ReviewRoundId, ReviewStreamId};

pub struct ProjectReviewRepositoryProvider {
    project_root: PathBuf,
    project_id: ProjectId,
    project_access: ProjectAccess,
}

impl ProjectReviewRepositoryProvider {
    pub fn new(project_root: &Path, project_id: ProjectId) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            project_id,
            project_access: ProjectAccess::ReadWrite,
        }
    }

    pub fn new_with_access(
        project_root: &Path,
        project_id: ProjectId,
        project_access: ProjectAccess,
    ) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            project_id,
            project_access,
        }
    }

    fn open(
        &self,
        access: ReviewRepositoryAccess,
    ) -> Result<ProjectReviewRepository, ReviewRepositoryError> {
        ProjectReviewRepository::open(&self.project_root, self.project_id, access)
    }

    fn readonly_metadata_is_absent(&self) -> bool {
        self.project_access == ProjectAccess::ReadOnly
            && fs::symlink_metadata(self.project_root.join(".viewer"))
                .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    }

    fn empty_catalog(&self) -> ReviewCatalog {
        ReviewCatalog {
            project_id: self.project_id,
            streams: vec![],
        }
    }
}

impl ReviewRepositoryProviderPort for ProjectReviewRepositoryProvider {
    fn inspect(&self) -> Result<ReviewRepositoryInspection, ReviewRepositoryError> {
        if self.readonly_metadata_is_absent() {
            return Ok(ReviewRepositoryInspection {
                catalog: self.empty_catalog(),
                active_draft: None,
            });
        }
        let repository = self.open(ReviewRepositoryAccess::ReadOnly)?;
        Ok(ReviewRepositoryInspection {
            catalog: repository.load_catalog()?,
            active_draft: repository.load_active_draft()?,
        })
    }

    fn open_reader(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        if self.readonly_metadata_is_absent() {
            return Ok(Box::new(EmptyReadOnlyReviewRepository {
                catalog: self.empty_catalog(),
            }));
        }
        self.open(ReviewRepositoryAccess::ReadOnly)
            .map(|repository| Box::new(repository) as Box<dyn ReviewRepositoryPort>)
    }

    fn open_writer(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        if self.project_access != ProjectAccess::ReadWrite {
            return Err(ReviewRepositoryError::ReadOnly);
        }
        self.open(ReviewRepositoryAccess::ReadWrite)
            .map(|repository| Box::new(repository) as Box<dyn ReviewRepositoryPort>)
    }
}

struct EmptyReadOnlyReviewRepository {
    catalog: ReviewCatalog,
}

impl ReviewRepositoryPort for EmptyReadOnlyReviewRepository {
    fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError> {
        Ok(self.catalog.clone())
    }

    fn load_active_draft(&self) -> Result<Option<ReviewDraft>, ReviewRepositoryError> {
        Ok(None)
    }

    fn load_draft(
        &self,
        _stream_id: ReviewStreamId,
        _round_id: ReviewRoundId,
    ) -> Result<Option<ReviewDraft>, ReviewRepositoryError> {
        Ok(None)
    }

    fn save_draft(&self, _draft: &ReviewDraft) -> Result<(), ReviewRepositoryError> {
        Err(ReviewRepositoryError::ReadOnly)
    }

    fn delete_draft(
        &self,
        _stream_id: ReviewStreamId,
        _round_id: ReviewRoundId,
    ) -> Result<(), ReviewRepositoryError> {
        Err(ReviewRepositoryError::ReadOnly)
    }

    fn load_completed(
        &self,
        _stream_id: ReviewStreamId,
        _round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError> {
        Ok(None)
    }

    fn publish(&self, _snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError> {
        Err(ReviewRepositoryError::ReadOnly)
    }
}
