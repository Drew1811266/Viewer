use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use viewer_application::{FileMutationPort, FileOperationError, TrashPort};
use viewer_domain::operation::ConflictPolicy;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictResult {
    Skipped,
    Placed(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum ConflictError {
    #[error(transparent)]
    File(#[from] FileOperationError),
    #[error(
        "previous destination was moved to the system Trash, but placement from {source:?} to {destination:?} failed: {cause}"
    )]
    PlacementAfterTrash {
        source: PathBuf,
        destination: PathBuf,
        #[source]
        cause: FileOperationError,
    },
}

pub struct ConflictExecutor {
    mutation: Arc<dyn FileMutationPort>,
    trash: Arc<dyn TrashPort>,
}

impl ConflictExecutor {
    pub fn new(mutation: Arc<dyn FileMutationPort>, trash: Arc<dyn TrashPort>) -> Self {
        Self { mutation, trash }
    }

    pub async fn place(
        &self,
        source: &Path,
        requested_destination: &Path,
        policy: ConflictPolicy,
    ) -> Result<ConflictResult, ConflictError> {
        if !requested_destination.exists() {
            self.mutation.rename(source, requested_destination).await?;
            return Ok(ConflictResult::Placed(requested_destination.to_path_buf()));
        }

        match policy {
            ConflictPolicy::Skip => Ok(ConflictResult::Skipped),
            ConflictPolicy::KeepBoth => {
                let destination = keep_both_destination(requested_destination)?;
                self.mutation.rename(source, &destination).await?;
                Ok(ConflictResult::Placed(destination))
            }
            ConflictPolicy::Replace => {
                self.trash.trash(requested_destination).await?;
                if let Err(cause) = self.mutation.rename(source, requested_destination).await {
                    return Err(ConflictError::PlacementAfterTrash {
                        source: source.to_path_buf(),
                        destination: requested_destination.to_path_buf(),
                        cause,
                    });
                }
                Ok(ConflictResult::Placed(requested_destination.to_path_buf()))
            }
        }
    }
}

pub fn keep_both_destination(destination: &Path) -> Result<PathBuf, FileOperationError> {
    let parent = destination
        .parent()
        .ok_or(FileOperationError::OutsideProject)?;
    let stem = destination
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or(FileOperationError::OutsideProject)?;
    let extension = destination.extension().and_then(|value| value.to_str());

    for index in 1_u32.. {
        let suffix = if index == 1 {
            " copy".to_owned()
        } else {
            format!(" copy {index}")
        };
        let file_name = match extension {
            Some(extension) => format!("{stem}{suffix}.{extension}"),
            None => format!("{stem}{suffix}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    unreachable!("u32 keep-both suffix space cannot be exhausted")
}
