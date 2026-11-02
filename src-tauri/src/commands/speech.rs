use std::sync::Arc;

use tauri::State;

use crate::{
    dto::{SpeechCaptureRequest, SpeechPermissionsDto, SpeechStatusDto, SpeechTranscriptDto},
    error::{CommandError, ErrorCategory},
};

#[cfg(target_os = "macos")]
use viewer_platform_macos::speech::{MacSpeechService, SpeechError};

type SpeechState = Arc<MacSpeechService>;

#[tauri::command]
pub fn speech_status(
    request: SpeechCaptureRequest,
    service: State<'_, SpeechState>,
) -> SpeechStatusDto {
    service.status(request.locale.bcp47()).into()
}

#[tauri::command]
pub async fn request_speech_permissions(
    service: State<'_, SpeechState>,
) -> Result<SpeechPermissionsDto, CommandError> {
    Ok(service.request_permissions().await.into())
}

#[tauri::command]
pub fn start_speech_capture(
    request: SpeechCaptureRequest,
    service: State<'_, SpeechState>,
) -> Result<SpeechStatusDto, CommandError> {
    service
        .start(request.locale.bcp47(), request.auto_punctuation)
        .map_err(speech_error)?;
    Ok(service.status(request.locale.bcp47()).into())
}

#[tauri::command]
pub async fn stop_speech_capture(
    service: State<'_, SpeechState>,
) -> Result<SpeechTranscriptDto, CommandError> {
    service.stop().await.map(Into::into).map_err(speech_error)
}

#[tauri::command]
pub fn cancel_speech_capture(service: State<'_, SpeechState>) {
    service.cancel();
}

#[tauri::command]
pub async fn open_speech_settings() -> Result<(), CommandError> {
    tokio::task::spawn_blocking(viewer_platform_macos::open_privacy_security_settings)
        .await
        .map_err(|_| settings_unavailable())?
        .map_err(|_| settings_unavailable())
}

fn settings_unavailable() -> CommandError {
    CommandError::new(
        "speech_settings_unavailable",
        ErrorCategory::Environment,
        "无法打开系统设置，请手动打开“系统设置 > 隐私与安全性 > 麦克风”。",
        true,
    )
}

#[cfg(target_os = "macos")]
fn speech_error(error: SpeechError) -> CommandError {
    let (code, message, retryable) = match error {
        SpeechError::SpeechNotAuthorized => (
            "speech_not_authorized",
            "尚未允许语音识别，请在系统设置中允许 Viewer 使用语音识别。",
            false,
        ),
        SpeechError::MicrophoneNotAuthorized => (
            "microphone_not_authorized",
            "尚未允许麦克风，请在系统设置中允许 Viewer 使用麦克风。",
            false,
        ),
        SpeechError::Unavailable => (
            "speech_unavailable",
            "当前无法使用本地语音识别，请稍后重试。",
            true,
        ),
        SpeechError::AlreadyRecording => (
            "speech_busy",
            "已有一段语音正在转写，请先停止或取消。",
            false,
        ),
        SpeechError::NotRecording => (
            "speech_not_recording",
            "当前没有正在进行的语音输入。",
            false,
        ),
        SpeechError::RecordingFailed => (
            "speech_recording_failed",
            "无法开始录音，请检查麦克风后重试。",
            true,
        ),
        SpeechError::Recognition(message) => {
            return CommandError::new(
                "speech_recognition_failed",
                ErrorCategory::Environment,
                &format!("语音转写失败：{message}"),
                true,
            );
        }
        SpeechError::EmptyTranscript => (
            "speech_empty_transcript",
            "没有识别到语音，请靠近麦克风后重试。",
            true,
        ),
        SpeechError::Timeout => ("speech_timeout", "语音转写超时，请缩短内容后重试。", true),
    };
    CommandError::new(code, ErrorCategory::Environment, message, retryable)
}
