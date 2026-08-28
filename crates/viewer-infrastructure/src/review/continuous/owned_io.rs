//! Descriptor-anchored repository IO. Callers only supply canonical single-component names.
use super::super::atomic::{leaf_name, open_at};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::os::{
    fd::AsRawFd,
    unix::fs::{MetadataExt, OpenOptionsExt},
};
use std::path::{Path, PathBuf};
use viewer_application::review_workspace::ReviewCommitError;

pub(super) struct Directory {
    pub file: File,
    path: PathBuf,
}

impl Directory {
    pub fn open(path: &Path) -> Result<Self, ReviewCommitError> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(map_io)?;
        let directory = Self {
            file,
            path: path.to_path_buf(),
        };
        directory.verify()?;
        Ok(directory)
    }

    pub fn verify(&self) -> Result<(), ReviewCommitError> {
        let live = fs::symlink_metadata(&self.path).map_err(map_io)?;
        let pinned = self.file.metadata().map_err(map_io)?;
        if !live.is_dir() || !same_identity(&live, &pinned) {
            return Err(ReviewCommitError::Integrity);
        }
        Ok(())
    }

    pub fn child(&self, name: &str, create: bool) -> Result<Option<Self>, ReviewCommitError> {
        self.exact_name(name)?;
        let leaf = leaf_name(name).map_err(map_io)?;
        if create {
            let result = unsafe { libc::mkdirat(self.file.as_raw_fd(), leaf.as_ptr(), 0o700) };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::AlreadyExists {
                    return Err(map_io(error));
                }
            } else {
                self.file.sync_all().map_err(map_io)?;
            }
        }
        let file = match open_at(&self.file, &leaf, libc::O_RDONLY | libc::O_DIRECTORY) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
            Err(error) => return Err(map_io(error)),
        };
        self.verify()?;
        let child = Self {
            file,
            path: self.path.join(name),
        };
        child.verify()?;
        Ok(Some(child))
    }

    pub fn required_child(&self, name: &str) -> Result<Self, ReviewCommitError> {
        self.child(name, false)?.ok_or(ReviewCommitError::Integrity)
    }

    pub fn regular(&self, name: &str, create: bool) -> Result<Option<File>, ReviewCommitError> {
        self.exact_name(name)?;
        let flags = if create {
            libc::O_RDWR | libc::O_CREAT
        } else {
            libc::O_RDONLY
        };
        let file = match open_at(&self.file, &leaf_name(name).map_err(map_io)?, flags) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound && !create => return Ok(None),
            Err(error) => return Err(map_io(error)),
        };
        let metadata = file.metadata().map_err(map_io)?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(ReviewCommitError::Integrity);
        }
        self.verify()?;
        Ok(Some(file))
    }

    pub fn read(&self, name: &str, limit: u64) -> Result<Option<Vec<u8>>, ReviewCommitError> {
        let Some(mut file) = self.regular(name, false)? else {
            return Ok(None);
        };
        let before = file.metadata().map_err(map_io)?;
        if before.len() > limit {
            return Err(ReviewCommitError::LimitExceeded);
        }
        let mut bytes = Vec::with_capacity((before.len() as usize).min(64 * 1024));
        (&mut file)
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(map_io)?;
        if bytes.len() as u64 > limit {
            return Err(ReviewCommitError::LimitExceeded);
        }
        if !same_contents(&before, &file.metadata().map_err(map_io)?)
            || before.len() != bytes.len() as u64
        {
            return Err(ReviewCommitError::Integrity);
        }
        self.verify()?;
        Ok(Some(bytes))
    }

    pub fn exact_name(&self, name: &str) -> Result<(), ReviewCommitError> {
        leaf_name(name).map_err(map_io)?;
        self.verify()?;
        for entry in fs::read_dir(&self.path).map_err(map_io)? {
            let entry = entry.map_err(map_io)?;
            let observed = entry.file_name();
            if observed.to_string_lossy().eq_ignore_ascii_case(name) && observed != name {
                return Err(ReviewCommitError::Integrity);
            }
        }
        self.verify()
    }
}

pub(super) fn same_identity(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

pub(super) fn same_contents(a: &Metadata, b: &Metadata) -> bool {
    same_identity(a, b)
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

pub(super) fn map_io(error: io::Error) -> ReviewCommitError {
    match error.raw_os_error() {
        Some(libc::ELOOP | libc::ENOTDIR) => ReviewCommitError::Integrity,
        _ if matches!(
            error.kind(),
            io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData
        ) =>
        {
            ReviewCommitError::Integrity
        }
        _ => ReviewCommitError::Io,
    }
}
