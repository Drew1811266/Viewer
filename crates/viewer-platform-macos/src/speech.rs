use std::{
    path::{Path, PathBuf},
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{AnyThread, runtime::Bool};
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use objc2_avf_audio::{AVAudioFormat, AVAudioRecorder};
use objc2_foundation::{NSLocale, NSString, NSURL};
use objc2_speech::{
    SFSpeechRecognitionResult, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
    SFSpeechURLRecognitionRequest,
};

const PERMISSION_TIMEOUT: Duration = Duration::from_secs(120);
const RECOGNITION_TIMEOUT: Duration = Duration::from_secs(30);
const MINIMUM_RECORDING: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeechPermissions {
    pub speech_authorized: bool,
    pub microphone_authorized: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechStatus {
    pub available: bool,
    pub speech_authorized: bool,
    pub microphone_authorized: bool,
    pub on_device: bool,
    pub locale: String,
    pub locale_supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechTranscript {
    pub text: String,
    pub elapsed_ms: u64,
    pub on_device: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SpeechError {
    #[error("speech recognition is not authorized")]
    SpeechNotAuthorized,
    #[error("microphone access is not authorized")]
    MicrophoneNotAuthorized,
    #[error("speech recognition is unavailable")]
    Unavailable,
    #[error("a dictation recording is already in progress")]
    AlreadyRecording,
    #[error("no dictation recording is in progress")]
    NotRecording,
    #[error("audio recording could not start")]
    RecordingFailed,
    #[error("speech recognition failed: {0}")]
    Recognition(String),
    #[error("no speech was recognized")]
    EmptyTranscript,
    #[error("speech recognition timed out")]
    Timeout,
}

struct Recording {
    recorder: Retained<AVAudioRecorder>,
    path: PathBuf,
    started_at: Instant,
    locale: String,
    punctuation: bool,
}

// The recorder is created, used and released behind this service's mutex.
unsafe impl Send for Recording {}

#[derive(Default)]
pub struct MacSpeechService {
    recording: Mutex<Option<Recording>>,
}

impl MacSpeechService {
    pub fn status(&self, locale: &str) -> SpeechStatus {
        let recognizer = recognizer_for(locale);
        let (available, on_device) = recognizer
            .as_ref()
            .map(|recognizer| {
                // SAFETY: The recognizer is a valid Speech framework instance.
                unsafe {
                    (
                        recognizer.isAvailable(),
                        recognizer.supportsOnDeviceRecognition(),
                    )
                }
            })
            .unwrap_or((false, false));
        SpeechStatus {
            available,
            speech_authorized: speech_authorized(),
            microphone_authorized: microphone_authorized(),
            on_device,
            locale: locale.to_string(),
            locale_supported: recognizer.is_some(),
        }
    }

    pub async fn request_permissions(&self) -> SpeechPermissions {
        if speech_status() == SFSpeechRecognizerAuthorizationStatus::NotDetermined {
            let _ = tokio::task::spawn_blocking(request_speech_authorization).await;
        }
        if microphone_status() == AVAuthorizationStatus::NotDetermined {
            let _ = tokio::task::spawn_blocking(request_microphone_access).await;
        }
        SpeechPermissions {
            speech_authorized: speech_authorized(),
            microphone_authorized: microphone_authorized(),
        }
    }

    pub fn start(&self, locale: &str, punctuation: bool) -> Result<(), SpeechError> {
        if !speech_authorized() {
            return Err(SpeechError::SpeechNotAuthorized);
        }
        if !microphone_authorized() {
            return Err(SpeechError::MicrophoneNotAuthorized);
        }
        let recognizer = recognizer_for(locale).ok_or(SpeechError::Unavailable)?;
        // SAFETY: The recognizer is a valid Speech framework instance.
        if !unsafe { recognizer.isAvailable() } {
            return Err(SpeechError::Unavailable);
        }
        let mut current = self
            .recording
            .lock()
            .map_err(|_| SpeechError::RecordingFailed)?;
        if current.is_some() {
            return Err(SpeechError::AlreadyRecording);
        }
        let path =
            std::env::temp_dir().join(format!("viewer-dictation-{}.wav", uuid::Uuid::new_v4()));
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        // SAFETY: The format and URL use their documented initializers.
        let recorder = unsafe {
            let format = AVAudioFormat::initStandardFormatWithSampleRate_channels(
                AVAudioFormat::alloc(),
                16_000.0,
                1,
            )
            .ok_or(SpeechError::RecordingFailed)?;
            AVAudioRecorder::initWithURL_format_error(AVAudioRecorder::alloc(), &url, &format)
                .map_err(|error| {
                    SpeechError::Recognition(error.localizedDescription().to_string())
                })?
        };
        // SAFETY: The recorder is prepared before recording begins.
        if !unsafe { recorder.prepareToRecord() } || !unsafe { recorder.record() } {
            let _ = std::fs::remove_file(&path);
            return Err(SpeechError::RecordingFailed);
        }
        *current = Some(Recording {
            recorder,
            path,
            started_at: Instant::now(),
            locale: locale.to_string(),
            punctuation,
        });
        Ok(())
    }

    pub async fn stop(&self) -> Result<SpeechTranscript, SpeechError> {
        let recording = self
            .recording
            .lock()
            .map_err(|_| SpeechError::NotRecording)?
            .take()
            .ok_or(SpeechError::NotRecording)?;
        // SAFETY: Stopping a recorder that was started above is documented.
        unsafe { recording.recorder.stop() };
        let elapsed = recording.started_at.elapsed();
        let path = recording.path.clone();
        let locale = recording.locale.clone();
        let punctuation = recording.punctuation;
        drop(recording);
        if elapsed < MINIMUM_RECORDING {
            let _ = std::fs::remove_file(&path);
            return Err(SpeechError::EmptyTranscript);
        }
        let result = tokio::task::spawn_blocking(move || {
            let bytes = std::fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            if bytes == 0 {
                return Err(SpeechError::EmptyTranscript);
            }
            let recognized = recognize_file(&path, &locale, punctuation);
            let _ = std::fs::remove_file(&path);
            recognized
        })
        .await
        .map_err(|error| SpeechError::Recognition(error.to_string()))?;
        let (text, on_device) = result?;
        Ok(SpeechTranscript {
            text,
            elapsed_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            on_device,
        })
    }

    pub fn cancel(&self) {
        let Ok(mut current) = self.recording.lock() else {
            return;
        };
        if let Some(recording) = current.take() {
            // SAFETY: The recorder belonged to this service and is stopped once.
            unsafe { recording.recorder.stop() };
            let _ = std::fs::remove_file(&recording.path);
        }
    }
}

fn request_speech_authorization() -> SFSpeechRecognizerAuthorizationStatus {
    let (sender, receiver) = mpsc::channel();
    let handler = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
        let _ = sender.send(status);
    });
    // SAFETY: The block matches the framework's callback signature.
    unsafe { SFSpeechRecognizer::requestAuthorization(&handler) };
    receiver
        .recv_timeout(PERMISSION_TIMEOUT)
        .unwrap_or(SFSpeechRecognizerAuthorizationStatus::NotDetermined)
}

fn request_microphone_access() -> bool {
    let Some(media_type) = media_type_audio() else {
        return false;
    };
    let (sender, receiver) = mpsc::channel();
    let handler = RcBlock::new(move |granted: Bool| {
        let _ = sender.send(granted.as_bool());
    });
    // SAFETY: The media type is the documented audio constant and the block
    // matches the completion handler signature.
    unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &handler) };
    receiver.recv_timeout(PERMISSION_TIMEOUT).unwrap_or(false)
}

