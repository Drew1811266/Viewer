use crate::{EntityId, RelativePath};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Directory,
    Jpeg,
    Png,
    Markdown,
    Text,
    UnsupportedImage,
    Other,
}

impl FileKind {
    pub const fn is_image(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png | Self::UnsupportedImage)
    }

    pub const fn is_previewable_image(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png)
    }

    pub const fn is_other_file(self) -> bool {
        matches!(self, Self::Markdown | Self::Text | Self::Other)
    }

    pub const fn is_previewable_text(self) -> bool {
        matches!(self, Self::Markdown | Self::Text)
    }
}

#[cfg(test)]
mod tests {
    use super::FileKind;

    #[test]
    fn file_kinds_expose_behavior_without_extension_logic() {
        assert!(FileKind::Jpeg.is_image());
        assert!(FileKind::Png.is_previewable_image());
        assert!(FileKind::UnsupportedImage.is_image());
        assert!(!FileKind::UnsupportedImage.is_previewable_image());
        assert!(FileKind::Markdown.is_other_file());
        assert!(FileKind::Text.is_previewable_text());
        assert!(FileKind::Other.is_other_file());
        assert!(!FileKind::Other.is_previewable_text());
        assert!(!FileKind::Directory.is_other_file());
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Keep,
    Pending,
    Reject,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub review_state: Option<ReviewState>,
    pub favorite: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageIndexStatus {
    Pending,
    Ready,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextIndexStatus {
    Pending,
    Ready,
    UnsupportedEncoding,
    TooLarge,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub entity_id: EntityId,
    pub relative_path: RelativePath,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ns: i128,
}
