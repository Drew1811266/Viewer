use serde::{Deserialize, Serialize};

use crate::dto::SpeechLocaleDto;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeechCaptureRequest {
    pub locale: SpeechLocaleDto,
    pub auto_punctuation: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStatusDto {
    pub available: bool,
    pub speech_authorized: bool,
    pub microphone_authorized: bool,
    pub on_device: bool,
    pub locale_supported: bool,
    pub locale: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechPermissionsDto {
    pub speech_authorized: bool,
    pub microphone_authorized: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechTranscriptDto {
    pub text: String,
    pub elapsed_ms: u64,
    pub on_device: bool,
}

#[cfg(target_os = "macos")]
impl From<viewer_platform_macos::speech::SpeechStatus> for SpeechStatusDto {
    fn from(value: viewer_platform_macos::speech::SpeechStatus) -> Self {
        Self {
            available: value.available,
            speech_authorized: value.speech_authorized,
            microphone_authorized: value.microphone_authorized,
            on_device: value.on_device,
            locale_supported: value.locale_supported,
            locale: value.locale,
        }
    }
}

#[cfg(target_os = "macos")]
impl From<viewer_platform_macos::speech::SpeechPermissions> for SpeechPermissionsDto {
    fn from(value: viewer_platform_macos::speech::SpeechPermissions) -> Self {
        Self {
            speech_authorized: value.speech_authorized,
            microphone_authorized: value.microphone_authorized,
        }
    }
}

#[cfg(target_os = "macos")]
impl From<viewer_platform_macos::speech::SpeechTranscript> for SpeechTranscriptDto {
    fn from(value: viewer_platform_macos::speech::SpeechTranscript) -> Self {
        Self {
            text: value.text,
            elapsed_ms: value.elapsed_ms,
            on_device: value.on_device,
        }
    }
}
