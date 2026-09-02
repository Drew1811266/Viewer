use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::Arc,
};
use viewer_application::ReviewArtifactError as Error;

pub(super) struct Root {
    path: PathBuf,
    file: File,
}
impl Root {
    pub fn open(path: &Path) -> Result<Arc<Self>, Error> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| Error::UnsafeSource)?;
        let value = Arc::new(Self {
            path: fs::canonicalize(path).map_err(|_| Error::UnsafeSource)?,
            file,
        });
        value.verify()?;
        Ok(value)
    }
    fn verify(&self) -> Result<(), Error> {
        verify_path(&self.path, &self.file, true)
    }
}
pub(super) struct Scratch {
    root: Arc<Root>,
    name: CString,
    pub path: PathBuf,
    directory: File,
}
impl Scratch {
    pub fn new(root: Arc<Root>) -> Result<Self, Error> {
        root.verify()?;
        let name = CString::new(format!(".review-evidence-{}", uuid::Uuid::new_v4())).unwrap();
        // Descriptor-relative, exclusive, private scratch; never a source-media directory.
        if unsafe { libc::mkdirat(root.file.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            return Err(Error::Unavailable);
        }
        let directory = open_at(&root.file, &name, libc::O_RDONLY | libc::O_DIRECTORY)?;
        let value = Self {
            path: root.path.join(name.to_str().unwrap()),
            root,
            name,
            directory,
        };
        value.verify()?;
        Ok(value)
    }
    pub fn verify(&self) -> Result<(), Error> {
        self.root.verify()?;
        verify_path(&self.path, &self.directory, true)
    }
    pub fn create(&self, name: &str) -> Result<File, Error> {
        self.verify()?;
        open_at(
            &self.directory,
            &leaf(name)?,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )
    }
    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, Error> {
        let mut file = self.create(name)?;
        file.write_all(bytes).map_err(|_| Error::Unavailable)?;
        file.sync_all().map_err(|_| Error::Unavailable)?;
        self.verify()?;
        Ok(self.path.join(name))
    }
    pub fn encode(
        &self,
        name: &str,
        image: &objc2_core_graphics::CGImage,
        limit: u64,
    ) -> Result<super::super::encode::EncodedPng, Error> {
        self.verify()?;
        let file = self.create(name)?;
        let encoded = super::super::encode::encode_png_streaming(image, file, limit)
            .map_err(super::super::review_annotation::map_image_error)?;
        self.verify()?;
        Ok(encoded)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        for name in ["source.dat", "base.png", "annotated.png"] {
            let name = CString::new(name).unwrap();
            // Only our three fixed scratch entries, relative to the pinned directory.
            unsafe {
                libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0);
            }
        }
        if let Ok(current) = open_at(
            &self.root.file,
            &self.name,
            libc::O_RDONLY | libc::O_DIRECTORY,
        ) && let (Ok(a), Ok(b)) = (current.metadata(), self.directory.metadata())
            && a.dev() == b.dev()
            && a.ino() == b.ino()
        {
            unsafe {
                libc::unlinkat(
                    self.root.file.as_raw_fd(),
                    self.name.as_ptr(),
                    libc::AT_REMOVEDIR,
                );
            }
        }
    }
}
fn leaf(name: &str) -> Result<CString, Error> {
    if !matches!(name, "source.dat" | "base.png" | "annotated.png") {
        return Err(Error::InvalidRequest);
    }
    CString::new(name).map_err(|_| Error::InvalidRequest)
}
fn open_at(parent: &File, name: &CString, flags: i32) -> Result<File, Error> {
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(Error::UnsafeSource);
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn verify_path(path: &Path, file: &File, directory: bool) -> Result<(), Error> {
    let live = fs::symlink_metadata(path).map_err(|_| Error::UnsafeSource)?;
    let pinned = file.metadata().map_err(|_| Error::UnsafeSource)?;
    if live.file_type().is_symlink()
        || live.is_dir() != directory
        || live.dev() != pinned.dev()
        || live.ino() != pinned.ino()
    {
        return Err(Error::UnsafeSource);
    }
    Ok(())
}
pub(super) fn same_contents(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
