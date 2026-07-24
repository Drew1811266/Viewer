use std::{fs::File, path::Path};
use viewer_application::FileOperationError;

pub(crate) fn sync_parent(path: &Path) -> Result<(), FileOperationError> {
    let parent = path.parent().ok_or(FileOperationError::OutsideProject)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| FileOperationError::io("sync containing directory", parent, &error))
}
