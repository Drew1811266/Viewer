//! Atomic path swaps for replace-in-place writes.

use std::{ffi::CString, io, os::unix::ffi::OsStrExt, path::Path};

/// Atomically exchange the entries at `left` and `right` with
/// `renamex_np(RENAME_SWAP)`. Both paths must exist and live on the same
/// volume. After the call, `left` holds the old `right` bytes and vice versa;
/// there is no moment in which either path is missing.
pub fn swap_paths(left: &Path, right: &Path) -> io::Result<()> {
    let left = CString::new(left.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let right = CString::new(right.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    // SAFETY: both pointers are valid NUL-terminated strings for the duration
    // of the call, and `renamex_np` does not retain them.
    let result = unsafe { libc::renamex_np(left.as_ptr(), right.as_ptr(), libc::RENAME_SWAP) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::swap_paths;
    use std::fs;

    #[test]
    fn swap_exchanges_both_entries() {
        let directory = tempfile::tempdir().unwrap();
        let left = directory.path().join("left");
        let right = directory.path().join("right");
        fs::write(&left, b"new").unwrap();
        fs::write(&right, b"old").unwrap();

        swap_paths(&left, &right).unwrap();

        assert_eq!(fs::read(&left).unwrap(), b"old");
        assert_eq!(fs::read(&right).unwrap(), b"new");
    }

    #[test]
    fn swap_requires_both_entries() {
        let directory = tempfile::tempdir().unwrap();
        let left = directory.path().join("left");
        let right = directory.path().join("missing");
        fs::write(&left, b"new").unwrap();

        assert!(swap_paths(&left, &right).is_err());
        assert_eq!(fs::read(&left).unwrap(), b"new");
    }
}
