use std::{ffi::OsStr, path::Path};
use viewer_domain::file::FileKind;

const UNSUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "gif", "webp", "avif", "heic", "heif", "bmp", "tif", "tiff", "svg", "svgz", "ico", "jxl",
    "jfif", "apng", "psd", "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf",
    "rw2", "pef", "srw", "x3f",
];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "avi", "mkv", "webm", "wmv", "flv", "mpeg", "mpg",
];

pub(crate) fn classify_regular_file(path: &Path) -> Option<FileKind> {
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("jpg" | "jpeg") => Some(FileKind::Jpeg),
        Some("png") => Some(FileKind::Png),
        Some("md" | "markdown") => Some(FileKind::Markdown),
        Some("txt") => Some(FileKind::Text),
        Some(value) if UNSUPPORTED_IMAGE_EXTENSIONS.contains(&value) => {
            Some(FileKind::UnsupportedImage)
        }
        Some(value) if VIDEO_EXTENSIONS.contains(&value) => None,
        _ => Some(FileKind::Other),
    }
}

pub(crate) fn is_ignored_entry_name(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    let lowercase = name.to_ascii_lowercase();
    name.starts_with('.') || matches!(lowercase.as_str(), "thumbs.db" | "desktop.ini")
}
