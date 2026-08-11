use std::ffi::{c_char, c_int, c_ulong, c_void};

pub const MPV_CLIENT_API_MAJOR: c_ulong = 2;
pub const MPV_ERROR_PROPERTY_UNAVAILABLE: c_int = -10;
pub const MPV_FORMAT_STRING: MpvFormat = 1;
pub const MPV_FORMAT_FLAG: MpvFormat = 3;
pub const MPV_FORMAT_INT64: MpvFormat = 4;
pub const MPV_FORMAT_DOUBLE: MpvFormat = 5;

pub const MPV_RENDER_PARAM_INVALID: MpvRenderParamType = 0;
pub const MPV_RENDER_PARAM_API_TYPE: MpvRenderParamType = 1;
pub const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: MpvRenderParamType = 2;
pub const MPV_RENDER_PARAM_OPENGL_FBO: MpvRenderParamType = 3;
pub const MPV_RENDER_PARAM_FLIP_Y: MpvRenderParamType = 4;
pub const MPV_RENDER_UPDATE_FRAME: u64 = 1;

pub type MpvFormat = c_int;
pub type MpvRenderParamType = c_int;
pub type MpvRenderUpdateCallback = unsafe extern "C" fn(*mut c_void);
pub type MpvOpenGlGetProcAddress = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;

#[repr(C)]
pub struct MpvHandle {
    _private: [u8; 0],
}

#[repr(C)]
pub struct MpvRenderContextRaw {
    _private: [u8; 0],
}

#[repr(C)]
pub struct MpvEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct MpvRenderParam {
    pub param_type: MpvRenderParamType,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct MpvOpenGlInitParams {
    pub get_proc_address: Option<MpvOpenGlGetProcAddress>,
    pub get_proc_address_context: *mut c_void,
}

#[repr(C)]
pub struct MpvOpenGlFbo {
    pub framebuffer: c_int,
    pub width: c_int,
    pub height: c_int,
    pub internal_format: c_int,
}

#[derive(Clone, Copy)]
pub struct MpvApi {
    pub client_api_version: unsafe extern "C" fn() -> c_ulong,
    pub create: unsafe extern "C" fn() -> *mut MpvHandle,
    pub initialize: unsafe extern "C" fn(*mut MpvHandle) -> c_int,
    pub terminate_destroy: unsafe extern "C" fn(*mut MpvHandle),
    pub set_option_string:
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(*mut MpvHandle, *const *const c_char) -> c_int,
    pub set_property:
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, MpvFormat, *mut c_void) -> c_int,
    pub get_property:
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, MpvFormat, *mut c_void) -> c_int,
    pub free: unsafe extern "C" fn(*mut c_void),
    pub observe_property:
        unsafe extern "C" fn(*mut MpvHandle, u64, *const c_char, MpvFormat) -> c_int,
    pub wait_event: unsafe extern "C" fn(*mut MpvHandle, f64) -> *const MpvEvent,
}

#[derive(Clone, Copy)]
pub struct MpvRenderApi {
    pub create: unsafe extern "C" fn(
        *mut *mut MpvRenderContextRaw,
        *mut MpvHandle,
        *mut MpvRenderParam,
    ) -> c_int,
    pub free: unsafe extern "C" fn(*mut MpvRenderContextRaw),
    pub set_update_callback: unsafe extern "C" fn(
        *mut MpvRenderContextRaw,
        Option<MpvRenderUpdateCallback>,
        *mut c_void,
    ),
    pub update: unsafe extern "C" fn(*mut MpvRenderContextRaw) -> u64,
    pub render: unsafe extern "C" fn(*mut MpvRenderContextRaw, *mut MpvRenderParam) -> c_int,
    pub report_swap: unsafe extern "C" fn(*mut MpvRenderContextRaw),
}
