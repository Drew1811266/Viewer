use std::{ffi::OsStr, path::Path};
use viewer_domain::file::FileKind;

const IMAGE_EXTENSIONS: &[&str] = &[
    "gif", "webp", "avif", "heic", "heif", "bmp", "tif", "tiff", "ico", "apng", "psd", "dng",
    "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f",
    "svg", "svgz", "ai",
];

const UNSUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["jxl"];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "webm", "avi", "wmv", "mpg", "mpeg", "ts", "mts", "m2ts", "flv",
    "ogv", "3gp",
];

pub(crate) fn classify_regular_file(path: &Path) -> Option<FileKind> {
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("jpg" | "jpeg" | "jfif") => Some(FileKind::Jpeg),
        Some("png") => Some(FileKind::Png),
        Some("md" | "markdown") => Some(FileKind::Markdown),
        Some("txt") => Some(FileKind::Text),
        Some(value) if IMAGE_EXTENSIONS.contains(&value) => Some(FileKind::Image),
        Some(value) if UNSUPPORTED_IMAGE_EXTENSIONS.contains(&value) => {
            Some(FileKind::UnsupportedImage)
        }
        Some(value) if VIDEO_EXTENSIONS.contains(&value) => Some(FileKind::Video),
        _ => Some(FileKind::Other),
    }
}

pub(crate) fn is_ignored_entry_name(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    let lowercase = name.to_ascii_lowercase();
    name.starts_with('.') || matches!(lowercase.as_str(), "thumbs.db" | "desktop.ini")
}

#[cfg(test)]
mod tests {
    use super::classify_regular_file;
    use std::path::Path;
    use viewer_domain::file::FileKind;

    #[test]
    fn common_local_video_extensions_are_candidates() {
        for name in [
            "a.mp4", "a.m4v", "a.mov", "a.mkv", "a.webm", "a.avi", "a.wmv", "a.mpg", "a.mpeg",
            "a.ts", "a.mts", "a.m2ts", "a.flv", "a.ogv", "a.3gp",
        ] {
            assert_eq!(
                classify_regular_file(Path::new(name)),
                Some(FileKind::Video),
                "classified {name} incorrectly"
            );
        }
    }

    #[test]
    fn video_candidate_extensions_follow_case_insensitive_classifier_convention() {
        assert_eq!(
            classify_regular_file(Path::new("CLIP.M2TS")),
            Some(FileKind::Video)
        );
    }

    #[test]
    fn imageio_matrix_extensions_classify_as_previewable_image() {
        for name in [
            "a.gif", "a.webp", "a.avif", "a.heic", "a.heif", "a.bmp", "a.tif", "a.tiff", "a.ico",
            "a.apng", "a.psd", "a.dng", "a.cr2", "a.cr3", "a.nef", "a.nrw", "a.arw", "a.srf",
            "a.sr2", "a.raf", "a.orf", "a.rw2", "a.pef", "a.srw", "a.x3f",
        ] {
            assert_eq!(
                classify_regular_file(Path::new(name)),
                Some(FileKind::Image),
                "classified {name} incorrectly"
            );
        }
    }

    #[test]
    fn jfif_classifies_as_jpeg_and_remains_case_insensitive() {
        assert_eq!(
            classify_regular_file(Path::new("photo.jfif")),
            Some(FileKind::Jpeg)
        );
        assert_eq!(
            classify_regular_file(Path::new("PHOTO.JFIF")),
            Some(FileKind::Jpeg)
        );
        assert_eq!(
            classify_regular_file(Path::new("vector.HEIC")),
            Some(FileKind::Image)
        );
    }

    #[test]
    fn vector_formats_are_previewable_images_and_jxl_stays_unsupported() {
        for name in ["a.svg", "a.svgz", "a.ai"] {
            assert_eq!(
                classify_regular_file(Path::new(name)),
                Some(FileKind::Image),
                "classified {name} incorrectly"
            );
        }
        assert_eq!(
            classify_regular_file(Path::new("a.jxl")),
            Some(FileKind::UnsupportedImage)
        );
    }
}
