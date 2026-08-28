use super::{owned_io::Directory, usage_source};
use crate::review::{ReviewProtocolError, v3};
use std::{collections::HashSet, path::Path, sync::Arc};
use viewer_application::review_workspace::*;
use viewer_domain::{ProjectId, RelativePath, ReviewStreamId};

pub struct ProjectUsageImporter {
    root: Directory,
    project_id: ProjectId,
    stream_id: ReviewStreamId,
    repository: Arc<dyn ContinuousReviewRepositoryPort>,
}
impl ProjectUsageImporter {
    pub fn new(
        root: &Path,
        project_id: ProjectId,
        stream_id: ReviewStreamId,
        repository: Arc<dyn ContinuousReviewRepositoryPort>,
    ) -> Result<Self, UsageImportError> {
        Ok(Self {
            root: Directory::open(root).map_err(usage_source::map)?,
            project_id,
            stream_id,
            repository,
        })
    }
}
impl UsageImportPort for ProjectUsageImporter {
    fn inspect(&self, source: &RelativePath) -> Result<UsageImportPreview, UsageImportError> {
        let (bytes, source_digest) = usage_source::read(&self.root, source, true)?;
        let record = v3::decode_usage_v1(&bytes).map_err(protocol)?;
        if record.project_id != self.project_id || record.review_stream_id != self.stream_id {
            return Err(UsageImportError::WrongContext);
        }
        let canonical = v3::encode_usage_v1(&record).map_err(protocol)?;
        let canonical_digest = *blake3::hash(&canonical).as_bytes();
        let basis = self
            .repository
            .load_snapshot(self.stream_id, &record.basis)
            .map_err(|_| UsageImportError::UnknownBasis)?;
        let keys = super::references::target_keys(&basis.state);
        if record.targets.iter().any(|k| !keys.contains(k)) {
            return Err(UsageImportError::InvalidScope);
        }
        if let Some(existing) = self
            .repository
            .load_usage(self.stream_id, record.declaration_id)
            .map_err(|_| UsageImportError::Conflict)?
        {
            let existing = v3::encode_usage_v1(&existing.into()).map_err(protocol)?;
            if existing != canonical {
                return Err(UsageImportError::Conflict);
            }
        }
        let assets: HashSet<_> = basis.state.assets.iter().map(|a| a.id).collect();
        let declaration: ReviewUsageDeclaration = record.into();
        let mut outputs = vec![];
        for output in &declaration.outputs {
            let status = if !assets.contains(&output.previous_asset_version_id) {
                UsageOutputStatus::UnknownPreviousAsset
            } else {
                match usage_source::read(&self.root, &output.relative_path, false) {
                    Ok((_, digest)) if digest == output.blake3 => {
                        UsageOutputStatus::VerifiedCandidate
                    }
                    Ok(_) => UsageOutputStatus::Changed,
                    Err(UsageImportError::UnsafePath) => UsageOutputStatus::Unsafe,
                    Err(UsageImportError::SourceChanged) => UsageOutputStatus::Changed,
                    Err(_) => UsageOutputStatus::Unreadable,
                }
            };
            outputs.push(UsageOutputCheck {
                output: output.clone(),
                status,
            });
        }
        Ok(UsageImportPreview {
            declaration,
            canonical_digest,
            source: source.clone(),
            source_digest,
            outputs,
        })
    }
}
fn protocol(error: ReviewProtocolError) -> UsageImportError {
    match error {
        ReviewProtocolError::LimitExceeded => UsageImportError::LimitExceeded,
        _ => UsageImportError::InvalidDeclaration,
    }
}
