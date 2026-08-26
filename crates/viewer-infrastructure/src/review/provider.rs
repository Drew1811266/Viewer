use super::{ProjectReviewRepository, ReviewRepositoryAccess};
use std::path::{Path, PathBuf};
use viewer_application::{
    ReviewRepositoryError, ReviewRepositoryInspection, ReviewRepositoryPort,
    ReviewRepositoryProviderPort,
};
use viewer_domain::ProjectId;

pub struct ProjectReviewRepositoryProvider {
    project_root: PathBuf,
    project_id: ProjectId,
}

impl ProjectReviewRepositoryProvider {
    pub fn new(project_root: &Path, project_id: ProjectId) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            project_id,
        }
    }

    fn open(
        &self,
        access: ReviewRepositoryAccess,
    ) -> Result<ProjectReviewRepository, ReviewRepositoryError> {
        ProjectReviewRepository::open(&self.project_root, self.project_id, access)
    }
}

impl ReviewRepositoryProviderPort for ProjectReviewRepositoryProvider {
    fn inspect(&self) -> Result<ReviewRepositoryInspection, ReviewRepositoryError> {
        let repository = self.open(ReviewRepositoryAccess::ReadOnly)?;
        Ok(ReviewRepositoryInspection {
            catalog: repository.load_catalog()?,
            active_draft: repository.load_active_draft()?,
        })
    }

    fn open_reader(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        self.open(ReviewRepositoryAccess::ReadOnly)
            .map(|repository| Box::new(repository) as Box<dyn ReviewRepositoryPort>)
    }

    fn open_writer(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError> {
        self.open(ReviewRepositoryAccess::ReadWrite)
            .map(|repository| Box::new(repository) as Box<dyn ReviewRepositoryPort>)
    }
}
