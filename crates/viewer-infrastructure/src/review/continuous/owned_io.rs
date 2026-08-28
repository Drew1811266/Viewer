//! Descriptor-anchored repository IO. Callers only supply canonical single-component names.
use super::super::atomic::{leaf_name, open_at};
use std::ffi::{CStr, CString};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::os::{
    fd::{AsRawFd, IntoRawFd},
    unix::fs::{MetadataExt, OpenOptionsExt},
};
use std::path::{Path, PathBuf};
use viewer_application::review_workspace::ReviewCommitError;

pub(super) struct Directory {
    pub file: File,
    path: PathBuf,
}

impl Directory {
    pub fn entries(&self, limit: usize) -> Result<Vec<String>, ReviewCommitError> {
        self.verify()?;
        let names = descriptor_entries(&self.file, limit)?;
        self.verify()?;
        Ok(names)
    }

    /// Resolve caller-supplied root once, then anchor every open to a directory descriptor.
    pub fn open_anchored(path: &Path) -> Result<Self, ReviewCommitError> {
        let before = fs::symlink_metadata(path).map_err(map_io)?;
        if !before.is_dir() {
            return Err(ReviewCommitError::Integrity);
        }
        let canonical = fs::canonicalize(path).map_err(map_io)?;
        let mut directory = Self::open(Path::new("/"))?;
        for component in canonical.components() {
            if let std::path::Component::Normal(name) = component {
                directory =
                    directory.required_child(name.to_str().ok_or(ReviewCommitError::Integrity)?)?;
            }
        }
        if !same_identity(&before, &directory.file.metadata().map_err(map_io)?) {
            return Err(ReviewCommitError::Integrity);
        }
        Ok(directory)
    }

    /// The mutable publication pointer is deliberately distinct from immutable object reads.
    pub fn pin_index(&self) -> Result<Option<PinnedIndex<'_>>, ReviewCommitError> {
        self.scan_name("index.json")?;
        let file = match open_at(
            &self.file,
            &leaf_name("index.json").map_err(map_io)?,
            libc::O_RDONLY,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(map_io(error)),
        };
        let before = file.metadata().map_err(map_io)?;
        if !before.is_file() || before.nlink() > 1 {
            return Err(ReviewCommitError::Integrity);
        }
        // Atomic rename may already have unlinked this opened index. Never reopen the new head.
        if before.nlink() == 1 {
            self.verify_open_name(&file, "index.json")?;
        }
        self.verify()?;
        Ok(Some(PinnedIndex {
            directory: self,
            file,
            before,
        }))
    }
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
        self.verify_open_name(&file, name)?;
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
        self.verify_open_name(&file, name)?;
        self.verify()?;
        Ok(Some(file))
    }

    pub fn read(&self, name: &str, limit: u64) -> Result<Option<Vec<u8>>, ReviewCommitError> {
        let Some(mut file) = self.regular(name, false)? else {
            return Ok(None);
        };
        let before = file.metadata().map_err(map_io)?;
        if before.nlink() != 1 {
            return Err(ReviewCommitError::Integrity);
        }
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
        let after = file.metadata().map_err(map_io)?;
        if after.nlink() != 1
            || !same_contents(&before, &after)
            || before.len() != bytes.len() as u64
        {
            return Err(ReviewCommitError::Integrity);
        }
        self.verify()?;
        Ok(Some(bytes))
    }

    pub fn exact_name(&self, name: &str) -> Result<(), ReviewCommitError> {
        let leaf = leaf_name(name).map_err(map_io)?;
        self.verify()?;
        match open_at(&self.file, &leaf, libc::O_RDONLY) {
            Ok(file) => {
                self.verify_open_name(&file, name)?;
                return self.verify();
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(map_io(error)),
        }
        self.scan_name(name)
    }

    fn scan_name(&self, name: &str) -> Result<(), ReviewCommitError> {
        for observed in descriptor_entries(&self.file, 100_000)? {
            if observed.eq_ignore_ascii_case(name) && observed != name {
                return Err(ReviewCommitError::Integrity);
            }
        }
        self.verify()
    }

    fn verify_open_name(&self, file: &File, name: &str) -> Result<(), ReviewCommitError> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let observed = descriptor_path(file)?;
            if observed.file_name() != Some(std::ffi::OsStr::new(name)) {
                return Err(ReviewCommitError::Integrity);
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = file;
            self.scan_name(name)
        }
    }
}

pub(super) struct PinnedIndex<'a> {
    directory: &'a Directory,
    file: File,
    before: Metadata,
}

