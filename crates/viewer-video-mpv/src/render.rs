use std::{
    ffi::{c_char, c_int, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use thiserror::Error;

use crate::{
    client::{ClientInner, MpvClient},
    ffi::{
        MPV_RENDER_PARAM_API_TYPE, MPV_RENDER_PARAM_FLIP_Y, MPV_RENDER_PARAM_INVALID,
        MPV_RENDER_PARAM_OPENGL_FBO, MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, MPV_RENDER_UPDATE_FRAME,
        MpvOpenGlFbo, MpvOpenGlGetProcAddress, MpvOpenGlInitParams, MpvRenderApi,
        MpvRenderContextRaw, MpvRenderParam,
    },
};

static OPENGL_API: &[u8] = b"opengl\0";

pub trait RenderTarget {
    fn framebuffer(&self) -> i32;
    fn pixel_size(&self) -> (i32, i32);
    fn scale_factor(&self) -> f64;
}

#[derive(Clone, Copy)]
pub struct OpenGlInit {
    pub get_proc_address: MpvOpenGlGetProcAddress,
    pub context: *mut c_void,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MpvRenderError {
    #[error("the libmpv client must be initialized before creating a render context")]
    ClientNotInitialized,
    #[error("an injected client has no bundled render API")]
    RenderApiUnavailable,
    #[error("libmpv could not create the render context: {0}")]
    Create(c_int),
    #[error("the render target has invalid dimensions or scale")]
    InvalidTarget,
    #[error("libmpv could not render the current frame: {0}")]
    Render(c_int),
}

struct WakeState {
    pending: AtomicBool,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl WakeState {
    fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            pending: AtomicBool::new(false),
            wake,
        }
    }

    fn notify(&self) {
        if !self.pending.swap(true, Ordering::AcqRel) {
            let _ = catch_unwind(AssertUnwindSafe(|| (self.wake)()));
        }
    }

    fn consume(&self) {
        self.pending.store(false, Ordering::Release);
    }
}

unsafe extern "C" fn update_callback(context: *mut c_void) {
    if let Some(state) = NonNull::new(context.cast::<WakeState>()) {
        unsafe { state.as_ref() }.notify();
    }
}

pub struct MpvRenderContext {
    context: NonNull<MpvRenderContextRaw>,
    api: MpvRenderApi,
    wake: Box<WakeState>,
    _client: Arc<ClientInner>,
}

impl MpvRenderContext {
    /// Creates an OpenGL render context backed by the bundled libmpv runtime.
    ///
    /// # Safety
    /// `init.context` and the function pointers returned by
    /// `init.get_proc_address` must remain valid until this context is dropped.
    /// Calls to `update`, `render`, `report_swap`, and `drop` must follow the
    /// thread and current-OpenGL-context rules documented by libmpv.
    pub unsafe fn new_opengl(
        client: &MpvClient,
        init: OpenGlInit,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, MpvRenderError> {
        if !client.is_initialized() {
            return Err(MpvRenderError::ClientNotInitialized);
        }
        let (api, client_owner) = client
            .render_parts()
            .ok_or(MpvRenderError::RenderApiUnavailable)?;
        let mut init_params = MpvOpenGlInitParams {
            get_proc_address: Some(init.get_proc_address),
            get_proc_address_context: init.context,
        };
        let mut params = [
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_API_TYPE,
                data: OPENGL_API.as_ptr().cast::<c_char>() as *mut c_void,
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&raw mut init_params).cast::<c_void>(),
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        let mut raw_context = std::ptr::null_mut();
        let result = unsafe {
            (api.create)(
                &raw mut raw_context,
                client.raw_handle(),
                params.as_mut_ptr(),
            )
        };
        if result < 0 {
            return Err(MpvRenderError::Create(result));
        }
        let context = NonNull::new(raw_context).ok_or(MpvRenderError::Create(-1))?;
        let mut wake = Box::new(WakeState::new(wake));
        unsafe {
            (api.set_update_callback)(
                context.as_ptr(),
                Some(update_callback),
                (&raw mut *wake).cast::<c_void>(),
            );
        }
        Ok(Self {
            context,
            api,
            wake,
            _client: client_owner,
        })
    }

    pub fn update(&self) -> bool {
        self.wake.consume();
        let flags = unsafe { (self.api.update)(self.context.as_ptr()) };
        flags & MPV_RENDER_UPDATE_FRAME != 0
    }

    pub fn render(&mut self, target: &impl RenderTarget) -> Result<(), MpvRenderError> {
        let (width, height) = target.pixel_size();
        let scale = target.scale_factor();
        if width <= 0 || height <= 0 || !scale.is_finite() || scale <= 0.0 {
            return Err(MpvRenderError::InvalidTarget);
        }
        let mut framebuffer = MpvOpenGlFbo {
            framebuffer: target.framebuffer(),
            width,
            height,
            internal_format: 0,
        };
        let mut flip_y: c_int = 1;
        let mut params = [
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_OPENGL_FBO,
                data: (&raw mut framebuffer).cast::<c_void>(),
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_FLIP_Y,
                data: (&raw mut flip_y).cast::<c_void>(),
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        let result = unsafe { (self.api.render)(self.context.as_ptr(), params.as_mut_ptr()) };
        if result < 0 {
            Err(MpvRenderError::Render(result))
        } else {
            Ok(())
        }
    }

    pub fn report_swap(&self) {
        unsafe { (self.api.report_swap)(self.context.as_ptr()) };
    }
}

impl Drop for MpvRenderContext {
    fn drop(&mut self) {
        unsafe {
            (self.api.set_update_callback)(self.context.as_ptr(), None, std::ptr::null_mut());
            (self.api.free)(self.context.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::WakeState;

    #[test]
    fn render_wakes_are_coalesced_until_the_draw_loop_consumes_them() {
        let wake_count = Arc::new(AtomicUsize::new(0));
        let count_for_callback = Arc::clone(&wake_count);
        let state = WakeState::new(Arc::new(move || {
            count_for_callback.fetch_add(1, Ordering::SeqCst);
        }));

        state.notify();
        state.notify();
        assert_eq!(wake_count.load(Ordering::SeqCst), 1);

        state.consume();
        state.notify();
        assert_eq!(wake_count.load(Ordering::SeqCst), 2);
    }
}
