use super::{
    Checkpoint, Hook, check,
    scratch::{Scratch, same_contents},
};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
};
use viewer_application::{
    PreparedReviewAsset, ReviewArtifactError as Error, ReviewTaskCancellation,
};

pub(super) struct CapturedSource {
    file: File,
    before: Metadata,
}
impl CapturedSource {
    pub fn copy(
        asset: &PreparedReviewAsset,
        scratch: &Scratch,
        cancellation: &ReviewTaskCancellation,
        hook: &Hook,
    ) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&asset.source_path)
            .map_err(|_| Error::UnsafeSource)?;
        let before = file.metadata().map_err(|_| Error::Unavailable)?;
        if !before.is_file()
            || before.len() != asset.asset.evidence.size_bytes
            || viewer_domain::EntityId::from_u128(
                (u128::from(before.dev()) << 64) | u128::from(before.ino()),
            ) != asset.entity_id
        {
            return Err(Error::SourceChanged);
        }
        let mut value = Self { file, before };
        // PreparedReviewAsset supplies the verified live locator; the historical
        // AssetVersion's path/entity/mtime stay unchanged after confirmed relocation.
        // Complete bytes are still required to match its captured content digest.
        value.verify(asset)?;
        check(cancellation, hook, Checkpoint::CaptureOpened)?;
        let mut output = scratch.create("source.dat")?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0; 64 * 1024];
        let mut total = 0_u64;
        loop {
            if cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let count = value
                .file
                .read(&mut buffer)
                .map_err(|_| Error::Unavailable)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .ok_or(Error::LimitExceeded)?;
            if total > value.before.len() {
                return Err(Error::SourceChanged);
            }
            hasher.update(&buffer[..count]);
            output
                .write_all(&buffer[..count])
                .map_err(|_| Error::Unavailable)?;
        }
        output.flush().map_err(|_| Error::Unavailable)?;
        if total != value.before.len()
            || Some(*hasher.finalize().as_bytes()) != asset.asset.evidence.blake3
        {
            return Err(Error::SourceChanged);
        }
        check(cancellation, hook, Checkpoint::CaptureCopied)?;
        value.verify(asset)?;
        scratch.verify()?;
        Ok(value)
    }
    pub fn verify(&self, asset: &PreparedReviewAsset) -> Result<(), Error> {
        let handle = self.file.metadata().map_err(|_| Error::SourceChanged)?;
        let live = fs::symlink_metadata(&asset.source_path).map_err(|_| Error::SourceChanged)?;
        if !same_contents(&self.before, &handle)
            || !same_contents(&handle, &live)
            || live.file_type().is_symlink()
            || !live.is_file()
        {
            return Err(Error::SourceChanged);
        }
        Ok(())
    }
}
