use serde::{Deserialize, Serialize};
use viewer_application::ImageBackend;
use viewer_domain::file::ImageMetadata;

/// 解码优先级（内部 IPC DTO，非评审协议）。缺省 interactive；
/// 预取请求传 `prefetch`，由统一解码调度器排队，不与交互请求抢并发槽。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImagePriorityDto {
    #[default]
    Interactive,
    Prefetch,
}

impl From<ImagePriorityDto> for viewer_application::ImagePriority {
    fn from(priority: ImagePriorityDto) -> Self {
        match priority {
            ImagePriorityDto::Interactive => Self::Interactive,
            ImagePriorityDto::Prefetch => Self::Prefetch,
        }
    }
}

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
    pub fingerprint: String,
    pub uses_crlf: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTextResultDto {
    pub fingerprint: String,
    pub entity_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRepresentationRequestDto {
    Thumbnail {
        #[serde(rename = "maxPixels")]
        max_pixels: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
        #[serde(default)]
        priority: ImagePriorityDto,
    },
    FitPreview {
        #[serde(rename = "maxWidth")]
        max_width: u32,
        #[serde(rename = "maxHeight")]
        max_height: u32,
        #[serde(rename = "scaleMilli")]
        scale_milli: u16,
        #[serde(default)]
        priority: ImagePriorityDto,
    },
    Original100Percent {
        #[serde(default)]
        priority: ImagePriorityDto,
    },
}

impl ImageRepresentationRequestDto {
    pub fn priority(&self) -> ImagePriorityDto {
        match self {
            Self::Thumbnail { priority, .. }
            | Self::FitPreview { priority, .. }
            | Self::Original100Percent { priority } => *priority,
        }
    }
}

impl From<ImageRepresentationRequestDto> for viewer_domain::image::ImageRepresentationKind {
    fn from(request: ImageRepresentationRequestDto) -> Self {
        match request {
            ImageRepresentationRequestDto::Thumbnail {
                max_pixels,
                scale_milli,
                ..
            } => Self::Thumbnail {
                max_pixels,
                scale_milli,
            },
            ImageRepresentationRequestDto::FitPreview {
                max_width,
                max_height,
                scale_milli,
                ..
            } => Self::FitPreview {
                max_width,
                max_height,
                scale_milli,
            },
            ImageRepresentationRequestDto::Original100Percent { .. } => Self::Original100Percent,
        }
    }
}
