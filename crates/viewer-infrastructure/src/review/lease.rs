use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use viewer_application::ReviewRepositoryError;

pub(super) struct ProjectReviewLease {
    _file: File,
}

impl ProjectReviewLease {
    pub(super) fn acquire(path: &Path) -> Result<Self, ReviewRepositoryError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(map_open_error)?;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(Self { _file: file });
        }
        let error = io::Error::last_os_error();
        if error
            .raw_os_error()
            .is_some_and(|code| code == libc::EWOULDBLOCK || code == libc::EAGAIN)
        {
            Err(ReviewRepositoryError::Busy)
        } else {
            Err(ReviewRepositoryError::Unavailable)
        }
    }
}

fn map_open_error(error: io::Error) -> ReviewRepositoryError {
    if error.raw_os_error() == Some(libc::ELOOP) {
        ReviewRepositoryError::InvalidData
    } else {
        ReviewRepositoryError::Unavailable
    }
}