impl PinnedIndex<'_> {
    pub fn read(mut self, limit: u64) -> Result<Vec<u8>, ReviewCommitError> {
        if self.before.len() > limit {
            return Err(ReviewCommitError::LimitExceeded);
        }
        let mut bytes = Vec::with_capacity((self.before.len() as usize).min(64 * 1024));
        (&mut self.file)
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(map_io)?;
        if bytes.len() as u64 > limit {
            return Err(ReviewCommitError::LimitExceeded);
        }
        let after = self.file.metadata().map_err(map_io)?;
        let content_unchanged = same_identity(&self.before, &after)
            && self.before.len() == after.len()
            && self.before.len() == bytes.len() as u64
            && self.before.mtime() == after.mtime()
            && self.before.mtime_nsec() == after.mtime_nsec();
        if !content_unchanged
            || after.nlink() > 1
            || (after.nlink() != 0 && !same_contents(&self.before, &after))
        {
            return Err(ReviewCommitError::Integrity);
        }
        self.directory.verify()?;
        Ok(bytes)
    }
}

fn descriptor_entries(directory: &File, limit: usize) -> Result<Vec<String>, ReviewCommitError> {
    // openat(".") gives an independent cursor; dup would share offsets with concurrent enumeration.
    let descriptor = open_at(
        directory,
        &CString::new(".").unwrap(),
        libc::O_RDONLY | libc::O_DIRECTORY,
    )
    .map_err(map_io)?
    .into_raw_fd();
    let stream = unsafe { libc::fdopendir(descriptor) };
    if stream.is_null() {
        let error = io::Error::last_os_error();
        unsafe { libc::close(descriptor) };
        return Err(map_io(error));
    }
    struct Stream(*mut libc::DIR);
    impl Drop for Stream {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }
    let stream = Stream(stream);
    let mut names = vec![];
    loop {
        #[cfg(target_os = "macos")]
        let errno = unsafe { libc::__error() };
        #[cfg(not(target_os = "macos"))]
        let errno = unsafe { libc::__errno_location() };
        unsafe { *errno = 0 };
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let code = unsafe { *errno };
            if code != 0 {
                return Err(map_io(io::Error::from_raw_os_error(code)));
            }
            return Ok(names);
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if names.len() >= limit {
            return Err(ReviewCommitError::LimitExceeded);
        }
        names.push(
            std::str::from_utf8(name)
                .map_err(|_| ReviewCommitError::Integrity)?
                .to_owned(),
        );
    }
}

#[cfg(target_os = "macos")]
fn descriptor_path(file: &File) -> Result<PathBuf, ReviewCommitError> {
    use std::{
        ffi::{CStr, OsStr},
        os::unix::ffi::OsStrExt,
    };
    let mut path = vec![0_i8; libc::PATH_MAX as usize];
    // As in the file-operation adapter: resolve the live descriptor, never an untrusted link.
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, path.as_mut_ptr()) };
    if result < 0 {
        return Err(map_io(io::Error::last_os_error()));
    }
    let current = unsafe { CStr::from_ptr(path.as_ptr()) };
    Ok(PathBuf::from(OsStr::from_bytes(current.to_bytes())))
}

