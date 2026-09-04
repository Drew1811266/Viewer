use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread::{self, Thread, ThreadId},
    time::Instant,
};

use thiserror::Error;
use viewer_render_core::{LogicalSize, PhysicalSize, SceneSnapshot, TileCoordinate};

use crate::{
    FrameReceipt, FrameRequest, FrameRing, FrameState, SurfaceAcquireFailure, SurfaceRecovery,
    diagnostics::{GpuFault, GpuFaultState},
    surface_recovery,
};

pub struct SurfaceHandles {
    target: wgpu::SurfaceTarget<'static>,
}

impl SurfaceHandles {
    pub fn new(target: impl Into<wgpu::SurfaceTarget<'static>>) -> Self {
        Self {
            target: target.into(),
        }
    }
}

pub struct RendererDescriptor {
    surface: Option<SurfaceHandles>,
    logical_size: LogicalSize,
    physical_size: PhysicalSize,
    scale_factor: f64,
}

impl RendererDescriptor {
    pub fn headless(
        logical_size: LogicalSize,
        physical_size: PhysicalSize,
        scale_factor: f64,
    ) -> Result<Self, RendererInitError> {
        Self::validate(logical_size, physical_size, scale_factor)?;
        Ok(Self {
            surface: None,
            logical_size,
            physical_size,
            scale_factor,
        })
    }

    pub fn with_surface(
        handles: SurfaceHandles,
        logical_size: LogicalSize,
        physical_size: PhysicalSize,
        scale_factor: f64,
    ) -> Result<Self, RendererInitError> {
        Self::validate(logical_size, physical_size, scale_factor)?;
        Ok(Self {
            surface: Some(handles),
            logical_size,
            physical_size,
            scale_factor,
        })
    }

    fn validate(
        _logical_size: LogicalSize,
        physical_size: PhysicalSize,
        scale_factor: f64,
    ) -> Result<(), RendererInitError> {
        if physical_size.width == 0 || physical_size.height == 0 {
            return Err(RendererInitError::InvalidDescriptor(
                "physical size must be non-empty",
            ));
        }
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(RendererInitError::InvalidDescriptor(
                "scale factor must be finite and positive",
            ));
        }
        Ok(())
    }
}

pub struct WgpuImageRenderer {
    _instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    logical_size: LogicalSize,
    physical_size: PhysicalSize,
    scale_factor: f64,
    frame_state: FrameState,
    frame_ring: FrameRing,
    next_frame_index: u64,
    owner: ThreadId,
    gpu_faults: GpuFaultState,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl WgpuImageRenderer {
    pub fn new(descriptor: RendererDescriptor) -> Result<Self, RendererInitError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = descriptor;
            return Err(RendererInitError::UnsupportedPlatform);
        }