fn recognize_file(
    path: &Path,
    locale: &str,
    punctuation: bool,
) -> Result<(String, bool), SpeechError> {
    let recognizer = recognizer_for(locale).ok_or(SpeechError::Unavailable)?;
    // SAFETY: The recognizer is a valid Speech framework instance.
    let on_device = unsafe { recognizer.supportsOnDeviceRecognition() };
    // Product privacy contract: dictation never leaves the machine. The UI
    // already hides the entry point when on-device recognition is missing;
    // enforce it here as well so no caller can silently fall back to the
    // network.
    if !on_device {
        return Err(SpeechError::Unavailable);
    }
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    // SAFETY: The request and recognizer use documented Speech framework setters.
    let request = unsafe {
        let request = SFSpeechURLRecognitionRequest::initWithURL(
            SFSpeechURLRecognitionRequest::alloc(),
            &url,
        );
        request.setShouldReportPartialResults(false);
        request.setRequiresOnDeviceRecognition(true);
        request.setAddsPunctuation(punctuation);
        request
    };
    let (sender, receiver) = mpsc::channel();
    let handler = RcBlock::new(
        move |result: *mut SFSpeechRecognitionResult, error: *mut objc2_foundation::NSError| {
            // SAFETY: The framework provides either a result or an error.
            let outcome = unsafe {
                if let Some(error) = error.as_ref() {
                    Some(Err(SpeechError::Recognition(
                        error.localizedDescription().to_string(),
                    )))
                } else {
                    result
                        .as_ref()
                        .filter(|result| result.isFinal())
                        .map(|result| Ok(result.bestTranscription().formattedString().to_string()))
                }
            };
            if let Some(outcome) = outcome {
                let _ = sender.send(outcome);
            }
        },
    );
    // SAFETY: The request is a URL recognition request owned by this scope.
    let task = unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler) };
    let outcome = receiver.recv_timeout(RECOGNITION_TIMEOUT);
    // SAFETY: Cancelling a finished task is a no-op for the framework.
    unsafe { task.cancel() };
    let text = match outcome {
        Ok(Ok(text)) => text,
        Ok(Err(error)) => return Err(error),
        Err(mpsc::RecvTimeoutError::Timeout) => return Err(SpeechError::Timeout),
        Err(mpsc::RecvTimeoutError::Disconnected) => return Err(SpeechError::Unavailable),
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(SpeechError::EmptyTranscript);
    }
    Ok((text, on_device))
}