#[cfg(target_os = "linux")]
fn descriptor_path(file: &File) -> Result<PathBuf, ReviewCommitError> {
    fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).map_err(map_io)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opened_index_survives_atomic_publication_without_selecting_new_bytes() {
        let root = tempfile::TempDir::new().unwrap();
        fs::write(root.path().join("index.json"), b"old committed index").unwrap();
        let directory = Directory::open(root.path()).unwrap();
        let pinned = directory.pin_index().unwrap().unwrap();
        fs::write(root.path().join("replacement"), b"new committed index").unwrap();
        fs::rename(
            root.path().join("replacement"),
            root.path().join("index.json"),
        )
        .unwrap();
        assert_eq!(pinned.read(1024).unwrap(), b"old committed index");
        assert_eq!(
            directory.pin_index().unwrap().unwrap().read(1024).unwrap(),
            b"new committed index"
        );
    }

    #[test]
    fn pinned_index_rejects_in_place_mutation_and_size_before_allocation() {
        let root = tempfile::TempDir::new().unwrap();
        fs::write(root.path().join("index.json"), b"old").unwrap();
        let directory = Directory::open(root.path()).unwrap();
        let pinned = directory.pin_index().unwrap().unwrap();
        fs::write(root.path().join("index.json"), b"new and longer").unwrap();
        assert_eq!(pinned.read(1024), Err(ReviewCommitError::Integrity));
        assert_eq!(
            directory.pin_index().unwrap().unwrap().read(2),
            Err(ReviewCommitError::LimitExceeded)
        );
    }

    #[test]
    fn descriptor_enumeration_never_switches_to_replacement_directory() {
        let root = tempfile::TempDir::new().unwrap();
        let selected = root.path().join("selected");
        fs::create_dir(&selected).unwrap();
        fs::write(selected.join("owned"), b"owned").unwrap();
        let directory = Directory::open(&selected).unwrap();
        fs::rename(&selected, root.path().join("moved")).unwrap();
        fs::create_dir(&selected).unwrap();
        fs::write(selected.join("foreign"), b"foreign").unwrap();
        assert_eq!(
            descriptor_entries(&directory.file, 1).unwrap(),
            vec!["owned"]
        );
        assert_eq!(directory.entries(2), Err(ReviewCommitError::Integrity));
        assert!(directory.read("foreign", 100).is_err());
    }

    #[test]
    fn ancestor_replacement_cannot_redirect_an_already_open_project() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("parent/project");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("owned"), b"inside").unwrap();
        let project = Directory::open_anchored(&path).unwrap();
        fs::rename(temp.path().join("parent"), temp.path().join("old-parent")).unwrap();
        fs::create_dir_all(temp.path().join("outside/project")).unwrap();
        fs::write(temp.path().join("outside/project/owned"), b"outside").unwrap();
        std::os::unix::fs::symlink(temp.path().join("outside"), temp.path().join("parent"))
            .unwrap();
        assert!(project.read("owned", 100).is_err());
        let mut file =
            open_at(&project.file, &leaf_name("owned").unwrap(), libc::O_RDONLY).unwrap();
        let mut bytes = String::new();
        file.read_to_string(&mut bytes).unwrap();
        assert_eq!(bytes, "inside");
    }

    #[test]
    fn immutable_png_opened_before_replacement_is_not_treated_like_mutable_index() {
        let temp = tempfile::tempdir().unwrap();
        let source = viewer_test_support::image_fixtures::image_fixture("alpha.png");
        fs::copy(source, temp.path().join("image.png")).unwrap();
        let bytes = fs::read(temp.path().join("image.png")).unwrap();
        let directory = Directory::open(temp.path()).unwrap();
        let mut opened = directory.regular("image.png", false).unwrap().unwrap();
        fs::write(temp.path().join("replacement"), &bytes).unwrap();
        fs::rename(
            temp.path().join("replacement"),
            temp.path().join("image.png"),
        )
        .unwrap();
        let reference = crate::review::v3::EvidenceRef {
            blake3: *blake3::hash(&bytes).as_bytes(),
            size_bytes: bytes.len() as u64,
            width: 640,
            height: 480,
        };
        assert_eq!(
            crate::review::continuous::evidence::copy_png(&mut opened, &reference, &mut io::sink()),
            Err(ReviewCommitError::Integrity)
        );
    }

    #[test]
    fn descriptor_reads_reject_links_nonregular_files_and_excess_entries() {
        use std::os::unix::fs::symlink;
        let root = tempfile::TempDir::new().unwrap();
        fs::write(root.path().join("original"), b"data").unwrap();
        fs::hard_link(root.path().join("original"), root.path().join("hard")).unwrap();
        symlink(root.path().join("original"), root.path().join("link")).unwrap();
        fs::create_dir(root.path().join("folder")).unwrap();
        let directory = Directory::open(root.path()).unwrap();
        let fifo = leaf_name("fifo").unwrap();
        assert_eq!(
            unsafe { libc::mkfifoat(directory.file.as_raw_fd(), fifo.as_ptr(), 0o600) },
            0
        );
        for name in ["hard", "link", "folder", "fifo", "../original"] {
            assert!(directory.read(name, 100).is_err(), "{name}");
        }
        assert_eq!(directory.entries(1), Err(ReviewCommitError::LimitExceeded));
    }

    #[test]
    fn canonical_names_remain_required_for_descriptor_reads() {
        let root = tempfile::TempDir::new().unwrap();
        fs::write(root.path().join("State.json"), b"data").unwrap();
        let directory = Directory::open(root.path()).unwrap();
        assert!(directory.regular("state.json", false).is_err());
        assert_eq!(
            directory.read("State.json", 10).unwrap(),
            Some(b"data".to_vec())
        );
        fs::create_dir(root.path().join("States")).unwrap();
        assert!(directory.child("states", false).is_err());
        assert!(directory.child("States", false).unwrap().is_some());
    }
}
