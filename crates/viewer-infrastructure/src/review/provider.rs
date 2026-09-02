use super::{ProjectReviewRepository, ReviewRepositoryAccess};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use viewer_application::review_workspace::{
    ContinuousReviewRepositoryPort, ContinuousReviewRepositoryProviderPort, ReviewCommitError,
};
use viewer_application::{
    PersistedReviewDraft, ProjectAccess, ReviewCatalog, ReviewPublication, ReviewRepositoryError,
    ReviewRepositoryInspection, ReviewRepositoryPort, ReviewRepositoryProviderPort,
};
use viewer_domain::review::ReviewSnapshot;
use viewer_domain::{ProjectId, ReviewRoundId, ReviewStreamId};

pub struct ProjectReviewRepositoryProvider {
    project_root: PathBuf,
    project_id: ProjectId,
    project_access: ProjectAccess,
}

impl ProjectReviewRepositoryProvider {
    pub fn authoring_reader(
        &self,
    ) -> Result<Arc<super::SqliteContinuousReviewAuthoringStore>, ReviewCommitError> {
        Ok(Arc::new(super::SqliteContinuousReviewAuthoringStore::open(
            &self.project_root,
            self.project_id,
            ProjectAccess::ReadOnly,
        )?))
    }

    pub fn authoring_writer(
        &self,
    ) -> Result<Arc<super::SqliteContinuousReviewAuthoringStore>, ReviewCommitError> {
        if self.project_access != ProjectAccess::ReadWrite {
            return Err(ReviewCommitError::ReadOnly);
        }
        Ok(Arc::new(super::SqliteContinuousReviewAuthoringStore::open(
            &self.project_root,
            self.project_id,
            ProjectAccess::ReadWrite,
        )?))
    }

    /// Resolves the unique manual stream, or a stable empty context before the first commit.
    /// Never creates metadata or acquires a write lease.
    pub fn manual_review_stream(&self) -> Result<ReviewStreamId, ReviewCommitError> {
        Ok(
            super::continuous::manual_context::resolve(&self.project_root, self.project_id)?
                .unwrap_or_else(|| {
                    super::continuous::manual_context::empty_stream(self.project_id)
                }),
        )
    }

    pub fn migrate_with_faults(
        &self,
        request: viewer_application::review_workspace::MigrationCommitRequest,
        faults: Arc<dyn super::ReviewCommitFaultInjector>,
    ) -> Result<viewer_application::review_workspace::ReviewCommitReceipt, ReviewCommitError> {
        if self.project_access != ProjectAccess::ReadWrite {
            return Err(ReviewCommitError::ReadOnly);
        }
        super::continuous::migration::migrate(
            &self.project_root,
            self.project_id,
            request,
            Some(faults),
        )
    }
    pub fn continuous_writer_with_faults(
        &self,
        faults: Arc<dyn super::ReviewCommitFaultInjector>,
    ) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        if self.project_access != ProjectAccess::ReadWrite {
            return Err(ReviewCommitError::ReadOnly);
        }
        Ok(Arc::new(
            super::continuous::ContinuousReviewRepository::open_with_faults(
                &self.project_root,
                self.project_id,
                true,
                faults,
            )?,
        ))
    }

    pub fn continuous_reader(
        &self,
    ) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        Ok(Arc::new(
            super::continuous::ContinuousReviewRepository::open(
                &self.project_root,
                self.project_id,
                false,
            )?,
        ))
    }

    pub fn continuous_writer(
        &self,
    ) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        if self.project_access != ProjectAccess::ReadWrite {
            return Err(ReviewCommitError::ReadOnly);
        }
        Ok(Arc::new(
            super::continuous::ContinuousReviewRepository::open(
                &self.project_root,
                self.project_id,
                true,
            )?,
        ))
    }

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

impl ContinuousReviewRepositoryProviderPort for ProjectReviewRepositoryProvider {
    fn save_migration_recovery(
        &self,
        draft: &viewer_application::review_workspace::RecoveryDraft,
    ) -> Result<(), ReviewCommitError> {
        if self.project_access != ProjectAccess::ReadWrite {
            return Err(ReviewCommitError::ReadOnly);
        }
        super::continuous::migration_recovery::save(&self.project_root, self.project_id, draft)
    }
    fn load_migration_recovery(
        &self,
    ) -> Result<Vec<viewer_application::review_workspace::RecoveryDraft>, ReviewCommitError> {
        super::continuous::migration_recovery::load(&self.project_root, self.project_id)
    }
    fn inspect_migration(
        &self,
    ) -> Result<Option<viewer_application::review_workspace::MigrationInspection>, ReviewCommitError>
    {
        super::continuous::migration::inspect(&self.project_root, self.project_id)
    }
    fn migrate(
        &self,
        request: viewer_application::review_workspace::MigrationCommitRequest,
    ) -> Result<viewer_application::review_workspace::ReviewCommitReceipt, ReviewCommitError> {
        if self.project_access != ProjectAccess::ReadWrite {
            return Err(ReviewCommitError::ReadOnly);
        }
        super::continuous::migration::migrate(&self.project_root, self.project_id, request, None)
    }
    fn open_reader(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.continuous_reader()
    }
    fn open_writer(&self) -> Result<Arc<dyn ContinuousReviewRepositoryPort>, ReviewCommitError> {
        self.continuous_writer()
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

    fn load_active_draft(&self) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        Ok(None)
    }

    fn load_draft(
        &self,
        _stream_id: ReviewStreamId,
        _round_id: ReviewRoundId,
    ) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError> {
        Ok(None)
    }

    fn save_draft(&self, _draft: &PersistedReviewDraft) -> Result<(), ReviewRepositoryError> {
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

    fn publish(&self, _publication: &ReviewPublication) -> Result<(), ReviewRepositoryError> {
        Err(ReviewRepositoryError::ReadOnly)
    }
}