fn recognizer_for(locale: &str) -> Option<Retained<SFSpeechRecognizer>> {
    let identifier = NSString::from_str(locale);
    let locale = NSLocale::localeWithLocaleIdentifier(&identifier);
    // SAFETY: The locale comes from the bounded speech locale enum.
    unsafe { SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale) }
}

fn speech_status() -> SFSpeechRecognizerAuthorizationStatus {
    // SAFETY: Reading the current authorization status has no preconditions.
    unsafe { SFSpeechRecognizer::authorizationStatus() }
}

fn speech_authorized() -> bool {
    speech_status() == SFSpeechRecognizerAuthorizationStatus::Authorized
}

fn microphone_status() -> AVAuthorizationStatus {
    media_type_audio()
        .map(|media_type| {
            // SAFETY: The media type is the documented audio constant.
            unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) }
        })
        .unwrap_or(AVAuthorizationStatus::Restricted)
}

fn microphone_authorized() -> bool {
    microphone_status() == AVAuthorizationStatus::Authorized
}

fn media_type_audio() -> Option<&'static objc2_av_foundation::AVMediaType> {
    // SAFETY: AVFoundation exports this media type for the process lifetime.
    unsafe { AVMediaTypeAudio }
}

#[cfg(test)]
mod tests {
    use super::{MacSpeechService, SpeechError};

    #[test]
    fn stop_without_a_recording_is_reported_explicitly() {
        let service = MacSpeechService::default();
        let error = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime")
            .block_on(service.stop())
            .expect_err("missing recording");
        assert_eq!(error, SpeechError::NotRecording);
    }

    #[test]
    fn cancel_without_a_recording_is_a_no_op() {
        let service = MacSpeechService::default();
        service.cancel();
        assert_eq!(service.status("zh-CN").locale, "zh-CN");
    }
}
