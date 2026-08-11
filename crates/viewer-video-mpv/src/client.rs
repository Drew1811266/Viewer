use std::{
    ffi::{CStr, CString, c_char, c_int, c_ulong, c_void},
    fs,
    path::Path,
    ptr::NonNull,
    sync::Arc,
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use thiserror::Error;

use crate::{
    ffi::{
        MPV_CLIENT_API_MAJOR, MPV_ERROR_PROPERTY_UNAVAILABLE, MPV_FORMAT_DOUBLE, MPV_FORMAT_FLAG,
        MPV_FORMAT_STRING, MpvApi, MpvHandle, MpvRenderApi,
    },
    loader::{LoadedLibrary, MpvLibrary},
};

const ISOLATION_OPTIONS: [(&str, &str); 17] = [
    ("vo", "libmpv"),
    ("hwdec", "auto-safe"),
    ("config", "no"),
    ("load-scripts", "no"),
    ("input-default-bindings", "no"),
    ("ytdl", "no"),
    ("autoload-files", "no"),
    ("access-references", "no"),
    ("load-unsafe-playlists", "no"),
    ("demuxer-lavf-o", "protocol_whitelist=%16%file,crypto,data"),
    ("sid", "no"),
    ("sub-auto", "no"),
    ("audio-file-auto", "no"),
    ("cover-art-auto", "no"),
    ("aid", "auto"),
    ("loop-file", "no"),
    ("network-timeout", "0"),
];

const COMPILE_TIME_REMOVED_OPTIONS: [&str; 2] = ["load-scripts", "ytdl"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackRate {
    Half,
    ThreeQuarters,
    Normal,
    FiveQuarters,
    OneAndHalf,
    Double,
}

impl PlaybackRate {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Half => 0.5,
            Self::ThreeQuarters => 0.75,
            Self::Normal => 1.0,
            Self::FiveQuarters => 1.25,
            Self::OneAndHalf => 1.5,
            Self::Double => 2.0,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MpvError {
    #[error("the bundled libmpv client API major version is unsupported: {actual}")]
    UnsupportedClientApi { actual: c_ulong },
    #[error("libmpv could not create a client")]
    CreateFailed,
    #[error("the media path must be absolute")]
    PathMustBeAbsolute,
    #[error("the media path is not an existing canonical path")]
    PathNotCanonical,
    #[error("the media path must identify a regular file")]
    NotRegularFile,
    #[error("the media path or value contains an interior NUL byte")]
    InteriorNul,
    #[error("volume must be between 0 and 100 percent")]
    InvalidVolume,
    #[error("libmpv rejected required isolation option {name} with code {code}")]
    IsolationOption { name: &'static str, code: c_int },
    #[error("libmpv operation {operation} failed with code {code}")]
    Api {
        operation: &'static str,
        code: c_int,
    },
}

pub(crate) struct ClientInner {
    handle: NonNull<MpvHandle>,
    api: MpvApi,
    _library: Option<Arc<LoadedLibrary>>,
}

// SAFETY: libmpv v0.41.0 documents the client API as fully thread-safe unless
// a specific function says otherwise. This owner uses only the general client
// functions in `MpvApi`; OpenGL render-context calls remain in `render.rs` and
// are not made through `ClientInner`.
unsafe impl Send for ClientInner {}
// SAFETY: see the `Send` justification above. Calls share only libmpv's
// thread-safe client handle and immutable function pointers/library ownership.
unsafe impl Sync for ClientInner {}

impl Drop for ClientInner {
    fn drop(&mut self) {
        unsafe { (self.api.terminate_destroy)(self.handle.as_ptr()) };
    }
}

pub struct MpvClient {
    inner: Arc<ClientInner>,
    render_api: Option<MpvRenderApi>,
    initialized: bool,
}

impl MpvClient {
    pub fn new(library: &MpvLibrary) -> Result<Self, MpvError> {
        unsafe {
            Self::from_parts(
                library.api(),
                Some(library.render_api()),
                Some(library.library()),
            )
        }
    }

    /// Constructs a client from an injected function table.
    ///
    /// # Safety
    /// Every function pointer must follow the libmpv v0.41.0 ABI and remain
    /// valid until the returned client is dropped.
    #[doc(hidden)]
    pub unsafe fn from_api(api: MpvApi) -> Result<Self, MpvError> {
        unsafe { Self::from_parts(api, None, None) }
    }

    unsafe fn from_parts(
        api: MpvApi,
        render_api: Option<MpvRenderApi>,
        library: Option<Arc<LoadedLibrary>>,
    ) -> Result<Self, MpvError> {
        let actual = unsafe { (api.client_api_version)() };
        if actual >> 16 != MPV_CLIENT_API_MAJOR {
            return Err(MpvError::UnsupportedClientApi { actual });
        }
        let handle = NonNull::new(unsafe { (api.create)() }).ok_or(MpvError::CreateFailed)?;
        Ok(Self {
            inner: Arc::new(ClientInner {
                handle,
                api,
                _library: library,
            }),
            render_api,
            initialized: false,
        })
    }

    pub fn open_local_file(&mut self, path: &Path) -> Result<(), MpvError> {
        validate_local_file(path)?;
        self.initialize_for_rendering()?;

        let media_path = path_to_c_string(path)?;
        self.run_command(&[
            CString::new("loadfile").expect("static string has no NUL"),
            media_path,
            CString::new("replace").expect("static string has no NUL"),
        ])
    }

    pub fn initialize_for_rendering(&mut self) -> Result<(), MpvError> {
        if !self.initialized {
            for (name, value) in ISOLATION_OPTIONS {
                self.set_option(name, value)?;
            }
            let result = unsafe { (self.inner.api.initialize)(self.inner.handle.as_ptr()) };
            ensure_success("initialize", result)?;
            self.initialized = true;
        }
        Ok(())
    }

    pub fn play(&self) -> Result<(), MpvError> {
        self.set_flag_property("pause", false)
    }

    pub fn pause(&self) -> Result<(), MpvError> {
        self.set_flag_property("pause", true)
    }

    pub fn seek_absolute_us(&self, time_us: u64) -> Result<(), MpvError> {
        let seconds = format!("{:.6}", time_us as f64 / 1_000_000.0);
        self.command_from_strings(&["seek", &seconds, "absolute+exact"])
    }

    pub fn frame_step(&self, direction: FrameDirection) -> Result<(), MpvError> {
        match direction {
            FrameDirection::Forward => self.command_from_strings(&["frame-step"]),
            FrameDirection::Backward => self.command_from_strings(&["frame-back-step"]),
        }
    }

    pub fn set_volume_percent(&self, volume: u8) -> Result<(), MpvError> {
        if volume > 100 {
            return Err(MpvError::InvalidVolume);
        }
        self.set_double_property("volume", f64::from(volume))
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), MpvError> {
        self.set_flag_property("mute", muted)
    }

    pub fn set_rate(&self, rate: PlaybackRate) -> Result<(), MpvError> {
        self.set_double_property("speed", rate.as_f64())
    }

    pub fn active_hardware_decoder(&self) -> Result<Option<String>, MpvError> {
        self.runtime_string_property("hwdec-current")
    }

    pub fn active_video_output(&self) -> Result<Option<String>, MpvError> {
        self.runtime_string_property("current-vo")
    }

    pub fn current_playback_time_us(&self) -> Result<Option<u64>, MpvError> {
        Ok(self
            .runtime_double_property("time-pos")?
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .map(|seconds| (seconds * 1_000_000.0).round() as u64))
    }

    /// Returns mpv's picture type for the currently decoded video frame.
    ///
    /// Unlike container metadata, `video-frame-info` is unavailable until mpv
    /// has an actual decoded frame. The feasibility surface uses this narrow
    /// typed read to avoid revealing a pre-video render-context redraw.
    pub fn current_video_picture_type(&self) -> Result<Option<String>, MpvError> {
        self.runtime_string_property("video-frame-info/picture-type")
    }

    fn set_option(&self, name: &'static str, value: &str) -> Result<(), MpvError> {
        let option_name = name;
        let name = CString::new(name).map_err(|_| MpvError::InteriorNul)?;
        let value = CString::new(value).map_err(|_| MpvError::InteriorNul)?;
        let result = unsafe {
            (self.inner.api.set_option_string)(
                self.inner.handle.as_ptr(),
                name.as_ptr(),
                value.as_ptr(),
            )
        };
        if result == -5 && COMPILE_TIME_REMOVED_OPTIONS.contains(&option_name) {
            Ok(())
        } else if result < 0 {
            Err(MpvError::IsolationOption {
                name: option_name,
                code: result,
            })
        } else {
            Ok(())
        }
    }

    fn set_flag_property(&self, name: &'static str, value: bool) -> Result<(), MpvError> {
        let name = CString::new(name).expect("static string has no NUL");
        let mut value: c_int = value.into();
        let result = unsafe {
            (self.inner.api.set_property)(
                self.inner.handle.as_ptr(),
                name.as_ptr(),
                MPV_FORMAT_FLAG,
                (&raw mut value).cast::<c_void>(),
            )
        };
        ensure_success("set_property", result)
    }

    fn set_double_property(&self, name: &'static str, mut value: f64) -> Result<(), MpvError> {
        let name = CString::new(name).expect("static string has no NUL");
        let result = unsafe {
            (self.inner.api.set_property)(
                self.inner.handle.as_ptr(),
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                (&raw mut value).cast::<c_void>(),
            )
        };
        ensure_success("set_property", result)
    }

    fn runtime_string_property(&self, name: &'static str) -> Result<Option<String>, MpvError> {
        let name = CString::new(name).expect("static string has no NUL");
        let mut value: *mut c_char = std::ptr::null_mut();
        let result = unsafe {
            (self.inner.api.get_property)(
                self.inner.handle.as_ptr(),
                name.as_ptr(),
                MPV_FORMAT_STRING,
                (&raw mut value).cast::<c_void>(),
            )
        };
        if result == MPV_ERROR_PROPERTY_UNAVAILABLE {
            return Ok(None);
        }
        ensure_success("get_property", result)?;
        let value = NonNull::new(value).ok_or(MpvError::Api {
            operation: "get_property",
            code: -2,
        })?;
        let property = unsafe { CStr::from_ptr(value.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        unsafe { (self.inner.api.free)(value.as_ptr().cast::<c_void>()) };
        Ok(Some(property))
    }

    fn runtime_double_property(&self, name: &'static str) -> Result<Option<f64>, MpvError> {
        let name = CString::new(name).expect("static string has no NUL");
        let mut value = 0.0;
        let result = unsafe {
            (self.inner.api.get_property)(
                self.inner.handle.as_ptr(),
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                (&raw mut value).cast::<c_void>(),
            )
        };
        if result == MPV_ERROR_PROPERTY_UNAVAILABLE {
            return Ok(None);
        }
        ensure_success("get_property", result)?;
        Ok(Some(value))
    }

    fn command_from_strings(&self, arguments: &[&str]) -> Result<(), MpvError> {
        let arguments = arguments
            .iter()
            .map(|argument| CString::new(*argument).map_err(|_| MpvError::InteriorNul))
            .collect::<Result<Vec<_>, _>>()?;
        self.run_command(&arguments)
    }

    fn run_command(&self, arguments: &[CString]) -> Result<(), MpvError> {
        let mut pointers = arguments
            .iter()
            .map(|argument| argument.as_ptr())
            .collect::<Vec<_>>();
        pointers.push(std::ptr::null());
        let result =
            unsafe { (self.inner.api.command)(self.inner.handle.as_ptr(), pointers.as_ptr()) };
        ensure_success("command", result)
    }

    pub(crate) fn raw_handle(&self) -> *mut MpvHandle {
        self.inner.handle.as_ptr()
    }

    pub(crate) fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub(crate) fn render_parts(&self) -> Option<(MpvRenderApi, Arc<ClientInner>)> {
        self.inner._library.as_ref()?;
        Some((self.render_api?, Arc::clone(&self.inner)))
    }
}

fn validate_local_file(path: &Path) -> Result<(), MpvError> {
    if !path.is_absolute() {
        return Err(MpvError::PathMustBeAbsolute);
    }
    let canonical = fs::canonicalize(path).map_err(|_| MpvError::PathNotCanonical)?;
    if canonical != path {
        return Err(MpvError::PathNotCanonical);
    }
    if !fs::metadata(path)
        .map_err(|_| MpvError::PathNotCanonical)?
        .is_file()
    {
        return Err(MpvError::NotRegularFile);
    }
    Ok(())
}

#[cfg(unix)]
fn path_to_c_string(path: &Path) -> Result<CString, MpvError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| MpvError::InteriorNul)
}

#[cfg(not(unix))]
fn path_to_c_string(path: &Path) -> Result<CString, MpvError> {
    let path = path.to_str().ok_or(MpvError::PathNotCanonical)?;
    CString::new(path).map_err(|_| MpvError::InteriorNul)
}

fn ensure_success(operation: &'static str, code: c_int) -> Result<(), MpvError> {
    if code < 0 {
        Err(MpvError::Api { operation, code })
    } else {
        Ok(())
    }
}
