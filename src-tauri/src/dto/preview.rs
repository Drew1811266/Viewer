use serde::{Deserialize, Serialize};
use viewer_application::ImageBackend;
use viewer_domain::file::ImageMetadata;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMetadataDto {
    pub width: u32,
    pub height: u32,
}

impl From<ImageMetadata> for ImageMetadataDto {
    fn from(metadata: ImageMetadata) -> Self {
        Self {
            width: metadata.width,
            height: metadata.height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageBackendDto {
    QuickLook,
    ImageIo,
}

impl From<ImageBackend> for ImageBackendDto {
    fn from(backend: ImageBackend) -> Self {
        match backend {
            ImageBackend::QuickLook => Self::QuickLook,
            ImageBackend::ImageIo => Self::ImageIo,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRepresentationDto {
    pub cache_key: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub backend: ImageBackendDto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextPreviewFormatDto {
    PlainText,
    Markdown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreviewDto {
    pub entity_id: String,
    pub format: TextPreviewFormatDto,
    pub plain_text: Option<String>,
    pub markdown_html: Option<String>,
    pub encoding: viewer_application::TextEncoding,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRepresentationRequestDto {
    Thumbnail {
        #[serde(rename = "maxPixels")]
        max_pixels: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
    },
    FitPreview {
        #[serde(rename = "maxWidth")]
        max_width: u32,
        #[serde(rename = "maxHeight")]
        max_height: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
    },
    Original100Percent,
}

impl From<ImageRepresentationRequestDto> for viewer_domain::image::ImageRepresentationKind {
    fn from(request: ImageRepresentationRequestDto) -> Self {
        match request {
            ImageRepresentationRequestDto::Thumbnail {
                max_pixels,
                scale_milli,
            } => Self::Thumbnail {
                max_pixels,
                scale_milli,
            },
            ImageRepresentationRequestDto::FitPreview {
                max_width,
                max_height,
                scale_milli,
            } => Self::FitPreview {
                max_width,
                max_height,
                scale_milli,
            },
            ImageRepresentationRequestDto::Original100Percent => Self::Original100Percent,
        }
    }
}