        #[cfg(target_os = "macos")]
        {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::METAL,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let surface = descriptor
                .surface
                .map(|handles| instance.create_surface(handles.target))
                .transpose()
                .map_err(|error| RendererInitError::SurfaceCreation(error.to_string()))?;
            let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface.as_ref(),
                apply_limit_buckets: false,
            }))
            .map_err(|error| RendererInitError::AdapterUnavailable(error.to_string()))?;
            let info = adapter.get_info();
            if info.backend != wgpu::Backend::Metal {
                return Err(RendererInitError::NonMetalAdapter(info.backend));
            }
            let required_limits = adapter.limits();
            let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Viewer image renderer"),
                required_features: wgpu::Features::empty(),
                required_limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            }))
            .map_err(|error| RendererInitError::DeviceUnavailable(error.to_string()))?;
            let gpu_faults = GpuFaultState::default();
            let callback_faults = gpu_faults.clone();
            device.on_uncaptured_error(Arc::new(move |error| callback_faults.record(error)));

            let surface_config = surface
                .as_ref()
                .map(|surface| {
                    surface
                        .get_default_config(
                            &adapter,
                            descriptor.physical_size.width,
                            descriptor.physical_size.height,
                        )
                        .ok_or(RendererInitError::SurfaceUnsupported)
                })
                .transpose()?;
            if let (Some(surface), Some(config)) = (&surface, &surface_config) {
                surface.configure(&device, config);
            }

            Ok(Self {
                _instance: instance,
                adapter,
                device,
                queue,
                surface,
                surface_config,
                logical_size: descriptor.logical_size,
                physical_size: descriptor.physical_size,
                scale_factor: descriptor.scale_factor,
                frame_state: FrameState::default(),
                frame_ring: FrameRing::default(),
                next_frame_index: 0,
                owner: thread::current().id(),
                gpu_faults,
                _not_send_sync: PhantomData,
            })
        }
    }

    pub fn adapter_backend(&self) -> wgpu::Backend {
        self.adapter.get_info().backend
    }

    pub fn frame_state(&mut self) -> &mut FrameState {
        &mut self.frame_state
    }

    pub fn resize(
        &mut self,
        logical_size: LogicalSize,
        physical_size: PhysicalSize,
        scale_factor: f64,
    ) -> Result<(), RenderError> {
        self.ensure_owner()?;
        RendererDescriptor::validate(logical_size, physical_size, scale_factor)
            .map_err(RenderError::InvalidResize)?;
        self.logical_size = logical_size;
        self.physical_size = physical_size;
        self.scale_factor = scale_factor;
        if let (Some(surface), Some(config)) = (&self.surface, self.surface_config.as_mut()) {
            config.width = physical_size.width;
            config.height = physical_size.height;
            surface.configure(&self.device, config);
        }
        self.frame_state.invalidate_surface();
        Ok(())
    }

    pub fn apply_scene(&mut self, scene: &SceneSnapshot) -> Result<(), RenderError> {
        self.ensure_owner()?;
        self.frame_state.invalidate_scene(scene.revision());
        Ok(())
    }

    pub fn upload_resource(&mut self, _tile: TileCoordinate) -> Result<(), RenderError> {
        self.ensure_owner()?;
        self.frame_state.invalidate_resource();
        Ok(())
    }

    pub fn render(&mut self, request: FrameRequest) -> Result<FrameReceipt, RenderError> {
        self.ensure_owner()?;
        match self.gpu_faults.take() {
            Some(GpuFault::OutOfMemory) => return Err(RenderError::OutOfMemory),
            Some(GpuFault::Validation) => return Err(RenderError::GpuValidation),
            Some(GpuFault::Internal) => return Err(RenderError::GpuInternal),
            None => {}
        }
        let slot = self.frame_ring.acquire().ok_or(RenderError::FramesBusy)?;
        let started = Instant::now();
        let result = self.render_owned(request, started);
        self.frame_ring.complete(slot);
        result
    }

    fn render_owned(
        &mut self,
        request: FrameRequest,
        started: Instant,
    ) -> Result<FrameReceipt, RenderError> {
        let presented = if let Some(surface) = &self.surface {
            let texture = match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => texture,
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                    self.frame_state.invalidate_surface();
                    texture
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    return Err(RenderError::Surface(SurfaceAcquireFailure::Timeout));
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    return Err(RenderError::Surface(SurfaceAcquireFailure::Occluded));
                }
                wgpu::CurrentSurfaceTexture::Outdated => {
                    return Err(RenderError::Surface(SurfaceAcquireFailure::Outdated));
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    return Err(RenderError::Surface(SurfaceAcquireFailure::Lost));
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err(RenderError::Surface(SurfaceAcquireFailure::Validation));
                }
            };
            self.queue.present(texture);
            true
        } else {
            let encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Viewer headless frame"),
                });
            self.queue.submit([encoder.finish()]);
            false
        };

        let frame_index = self.next_frame_index;
        self.next_frame_index = self.next_frame_index.saturating_add(1);
        Ok(FrameReceipt {
            frame_index,
            scene_revision: request.scene_revision,
            cpu_time_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
            gpu_time_ns: 0,
            presented,
        })
    }

    fn ensure_owner(&self) -> Result<(), RenderError> {
        if thread::current().id() == self.owner {
            Ok(())
        } else {
            Err(RenderError::WrongThread)
        }
    }
}

#[derive(Debug, Error)]
pub enum RendererInitError {
    #[error("native image rendering is only supported on macOS")]
    UnsupportedPlatform,
    #[error("invalid renderer descriptor: {0}")]
    InvalidDescriptor(&'static str),
    #[error("failed to create Metal surface: {0}")]
    SurfaceCreation(String),
    #[error("Metal adapter unavailable: {0}")]
    AdapterUnavailable(String),
    #[error("selected adapter is not Metal: {0:?}")]
    NonMetalAdapter(wgpu::Backend),
    #[error("failed to create Metal device: {0}")]
    DeviceUnavailable(String),
    #[error("surface is incompatible with the selected Metal adapter")]
    SurfaceUnsupported,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("renderer resources may only be accessed from their owner thread")]
    WrongThread,
    #[error("all frame ring slots are busy")]
    FramesBusy,
    #[error("invalid resize: {0}")]
    InvalidResize(RendererInitError),
    #[error("surface acquisition failed: {0:?}")]
    Surface(SurfaceAcquireFailure),
    #[error("GPU memory exhausted; renderer must terminate without fallback")]
    OutOfMemory,
    #[error("wgpu reported an uncaptured validation error")]
    GpuValidation,
    #[error("wgpu reported an internal GPU error")]
    GpuInternal,
}

impl RenderError {
    pub const fn recovery(&self) -> Option<SurfaceRecovery> {
        match self {
            Self::Surface(failure) => Some(surface_recovery(*failure)),
            Self::OutOfMemory => Some(SurfaceRecovery::Terminate),
            Self::WrongThread
            | Self::FramesBusy
            | Self::InvalidResize(_)
            | Self::GpuValidation
            | Self::GpuInternal => None,
        }
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(Thread);

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    loop {
        match Pin::as_mut(&mut future).poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}
