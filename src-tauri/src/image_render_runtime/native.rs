use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use dispatch2::{MainThreadBound, run_on_main};
use tauri::WebviewWindow;
use viewer_domain::image::ImageProbe;
use viewer_platform_macos::image_render::{
    DecodedResource as PlatformDecodedResource, DecodedResourceKind, DisplayTickSignal,
    InputExclusionRect, MacImageRenderHost, MacImageResourceProvider, MacMemoryPressureMonitor,
    NativeInputSink, PixelFormat, PreviewRequest, SurfaceLayout,
    system_ordinal_glyph_atlas_with_memory,
};
use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, AssetGeneration, CameraState, DeviceLimits,
    DraftGeometry, InteractionController, InteractionEvent, InteractionMode, LogicalPoint,
    LogicalSize, MemoryBudget, NativeInput, NormalizedPoint, NormalizedRect, PointerPhase,
    ResourcePlan, ResourcePlanner, ResourceRequest, Rotation, SceneRevision, SceneSnapshot,
    SourceSize, TextureStrategy, TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::{
    DecodedResource, MagnifierConfig, RenderError, RendererDescriptor, ResourceKey, SurfaceHandles,
    SurfaceRecovery, WgpuImageRenderer,
};

use super::{
    AuthorizedImageRenderCommand, ImageRenderDriver, ImageRenderMagnifierPreferences,
    ImageRenderRuntimeError, NativeRecoveryDecision, NativeRecoveryTracker,
};
use crate::{
    dto::{
        ImageRenderAnnotationGeometryDto, ImageRenderCameraDto, ImageRenderCameraModeDto,
        ImageRenderEventDto, ImageRenderPointDto, ImageRenderRotationDto,
    },
    image_render_events::ImageRenderEventPort,
};

const FIT_INSET: f64 = 0.9;
mod handoff;
const ACTOR_START_TIMEOUT: Duration = Duration::from_secs(15);
const ACTOR_MAILBOX_CAPACITY: usize = 256;
const INPUT_BOUNDARY_CAPACITY: usize = 32;
const MAGNIFIER_POINTER_GAP: f64 = 18.0;

pub struct NativeImageRenderDriver {
    _memory_pressure: MacMemoryPressureMonitor,
    pressure: Arc<DriverPressure>,
    retirement: viewer_render_wgpu::GpuRetirementService,
    memory: viewer_render_core::ImageMemoryCoordinator,
    cache_root: PathBuf,
    budget: MemoryBudget,
    events: Arc<dyn ImageRenderEventPort>,
    state: Mutex<NativeDriverState>,
}

struct DriverPressure {
    memory: viewer_render_core::ImageMemoryCoordinator,
    wake: Mutex<Option<DisplayTickSignal>>,
}

impl DriverPressure {
    fn sink(this: &Arc<Self>) -> viewer_platform_macos::image_render::MemoryPressureSink {
        let weak = Arc::downgrade(this);
        Arc::new(move |level| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            // Serialize bounded publication only. No driver-state lock, disk
            // traversal, allocation work, renderer call or GPU wait here.
            let wake = lock(&state.wake);
            let before = state.memory.snapshot().pressure_epoch;
            if let Ok(epoch) = state.memory.set_pressure(level)
                && epoch != before
                && let Some(wake) = &*wake
            {
                wake.wake();
            }
        })
    }
}

#[derive(Default)]
struct NativeDriverState {
    window: Option<WebviewWindow>,
    host: Option<MainThreadBound<MacImageRenderHost>>,
    actor: Option<RendererActorHandle>,
    initializer: Option<RendererActorInitializer>,
    pending: PendingRendererState,
    exclusions: Vec<InputExclusionRect>,
    tool: Option<InteractionMode>,
}

#[derive(Default)]
struct PendingRendererState {
    open: Option<AuthorizedImageRenderCommand>,
    scene: Option<AuthorizedImageRenderCommand>,
    camera: Option<AuthorizedImageRenderCommand>,
    magnifier: Option<AuthorizedImageRenderCommand>,
}

impl NativeDriverState {
    fn initializer_blocks_start(&self) -> bool {
        self.initializer
            .as_ref()
            .is_some_and(|initializer| !initializer.is_finished())
    }
}

impl PendingRendererState {
    fn record(&mut self, command: AuthorizedImageRenderCommand) {
        match command {
            command @ AuthorizedImageRenderCommand::Open { .. } => {
                self.open = Some(command);
                self.scene = None;
                self.camera = None;
                self.magnifier = None;
            }
            command @ AuthorizedImageRenderCommand::SetScene { .. } => {
                self.scene = Some(command);
            }
            command @ AuthorizedImageRenderCommand::Camera { .. } => {
                self.camera = Some(command);
            }
            command @ AuthorizedImageRenderCommand::SetMagnifier { .. } => {
                self.magnifier = Some(command);
            }
            AuthorizedImageRenderCommand::SetSurface { .. }
            | AuthorizedImageRenderCommand::SetInputExclusions { .. }
            | AuthorizedImageRenderCommand::SetTool { .. } => {}
        }
    }

    fn take_ordered(&mut self) -> Vec<AuthorizedImageRenderCommand> {
        let mut commands = Vec::with_capacity(4);
        commands.extend(self.open.take());
        commands.extend(self.scene.take());
        commands.extend(self.camera.take());
        commands.extend(self.magnifier.take());
        commands
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

impl NativeImageRenderDriver {
    /// Process-local seam for the exact driver-lifetime platform sink.
    pub fn memory_pressure_sink(&self) -> viewer_platform_macos::image_render::MemoryPressureSink {
        DriverPressure::sink(&self.pressure)
    }

    pub fn new(cache_root: &Path, events: Arc<dyn ImageRenderEventPort>) -> Self {
        let budget = MemoryBudget::baseline_8gb();
        let memory = viewer_render_core::ImageMemoryCoordinator::new(
            viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
        );
        let pressure = Arc::new(DriverPressure {
            memory: memory.clone(),
            wake: Mutex::new(None),
        });
        Self {
            _memory_pressure: MacMemoryPressureMonitor::install(DriverPressure::sink(&pressure)),
            pressure,
            retirement: viewer_render_wgpu::GpuRetirementService::new(),
            memory,
            cache_root: cache_root.to_path_buf(),
            budget,
            events,
            state: Mutex::new(NativeDriverState::default()),
        }
    }

    /// Process-local integration seam, using the same accumulator as AppKit.
    pub fn native_input_sink(&self) -> Option<NativeInputSink> {
        lock(&self.state)
            .actor
            .as_ref()
            .map(RendererActorHandle::input_sink)
    }

    fn set_surface(
        &self,
        state: &mut NativeDriverState,
        layout: SurfaceLayout,
    ) -> Result<(), ImageRenderRuntimeError> {
        if state.actor.is_none() {
            if state.initializer_blocks_start() {
                return Err(ImageRenderRuntimeError::DriverUnavailable);
            }
            if state.initializer.is_some() {
                let mut initializer = state
                    .initializer
                    .take()
                    .expect("checked initializer must remain present");
                let join_result = initializer.join();
                let unmount_result = if let Some(host) = state.host.as_mut() {
                    host.get_on_main_mut(MacImageRenderHost::unmount)
                        .map_err(|_| ImageRenderRuntimeError::DriverFailed)
                } else {
                    Ok(())
                };
                state.host = None;
                unmount_result?;
                join_result?;
            }
            self.start_surface(state, layout)?;
            return Ok(());
        }

        state
            .host
            .as_mut()
            .ok_or(ImageRenderRuntimeError::DriverFailed)?
            .get_on_main_mut(move |host| host.set_layout(layout))
            .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
        state
            .actor
            .as_ref()
            .ok_or(ImageRenderRuntimeError::DriverFailed)?
            .set_resize(layout);
        Ok(())
    }

    fn start_surface(
        &self,
        state: &mut NativeDriverState,
        layout: SurfaceLayout,
    ) -> Result<(), ImageRenderRuntimeError> {
        let window = state
            .window
            .clone()
            .ok_or(ImageRenderRuntimeError::DriverUnavailable)?;
        // Cancellation is generation-monotonic inside a provider. A fresh
        // provider per actor prevents one render session's cutoff from
        // cancelling a later session that restarts its generation counter.
        let provider = MacImageResourceProvider::with_memory(
            &self.cache_root,
            self.budget,
            self.memory.clone(),
        )
        .map_err(|_| ImageRenderRuntimeError::DriverUnavailable)?;
        let atlas_memory = self.memory.clone();
        let mounted = run_on_main(move |main_thread| {
            let host = MacImageRenderHost::mount(&window, layout)
                .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
            let handles = host
                .surface()
                .renderer_surface_handles()
                .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
            let glyph_atlas = system_ordinal_glyph_atlas_with_memory(&atlas_memory)
                .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
            Ok::<_, ImageRenderRuntimeError>((
                MainThreadBound::new(host, main_thread),
                handles,
                glyph_atlas,
            ))
        })?;
        let (mut host, handles, glyph_atlas) = mounted;
        let display_signal = DisplayTickSignal::default();
        *lock(&self.pressure.wake) = Some(display_signal.clone());
        let mut actor = match RendererActorHandle::start(
            handles,
            layout,
            display_signal.clone(),
            provider,
            Arc::clone(&self.events),
            glyph_atlas,
            self.retirement.clone(),
        ) {
            Ok(RendererActorStart::Ready(actor)) => actor,
            Ok(RendererActorStart::TimedOut(initializer)) => {
                state.host = Some(host);
                debug_assert!(state.initializer.is_none());
                state.initializer = Some(initializer);
                return Err(ImageRenderRuntimeError::DriverUnavailable);
            }
            Err(error) => {
                let _ = host.get_on_main_mut(MacImageRenderHost::unmount);
                return Err(error);
            }
        };
        let input_sink = actor.input_sink();
        let exclusions = state.exclusions.clone();
        let tool = state.tool.unwrap_or(InteractionMode::Browse);
        let host_started = host.get_on_main_mut(move |host| {
            host.start_display_link(display_signal)?;
            if let Err(error) = host.start_input_monitor(exclusions, tool, input_sink) {
                let _ = host.stop_display_link();
                return Err(error);
            }
            if let Err(error) = host.set_visible(true) {
                let _ = host.stop_input_monitor();
                let _ = host.stop_display_link();
                return Err(error);
            }
            Ok(())
        });
        if host_started.is_err() {
            let _ = actor.shutdown();
            let _ = host.get_on_main_mut(MacImageRenderHost::unmount);
            return Err(ImageRenderRuntimeError::DriverFailed);
        }

        actor.set_tool(tool);
        let pending = state.pending.take_ordered();
        for command in &pending {
            if actor
                .send(RendererMessage::Command(command.clone()))
                .is_err()
            {
                let _ = host.get_on_main_mut(|host| {
                    let input = host.stop_input_monitor();
                    let display = host.stop_display_link();
                    input.and(display)
                });
                let _ = actor.shutdown();
                let _ = host.get_on_main_mut(MacImageRenderHost::unmount);
                for command in pending {
                    state.pending.record(command);
                }
                return Err(ImageRenderRuntimeError::DriverFailed);
            }
        }
        state.host = Some(host);
        state.actor = Some(actor);
        Ok(())
    }

    fn stop(&self) -> Result<(), ImageRenderRuntimeError> {
        {
            let mut state = lock(&self.state);
            if let Some(initializer) = state.initializer.as_mut() {
                initializer.request_shutdown();
                if !initializer.is_finished() {
                    return Err(ImageRenderRuntimeError::DriverFailed);
                }
                initializer.join()?;
                state.initializer = None;
            }
        }
        let (mut host, mut actor) = {
            let mut state = lock(&self.state);
            state.pending.clear();
            state.window = None;
            state.exclusions.clear();
            state.tool = None;
            (state.host.take(), state.actor.take())
        };
        let mut failed = false;

        if let Some(host) = host.as_mut() {
            failed |= host
                .get_on_main_mut(|host| {
                    let input = host.stop_input_monitor();
                    let display = host.stop_display_link();
                    input.and(display)
                })
                .is_err();
        }
        if let Some(actor) = actor.as_mut() {
            failed |= actor.shutdown().is_err();
        }
        if let Some(host) = host.as_mut() {
            failed |= host.get_on_main_mut(MacImageRenderHost::unmount).is_err();
        }
        if failed {
            Err(ImageRenderRuntimeError::DriverFailed)
        } else {
            Ok(())
        }
    }
}

impl ImageRenderDriver for NativeImageRenderDriver {
    fn bind_window(&self, window: WebviewWindow) -> Result<(), ImageRenderRuntimeError> {
        let mut state = lock(&self.state);
        if let Some(current) = state.window.as_ref()
            && current.label() != window.label()
        {
            return Err(ImageRenderRuntimeError::DriverFailed);
        }
        state.window = Some(window);
        Ok(())
    }

    fn apply(&self, command: AuthorizedImageRenderCommand) -> Result<(), ImageRenderRuntimeError> {
        let mut state = lock(&self.state);
        match command {
            AuthorizedImageRenderCommand::SetSurface { layout } => {
                self.set_surface(&mut state, layout)
            }
            AuthorizedImageRenderCommand::SetInputExclusions { exclusions } => {
                state.exclusions.clone_from(&exclusions);
                if let Some(host) = state.host.as_ref() {
                    host.get_on_main(move |host| host.set_input_exclusions(exclusions))
                        .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
                }
                Ok(())
            }
            AuthorizedImageRenderCommand::SetTool { tool } => {
                if let Some(host) = state.host.as_ref() {
                    host.get_on_main(move |host| host.set_input_tool(tool))
                        .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
                }
                if let Some(actor) = state.actor.as_ref() {
                    actor.set_tool(tool);
                }
                state.tool = Some(tool);
                Ok(())
            }
            command => {
                if let Some(actor) = state.actor.as_ref() {
                    actor.send(RendererMessage::Command(command))
                } else {
                    state.pending.record(command);
                    Ok(())
                }
            }
        }
    }

    fn close(&self) -> Result<(), ImageRenderRuntimeError> {
        self.stop()
    }
}

impl Drop for NativeImageRenderDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct RendererActorHandle {
    sender: mpsc::SyncSender<RendererMessage>,
    inputs: Arc<NativeInputAccumulator>,
    controls: Arc<RendererControlAccumulator>,
    wake: DisplayTickSignal,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

enum RendererActorStart {
    Ready(RendererActorHandle),
    TimedOut(RendererActorInitializer),
}

struct RendererActorInitializer {
    wake: DisplayTickSignal,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl RendererActorInitializer {
    fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.wake.wake();
    }

    fn is_finished(&self) -> bool {
        self.join.as_ref().is_none_or(JoinHandle::is_finished)
    }

    fn join(&mut self) -> Result<(), ImageRenderRuntimeError> {
        self.request_shutdown();
        if self.join.take().is_none_or(|join| join.join().is_ok()) {
            Ok(())
        } else {
            Err(ImageRenderRuntimeError::DriverFailed)
        }
    }
}

impl RendererActorHandle {
    #[allow(clippy::too_many_arguments)]
    fn start(
        handles: SurfaceHandles,
        layout: SurfaceLayout,
        display_signal: DisplayTickSignal,
        provider: MacImageResourceProvider,
        events: Arc<dyn ImageRenderEventPort>,
        glyph_atlas: viewer_render_wgpu::OrdinalGlyphAtlas,
        retirement: viewer_render_wgpu::GpuRetirementService,
    ) -> Result<RendererActorStart, ImageRenderRuntimeError> {
        let (sender, receiver) = mpsc::sync_channel(ACTOR_MAILBOX_CAPACITY);
        let inputs = Arc::new(NativeInputAccumulator::default());
        let actor_inputs = Arc::clone(&inputs);
        let controls = Arc::new(RendererControlAccumulator::default());
        let actor_controls = Arc::clone(&controls);
        let actor_wake = display_signal.clone();
        let handle_wake = display_signal.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let actor_shutdown = Arc::clone(&shutdown);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("viewer-image-render".into())
            .spawn(move || {
                let descriptor = renderer_descriptor(handles, layout);
                let initialized = descriptor
                    .and_then(|descriptor| {
                        WgpuImageRenderer::new(
                            descriptor
                                .with_memory(provider.memory().clone())
                                .with_retirement_service(retirement),
                        )
                        .map_err(|error| format!("renderer initialization failed: {error}"))
                    })
                    .and_then(|mut renderer| {
                        renderer
                            .set_ordinal_glyph_atlas(&glyph_atlas)
                            .map_err(|error| format!("glyph atlas upload failed: {error}"))?;
                        Ok(renderer)
                    });
                let mut renderer = match initialized {
                    Ok(renderer) => renderer,
                    Err(error) => {
                        let _ = ready_sender.send(Err(error));
                        return;
                    }
                };
                display_signal.bind_current_thread();
                let mut actor = match RendererActor::new(
                    &mut renderer,
                    layout,
                    provider,
                    events,
                    actor_wake,
                    actor_inputs,
                    actor_controls,
                ) {
                    Ok(actor) => {
                        let _ = ready_sender.send(Ok(()));
                        actor
                    }
                    Err(error) => {
                        let _ = ready_sender.send(Err(error));
                        return;
                    }
                };
                actor.run(&mut renderer, receiver, display_signal, actor_shutdown);
            })
            .map_err(|_| ImageRenderRuntimeError::DriverUnavailable)?;
        match ready_receiver.recv_timeout(ACTOR_START_TIMEOUT) {
            Ok(Ok(())) => Ok(RendererActorStart::Ready(Self {
                sender,
                inputs,
                controls,
                wake: handle_wake,
                shutdown,
                join: Some(join),
            })),
            Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                shutdown.store(true, Ordering::Release);
                handle_wake.wake();
                let _ = join.join();
                Err(ImageRenderRuntimeError::DriverUnavailable)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                shutdown.store(true, Ordering::Release);
                handle_wake.wake();
                Ok(RendererActorStart::TimedOut(RendererActorInitializer {
                    wake: handle_wake,
                    shutdown,
                    join: Some(join),
                }))
            }
        }
    }

    fn input_sink(&self) -> NativeInputSink {
        let inputs = Arc::clone(&self.inputs);
        let wake = self.wake.clone();
        Arc::new(move |input| {
            if inputs.push(input) {
                wake.wake();
            }
        })
    }

    fn send(&self, message: RendererMessage) -> Result<(), ImageRenderRuntimeError> {
        self.sender
            .try_send(message)
            .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
        self.wake.wake();
        Ok(())
    }

    fn set_resize(&self, layout: SurfaceLayout) {
        self.controls.set_resize(layout);
        self.wake.wake();
    }

    fn set_tool(&self, tool: InteractionMode) {
        self.controls.set_tool(tool);
        self.wake.wake();
    }

    fn shutdown(&mut self) -> Result<(), ImageRenderRuntimeError> {
        if self.join.is_none() {
            return Ok(());
        }
        self.shutdown.store(true, Ordering::Release);
        self.wake.wake();
        let joined = self.join.take().is_none_or(|join| join.join().is_ok());
        if joined {
            Ok(())
        } else {
            Err(ImageRenderRuntimeError::DriverFailed)
        }
    }
}

impl Drop for RendererActorHandle {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

enum RendererMessage {
    Command(AuthorizedImageRenderCommand),
}

#[derive(Default)]
struct RendererControlAccumulator {
    state: Mutex<RendererControlState>,
}

#[derive(Default)]
struct RendererControlState {
    resize: Option<SurfaceLayout>,
    tool: Option<InteractionMode>,
}

impl RendererControlAccumulator {
    fn set_resize(&self, layout: SurfaceLayout) {
        lock(&self.state).resize = Some(layout);
    }

    fn set_tool(&self, tool: InteractionMode) {
        lock(&self.state).tool = Some(tool);
    }

    fn take(&self) -> RendererControlState {
        std::mem::take(&mut *lock(&self.state))
    }
}

enum DecodeResult {
    Refined {
        session_id: viewer_render_core::RenderSessionId,
        generation: AssetGeneration,
        revision: u64,
        resource: Box<PlatformDecodedResource>,
    },
    RefinementCompleted {
        session_id: viewer_render_core::RenderSessionId,
        generation: AssetGeneration,
        revision: u64,
        keys: Vec<ResourceKey>,
    },
    Decoded {
        session_id: viewer_render_core::RenderSessionId,
        generation: AssetGeneration,
        probe: ImageProbe,
        resource: Box<PlatformDecodedResource>,
    },
    DecodeFailed {
        session_id: viewer_render_core::RenderSessionId,
        generation: AssetGeneration,
        cancelled: bool,
        memory: Option<viewer_render_core::MemoryAdmissionError>,
        revision: Option<u64>,
        retry_demand: Option<DecodeDemandSnapshot>,
    },
}

#[derive(Default)]
struct NativeInputAccumulator {
    state: Mutex<NativeInputState>,
}

#[derive(Default)]
struct NativeInputState {
    dropped_input_samples: u64,
    boundaries: VecDeque<NativeInput>,
    pointer_move: Option<NativeInput>,
    scroll: Option<NativeInput>,
    magnify: Option<NativeInput>,
}

impl NativeInputState {
    fn push_boundary(&mut self, input: NativeInput) {
        if self.boundaries.len() >= INPUT_BOUNDARY_CAPACITY {
            self.dropped_input_samples = self
                .dropped_input_samples
                .saturating_add(self.boundaries.len() as u64 + 1);
            self.boundaries.clear();
            self.pointer_move = None;
            self.scroll = None;
            self.magnify = None;
            self.boundaries.push_back(NativeInput::Cancel);
            return;
        }
        self.boundaries.push_back(input);
    }
}

impl NativeInputAccumulator {
    fn dropped_input_samples(&self) -> u64 {
        lock(&self.state).dropped_input_samples
    }
    /// Returns true when the renderer actor must wake immediately. Continuous
    /// samples are consumed on the next display tick and never fill a queue.
    fn push(&self, input: NativeInput) -> bool {
        let mut state = lock(&self.state);
        match input {
            input @ NativeInput::Pointer(sample) => match sample.phase {
                PointerPhase::Down => {
                    state.push_boundary(input);
                    true
                }
                PointerPhase::Move => {
                    state.pointer_move = Some(input);
                    false
                }
                PointerPhase::Up => {
                    if let Some(final_move) = state.pointer_move.take() {
                        state.push_boundary(final_move);
                    }
                    state.push_boundary(input);
                    true
                }
                PointerPhase::Cancel => {
                    state.pointer_move = None;
                    state.scroll = None;
                    state.magnify = None;
                    state.push_boundary(input);
                    true
                }
            },
            input @ NativeInput::Hover(_) => {
                state.pointer_move = Some(input);
                false
            }
            NativeInput::Scroll(mut sample) => {
                if let Some(NativeInput::Scroll(previous)) = state.scroll.take() {
                    sample.delta.x += previous.delta.x;
                    sample.delta.y += previous.delta.y;
                }
                state.scroll = Some(NativeInput::Scroll(sample));
                false
            }
            NativeInput::Magnify(mut sample) => {
                if let Some(NativeInput::Magnify(previous)) = state.magnify.take() {
                    sample.factor = (sample.factor * previous.factor).clamp(0.01, 100.0);
                }
                state.magnify = Some(NativeInput::Magnify(sample));
                false
            }
            NativeInput::Cancel => {
                state.pointer_move = None;
                state.scroll = None;
                state.magnify = None;
                state.push_boundary(NativeInput::Cancel);
                true
            }
        }
    }

    fn take_boundaries(&self) -> Vec<NativeInput> {
        let mut state = lock(&self.state);
        state.boundaries.drain(..).collect()
    }

    fn take_continuous(&self) -> Vec<NativeInput> {
        let mut state = lock(&self.state);
        [
            state.pointer_move.take(),
            state.scroll.take(),
            state.magnify.take(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[derive(Clone)]
struct DecodeJob {
    session_id: viewer_render_core::RenderSessionId,
    generation: AssetGeneration,
    source: viewer_platform_macos::image_render::AuthorizedImageSource,
    physical_width: u32,
    physical_height: u32,
    refinement: Option<(u64, SourceSize, Vec<ResourcePlan>)>,
    retry_demand: DecodeDemandSnapshot,
}

#[derive(Clone, Copy, Debug)]
struct DecodeDemandSnapshot {
    combined_bytes: u64,
    gpu_bytes: u64,
    retiring_bytes: u64,
    pressure_epoch: u64,
}

impl DecodeDemandSnapshot {
    fn capture(renderer: &WgpuImageRenderer) -> Self {
        let snapshot = renderer.memory().snapshot();
        Self {
            combined_bytes: snapshot.combined_bytes,
            gpu_bytes: snapshot.gpu_bytes,
            retiring_bytes: snapshot.bytes_for_phase(viewer_render_core::AllocationPhase::Retiring),
            pressure_epoch: snapshot.pressure_epoch,
        }
    }

    const fn max() -> Self {
        Self {
            combined_bytes: u64::MAX,
            gpu_bytes: u64::MAX,
            retiring_bytes: u64::MAX,
            pressure_epoch: u64::MAX,
        }
    }
}

#[derive(Default)]
struct DecodeJobState {
    latest: Option<DecodeJob>,
    epoch: u64,
    shutdown: bool,
}

type DecodeResultSlot = handoff::ByteHandoff<DecodeResult>;

struct DecodeWorker {
    jobs: Arc<(Mutex<DecodeJobState>, Condvar)>,
    results: Arc<DecodeResultSlot>,
    join: Option<JoinHandle<()>>,
}

impl DecodeWorker {
    fn start(
        provider: MacImageResourceProvider,
        results: Arc<DecodeResultSlot>,
        wake: DisplayTickSignal,
    ) -> Result<Self, String> {
        let jobs = Arc::new((Mutex::new(DecodeJobState::default()), Condvar::new()));
        let worker_jobs = Arc::clone(&jobs);
        let worker_results = results.clone();
        let join = thread::Builder::new()
            .name("viewer-image-decode".into())
            .spawn(move || {
                loop {
                    let (job, epoch) = {
                        let (mutex, available) = worker_jobs.as_ref();
                        let mut state = lock(mutex);
                        while state.latest.is_none() && !state.shutdown {
                            state = available.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
                        }
                        if state.shutdown { return; }
                        (state.latest.take().expect("decode job is available"), state.epoch)
                    };
                    let cancelled = || {
                        let state = lock(&worker_jobs.0);
                        state.shutdown || state.epoch != epoch
                    };
                    let publish = |result, bytes| {
                        let accepted = worker_results.publish(result, bytes, epoch);
                        if accepted { wake.wake(); }
                        accepted
                    };
                    let result = if let Some((revision, source_size, plans)) = &job.refinement {
                        let mut keys = Vec::new();
                        decode_refinement(&provider, &job, *source_size, plans, &cancelled, &mut |resource| {
                            keys.push(resource_key(&resource));
                            let bytes = resource.pixels.len() as u64;
                            if publish(DecodeResult::Refined { session_id: job.session_id, generation: job.generation, revision: *revision, resource: Box::new(resource) }, bytes) {
                                Ok(())
                            } else { Err(viewer_platform_macos::image_render::ImageResourceError::Cancelled) }
                        }).map(|()| DecodeResult::RefinementCompleted { session_id: job.session_id, generation: job.generation, revision: *revision, keys })
                    } else {
                        PreviewRequest::new(job.physical_width.max(1), job.physical_height.max(1))
                            .and_then(|request| provider.probe_and_request_preview_cancellable(&job.source, job.generation, request, &cancelled))
                            .map(|(probe, resource)| DecodeResult::Decoded { session_id: job.session_id, generation: job.generation, probe, resource: Box::new(resource) })
                    };
                    if cancelled() { drop(result); wake.wake(); continue; }
                    let result = result.unwrap_or_else(|error| {
                        use viewer_platform_macos::image_render::ImageResourceError;
                        DecodeResult::DecodeFailed {
                            session_id: job.session_id, generation: job.generation,
                            cancelled: matches!(error, ImageResourceError::Cancelled),
                            memory: if let ImageResourceError::MemoryAdmission(error) = error { Some(error) } else { None },
                            revision: job.refinement.as_ref().map(|(revision, _, _)| *revision),
                            retry_demand: Some(job.retry_demand),
                        }
                    });
                    let bytes = match &result {
                        DecodeResult::Decoded { resource, .. } => resource.pixels.len() as u64,
                        _ => 0,
                    };
                    publish(result, bytes);
                }
            })
            .map_err(|error| format!("decode worker initialization failed: {error}"))?;
        Ok(Self {
            jobs,
            results,
            join: Some(join),
        })
    }

    fn submit(&self, job: DecodeJob) -> Result<(), ImageRenderRuntimeError> {
        let (mutex, available) = self.jobs.as_ref();
        let mut state = lock(mutex);
        if state.shutdown {
            return Err(ImageRenderRuntimeError::DriverFailed);
        }
        state.latest = Some(job);
        state.epoch = state.epoch.saturating_add(1);
        self.results.reset(state.epoch);
        available.notify_one();
        Ok(())
    }

    fn cancel_pending(&self) {
        let mut state = lock(&self.jobs.0);
        state.latest = None;
        state.epoch = state.epoch.saturating_add(1);
        self.results.reset(state.epoch);
    }

    fn shutdown(&mut self) {
        let (mutex, available) = self.jobs.as_ref();
        {
            let mut state = lock(mutex);
            state.shutdown = true;
            state.latest = None;
            self.results.close();
            available.notify_one();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for DecodeWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn requested_resource_plans(
    transform: TransformSnapshot,
    magnifier: Option<MagnifierConfig>,
    limit: u32,
    budget: MemoryBudget,
) -> Vec<ResourcePlan> {
    let source = transform.source();
    let viewport = transform.viewport();
    let display = transform.displayed_rect();
    let rotated = matches!(
        transform.camera().rotation,
        Rotation::Deg90 | Rotation::Deg270
    );
    let source_width = if rotated { source.height } else { source.width };
    let scale = display.width * viewport.scale_factor / f64::from(source_width);
    let corners = [
        LogicalPoint::ZERO,
        LogicalPoint {
            x: viewport.logical_size.width,
            y: 0.0,
        },
        LogicalPoint {
            x: 0.0,
            y: viewport.logical_size.height,
        },
        LogicalPoint {
            x: viewport.logical_size.width,
            y: viewport.logical_size.height,
        },
    ]
    .map(|point| transform.view_to_image_clamped(point));
    let left = corners.iter().map(|p| p.x).fold(1.0, f64::min);
    let top = corners.iter().map(|p| p.y).fold(1.0, f64::min);
    let right = corners.iter().map(|p| p.x).fold(0.0, f64::max);
    let bottom = corners.iter().map(|p| p.y).fold(0.0, f64::max);
    let mut requests = vec![(
        NormalizedRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        },
        scale,
    )];
    if let Some(lens) = magnifier {
        let half_width = lens.width_px * viewport.scale_factor
            / (2.0 * scale * lens.magnification * f64::from(source.width));
        let half_height = lens.height_px * viewport.scale_factor
            / (2.0 * scale * lens.magnification * f64::from(source.height));
        let (half_width, half_height) = if rotated {
            (
                lens.height_px * viewport.scale_factor
                    / (2.0 * scale * lens.magnification * f64::from(source.width)),
                lens.width_px * viewport.scale_factor
                    / (2.0 * scale * lens.magnification * f64::from(source.height)),
            )
        } else {
            (half_width, half_height)
        };
        let x = (lens.focus.x - half_width).max(0.0);
        let y = (lens.focus.y - half_height).max(0.0);
        requests.push((
            NormalizedRect {
                x,
                y,
                width: (lens.focus.x + half_width).min(1.0) - x,
                height: (lens.focus.y + half_height).min(1.0) - y,
            },
            scale * lens.magnification,
        ));
    }
    let planner = ResourcePlanner::new(
        DeviceLimits {
            max_texture_dimension_2d: limit,
        },
        budget,
    );
    let mut plans: Vec<ResourcePlan> = Vec::new();
    for (visible, display_scale) in requests {
        if visible.width <= 0.0 || visible.height <= 0.0 {
            continue;
        }
        let plan = planner
            .plan(ResourceRequest {
                source_size: source,
                visible_normalized_rect: visible,
                display_scale,
                viewport_physical_size: transform.physical_viewport(),
            })
            .expect("validated transform resource request");
        if let Some(existing) = plans
            .iter_mut()
            .find(|existing| existing.level == plan.level && existing.strategy == plan.strategy)
        {
            existing.required_tiles.extend(plan.required_tiles);
            existing.required_tiles.sort_unstable();
            existing.required_tiles.dedup();
            existing.prefetch_tiles.extend(plan.prefetch_tiles);
            existing.prefetch_tiles.sort_unstable();
            existing.prefetch_tiles.dedup();
            existing
                .prefetch_tiles
                .retain(|tile| !existing.required_tiles.contains(tile));
        } else {
            plans.push(plan);
        }
    }
    if plans.len() == 2 && plans[0].strategy == TextureStrategy::SingleTexture {
        // Both viewports share the sharpest whole-image texture.
        let sharpest = plans.iter().min_by_key(|plan| plan.level).unwrap().clone();
        plans = vec![sharpest];
    }
    plans
}

fn refinement_plans(
    transform: TransformSnapshot,
    magnifier: Option<MagnifierConfig>,
    limit: u32,
    budget: MemoryBudget,
    preview_bytes: u64,
) -> Result<Vec<ResourcePlan>, ()> {
    let source = transform.source();
    let mut plans = requested_resource_plans(transform, magnifier, limit, budget);
    // The native decoder retains one mip at a time while completed tiles
    // from earlier viewport plans remain alive. Reserve that peak explicitly.
    let native_mip_bytes = plans
        .iter()
        .map(|plan| mip_bytes(source, plan.level))
        .max()
        .unwrap_or(0);
    let available_gpu = budget.gpu_texture_bytes.saturating_sub(preview_bytes);
    let available_cpu = budget.cpu_staging_bytes.saturating_sub(native_mip_bytes);
    let mut returned_bytes = plans
        .iter()
        .map(|plan| {
            plan.required_tiles
                .iter()
                .map(|tile| planned_tile_bytes(source, plan.strategy, *tile))
                .sum::<u64>()
        })
        .sum::<u64>();
    let vertex_bytes = std::mem::size_of::<[viewer_render_wgpu::ImageVertex; 6]>() as u64;
    let mut gpu_bytes = returned_bytes
        + plans
            .iter()
            .map(|plan| plan.required_tiles.len() as u64 * vertex_bytes)
            .sum::<u64>();
    if native_mip_bytes > budget.cpu_staging_bytes
        || returned_bytes > available_cpu
        || gpu_bytes > available_gpu
    {
        return Err(());
    }
    for plan in &mut plans {
        plan.prefetch_tiles.retain(|tile| {
            let bytes = planned_tile_bytes(source, plan.strategy, *tile);
            if returned_bytes.saturating_add(bytes) > available_cpu
                || gpu_bytes.saturating_add(bytes).saturating_add(vertex_bytes) > available_gpu
            {
                return false;
            }
            returned_bytes += bytes;
            gpu_bytes += bytes + vertex_bytes;
            true
        });
    }
    Ok(plans)
}

fn mip_bytes(source: SourceSize, level: u32) -> u64 {
    let divisor = 1u32 << level.min(31);
    u64::from(source.width.div_ceil(divisor)) * u64::from(source.height.div_ceil(divisor)) * 4
}

fn planned_tile_bytes(
    source: SourceSize,
    strategy: TextureStrategy,
    tile: viewer_render_core::TileCoordinate,
) -> u64 {
    let TextureStrategy::Tiled { tile_size } = strategy else {
        return mip_bytes(source, tile.level);
    };
    let divisor = 1u32 << tile.level.min(31);
    let width = source
        .width
        .div_ceil(divisor)
        .saturating_sub(tile.x.saturating_mul(tile_size))
        .min(tile_size);
    let height = source
        .height
        .div_ceil(divisor)
        .saturating_sub(tile.y.saturating_mul(tile_size))
        .min(tile_size);
    u64::from(width) * u64::from(height) * 4
}

fn decode_refinement(
    provider: &MacImageResourceProvider,
    job: &DecodeJob,
    source_size: SourceSize,
    plans: &[ResourcePlan],
    cancelled: &dyn Fn() -> bool,
    deliver: &mut dyn FnMut(
        PlatformDecodedResource,
    )
        -> Result<(), viewer_platform_macos::image_render::ImageResourceError>,
) -> Result<(), viewer_platform_macos::image_render::ImageResourceError> {
    use viewer_platform_macos::image_render::ImageResourceError;
    for plan in plans {
        if cancelled() {
            return Err(ImageResourceError::Cancelled);
        }
        // An empty off-image request is cancellation/no refinement, not failure.
        if plan.required_tiles.is_empty() && plan.prefetch_tiles.is_empty() {
            continue;
        }
        match plan.strategy {
            TextureStrategy::SingleTexture => {
                let divisor = 1u32 << plan.level.min(31);
                let (_, resource) = provider.probe_and_request_preview_cancellable(
                    &job.source,
                    job.generation,
                    PreviewRequest::new(
                        source_size.width.div_ceil(divisor),
                        source_size.height.div_ceil(divisor),
                    )?,
                    cancelled,
                )?;
                if cancelled() {
                    return Err(ImageResourceError::Cancelled);
                }
                deliver(resource)?;
            }
            TextureStrategy::Tiled { .. } => provider.stream_tiles_cancellable(
                &job.source,
                job.generation,
                plan,
                cancelled,
                deliver,
            )?,
        }
    }
    if cancelled() {
        return Err(ImageResourceError::Cancelled);
    }
    Ok(())
}

fn resource_key(resource: &PlatformDecodedResource) -> ResourceKey {
    match resource.kind {
        DecodedResourceKind::Preview { level } => ResourceKey::WholeImage { level },
        DecodedResourceKind::Tile(tile) => ResourceKey::Tile(tile),
    }
}

fn gpu_resource(
    resource: PlatformDecodedResource,
    source_size: SourceSize,
) -> Option<DecodedResource> {
    if resource.pixel_format != PixelFormat::Bgra8PremultipliedSrgb
        || resource.bytes_per_row != resource.width.checked_mul(4)?
    {
        return None;
    }
    Some(match resource.kind {
        DecodedResourceKind::Preview { level } => DecodedResource::WholeImage {
            generation: resource.generation,
            level,
            source_size,
            width: resource.width,
            height: resource.height,
            pixels: resource.pixels,
        },
        DecodedResourceKind::Tile(tile) => DecodedResource::Tile {
            generation: resource.generation,
            tile,
            tile_size: viewer_render_core::DEFAULT_TILE_SIZE,
            sample_border: resource.sample_border,
            source_size,
            width: resource.width,
            height: resource.height,
            pixels: resource.pixels,
        },
    })
}

struct PendingPreview {
    session_id: viewer_render_core::RenderSessionId,
    generation: AssetGeneration,
    source_size: SourceSize,
    key: ResourceKey,
    bytes: u64,
}
struct PendingActorUpload {
    resource: DecodedResource,
    handle: Option<viewer_render_wgpu::ResourceHandle>,
    preview: Option<PendingPreview>,
    admission_snapshot: Option<(u64, u64, u64)>,
}
struct RendererActor {
    provider: MacImageResourceProvider,
    pressure_epoch: u64,
    gpu_wake: Arc<dyn Fn() + Send + Sync>,
    decode_worker: DecodeWorker,
    decode_results: Arc<DecodeResultSlot>,
    inputs: Arc<NativeInputAccumulator>,
    controls: Arc<RendererControlAccumulator>,
    events: Arc<dyn ImageRenderEventPort>,
    viewport: ViewportLayout,
    session_id: Option<viewer_render_core::RenderSessionId>,
    generation: AssetGeneration,
    source_size: Option<SourceSize>,
    source: Option<viewer_platform_macos::image_render::AuthorizedImageSource>,
    resource_revision: u64,
    resource_plans: Vec<ResourcePlan>,
    detail_availability: Option<(u64, bool)>,
    demand_failed: bool,
    pending_upload: Option<PendingActorUpload>,
    pending_manifest: Option<Vec<ResourceKey>>,
    suspended_decode: bool,
    retry_decode: Option<(Option<u64>, DecodeDemandSnapshot)>,
    refinement_keys: Vec<ResourceKey>,
    preview_key: Option<ResourceKey>,
    preview_bytes: u64,
    camera: CameraState,
    transform: Option<TransformSnapshot>,
    magnifier_preferences: Option<ImageRenderMagnifierPreferences>,
    magnifier_pointer: Option<LogicalPoint>,
    applied_magnifier: Option<MagnifierConfig>,
    content_frames: ContentFrameGate,
    scene: SceneSnapshot,
    rendered_scene: SceneSnapshot,
    pending_rendered_scene: Option<SceneSnapshot>,
    interaction: InteractionController,
    geometry_edit: Option<NativeGeometryEdit>,
    editor_anchor: Option<NormalizedPoint>,
    pending_camera_event: Option<ImageRenderEventDto>,
    pending_draft_event: Option<ImageRenderEventDto>,
    pending_editor_placement_event: Option<ImageRenderEventDto>,
    recovery: NativeRecoveryTracker,
}

#[derive(Default)]
struct ContentFrameGate {
    installed_generation: Option<AssetGeneration>,
}

struct NativeGeometryEdit {
    annotation_id: AnnotationId,
    original: AnnotationGeometry,
    current: AnnotationGeometry,
    completed: bool,
}

impl ContentFrameGate {
    fn begin_generation(&mut self) {
        self.installed_generation = None;
    }

    fn install(&mut self, generation: AssetGeneration) {
        self.installed_generation = Some(generation);
    }

    fn can_publish(&self, generation: AssetGeneration) -> bool {
        self.installed_generation == Some(generation)
    }
}

impl RendererActor {
    fn planning_budget(&self, renderer: &WgpuImageRenderer) -> MemoryBudget {
        let snapshot = self.provider.memory().snapshot();
        // Retirement remains charged for admission, but is not a permanent
        // fixed-resource requirement for the eventual replacement plan.
        let fixed_gpu = snapshot
            .gpu_bytes
            .saturating_sub(snapshot.retiring_gpu_bytes)
            .saturating_sub(renderer.image_texture_bytes());
        let base = self.provider.budget();
        MemoryBudget {
            cpu_staging_bytes: base.cpu_staging_bytes.min(
                snapshot
                    .limits
                    .combined_bytes
                    .saturating_sub(snapshot.gpu_bytes),
            ),
            gpu_texture_bytes: base
                .gpu_texture_bytes
                .min(snapshot.limits.gpu_bytes.saturating_sub(fixed_gpu)),
            disk_cache_bytes: base.disk_cache_bytes,
        }
    }

    fn apply_memory_pressure(&mut self, renderer: &mut WgpuImageRenderer) {
        let snapshot = self.provider.memory().snapshot();
        if snapshot.pressure_epoch == self.pressure_epoch {
            return;
        }
        self.pressure_epoch = snapshot.pressure_epoch;
        self.decode_worker.cancel_pending();
        self.resource_revision = self.resource_revision.saturating_add(1);
        self.suspended_decode = false;
        self.pending_upload = None;
        renderer.cancel_pending_upload();
        self.pending_manifest = None;
        self.retry_decode = None;
        self.demand_failed = true;
        // Derive the latest camera/lens keys even when their complete requested
        // detail no longer fits. This is priority metadata, not an admission.
        let desired = self
            .transform
            .map(|transform| {
                requested_resource_plans(
                    transform,
                    self.applied_magnifier,
                    renderer.max_texture_dimension_2d(),
                    self.provider.budget(),
                )
            })
            .unwrap_or_default();
        let keys = |prefetch: bool| {
            desired
                .iter()
                .flat_map(|plan| {
                    let tiles = if prefetch {
                        &plan.prefetch_tiles
                    } else {
                        &plan.required_tiles
                    };
                    tiles.iter().map(|tile| match plan.strategy {
                        TextureStrategy::SingleTexture => {
                            ResourceKey::WholeImage { level: plan.level }
                        }
                        TextureStrategy::Tiled { .. } => ResourceKey::Tile(*tile),
                    })
                })
                .collect::<Vec<_>>()
        };
        let visible = keys(false);
        renderer.touch_resources(&visible);
        if snapshot.pressure != viewer_render_core::PressureLevel::Normal {
            renderer.reclaim_resources(
                &visible,
                self.preview_key,
                &keys(true),
                self.planning_budget(renderer).gpu_texture_bytes,
            );
            self.refinement_keys
                .retain(|key| renderer.resource_key_is_ready(*key));
            if renderer.image_texture_bytes() == 0 {
                self.content_frames.begin_generation();
                self.publish_detail_availability(false);
            }
        }
        if self.source_size.is_some() {
            self.schedule_refinement(renderer);
        } else {
            self.retry_decode = Some((None, DecodeDemandSnapshot::max()));
            self.retry_decode_after_release(renderer);
        }
    }

    fn new(
        renderer: &mut WgpuImageRenderer,
        layout: SurfaceLayout,
        provider: MacImageResourceProvider,
        events: Arc<dyn ImageRenderEventPort>,
        wake: DisplayTickSignal,
        inputs: Arc<NativeInputAccumulator>,
        controls: Arc<RendererControlAccumulator>,
    ) -> Result<Self, String> {
        let viewport = viewport_layout(layout).expect("validated surface layout");
        let scene = SceneSnapshot::empty(SceneRevision(0));
        renderer
            .apply_scene(&scene)
            .map_err(|error| format!("initial scene upload failed: {error}"))?;
        let decode_results = Arc::new(DecodeResultSlot::default());
        let completion_wake = wake.clone();
        let decode_worker =
            DecodeWorker::start(provider.clone(), Arc::clone(&decode_results), wake)?;
        Ok(Self {
            provider,
            pressure_epoch: 0,
            gpu_wake: Arc::new(move || completion_wake.wake()),
            decode_worker,
            decode_results,
            inputs,
            controls,
            events,
            viewport,
            session_id: None,
            generation: AssetGeneration(0),
            source_size: None,
            source: None,
            resource_revision: 0,
            resource_plans: Vec::new(),
            detail_availability: None,
            demand_failed: false,
            pending_upload: None,
            pending_manifest: None,
            suspended_decode: false,
            retry_decode: None,
            refinement_keys: Vec::new(),
            preview_key: None,
            preview_bytes: 0,
            camera: CameraState::fit(Rotation::Deg0),
            transform: None,
            magnifier_preferences: None,
            magnifier_pointer: Some(LogicalPoint {
                x: viewport.logical_size.width / 2.0,
                y: viewport.logical_size.height / 2.0,
            }),
            applied_magnifier: None,
            content_frames: ContentFrameGate::default(),
            scene: scene.clone(),
            rendered_scene: scene,
            pending_rendered_scene: None,
            interaction: InteractionController::new(InteractionMode::Browse),
            geometry_edit: None,
            editor_anchor: None,
            pending_camera_event: None,
            pending_draft_event: None,
            pending_editor_placement_event: None,
            recovery: NativeRecoveryTracker::default(),
        })
    }

    fn run(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        receiver: mpsc::Receiver<RendererMessage>,
        display_signal: DisplayTickSignal,
        shutdown: Arc<AtomicBool>,
    ) {
        self.events
            .gpu_timing_support(renderer.renderer_id(), renderer.gpu_timing_support());
        while !shutdown.load(Ordering::Acquire) {
            let mut did_work = false;
            self.apply_memory_pressure(renderer);
            // Any wake (pressure, producer release, GPU completion, or display)
            // can make progress. Hidden windows do not require a display tick.
            match renderer.poll_gpu_timings() {
                Ok(samples) => {
                    for sample in samples {
                        self.events.gpu_frame_completed(sample);
                    }
                }
                Err(error) => {
                    self.handle_render_error(renderer, error);
                    break;
                }
            }
            self.progress_upload(renderer);
            self.sync_rendered_scene(renderer);
            let controls = self.controls.take();
            if let Some(layout) = controls.resize {
                self.resize(renderer, layout);
                did_work = true;
            }
            if let Some(tool) = controls.tool {
                self.handle_command(renderer, AuthorizedImageRenderCommand::SetTool { tool });
                did_work = true;
            }
            while let Ok(message) = receiver.try_recv() {
                self.handle_message(renderer, message);
                did_work = true;
            }
            for input in self.inputs.take_boundaries() {
                self.handle_input(renderer, input);
                did_work = true;
            }
            if self.pending_upload.is_none()
                && !renderer.has_pending_upload()
                && let Some(result) = self.decode_results.take()
            {
                self.handle_decode_result(renderer, result);
                did_work = true;
            }
            if let Some(timestamp_ns) = display_signal.take_latest() {
                did_work = true;
                for input in self.inputs.take_continuous() {
                    self.handle_input(renderer, input);
                }
                self.flush_continuous_events();
                if let Some(request) = renderer.on_display_tick(timestamp_ns) {
                    match renderer.render(request) {
                        Ok(receipt) if receipt.presented => {
                            self.recovery.record_presented_frame();
                            if let Some(session_id) = self.session_id
                                && self.content_frames.can_publish(self.generation)
                            {
                                self.events.surface_frame_submitted(
                                    receipt,
                                    timestamp_ns,
                                    self.generation.0,
                                    self.inputs.dropped_input_samples(),
                                    renderer.renderer_id(),
                                );
                                self.events.publish(ImageRenderEventDto::FramePresented {
                                    session_id: session_id.0,
                                    asset_generation: self.generation.0,
                                    scene_revision: receipt.scene_revision.0,
                                    frame_index: receipt.frame_index,
                                });
                            }
                        }
                        Ok(_) => {}
                        Err(error) => self.handle_render_error(renderer, error),
                    }
                }
            }
            renderer.wake_when_gpu_progress(self.gpu_wake.clone());
            if !did_work {
                thread::park();
            }
        }
        self.provider.cancel_generation(self.generation);
        self.decode_worker.shutdown();
    }

    fn handle_message(&mut self, renderer: &mut WgpuImageRenderer, message: RendererMessage) {
        match message {
            RendererMessage::Command(command) => self.handle_command(renderer, command),
        }
    }

    fn handle_decode_result(&mut self, renderer: &mut WgpuImageRenderer, result: DecodeResult) {
        match result {
            DecodeResult::Refined {
                session_id,
                generation,
                revision,
                resource,
            } => {
                if self.session_id != Some(session_id)
                    || generation != self.generation
                    || revision != self.resource_revision
                {
                    return;
                }
                let Some(source_size) = self.source_size else {
                    return;
                };
                let Some(upload) = gpu_resource(*resource, source_size) else {
                    self.publish_failure("image_render_resource_invalid", false);
                    return;
                };
                // Same-generation resources are immutable. Reuse completed
                // latest-demand tiles instead of allocating duplicate copies.
                if !renderer.resource_key_is_ready(upload.key()) {
                    self.pending_upload = Some(PendingActorUpload {
                        resource: upload,
                        handle: None,
                        preview: None,
                        admission_snapshot: None,
                    });
                    self.progress_upload(renderer);
                }
            }
            DecodeResult::RefinementCompleted {
                session_id,
                generation,
                revision,
                keys,
            } => {
                if self.session_id != Some(session_id)
                    || generation != self.generation
                    || revision != self.resource_revision
                {
                    return;
                }
                if keys.is_empty() {
                    return;
                }
                self.pending_manifest = Some(keys);
                self.publish_completed_refinement(renderer);
            }
            DecodeResult::Decoded {
                session_id,
                generation,
                probe,
                resource,
            } => self.install_resource(renderer, session_id, generation, probe, *resource),
            DecodeResult::DecodeFailed {
                session_id,
                generation,
                cancelled,
                revision,
                memory,
                retry_demand,
            } => {
                if !cancelled
                    && self.session_id == Some(session_id)
                    && self.generation == generation
                    && revision.is_none_or(|revision| revision == self.resource_revision)
                {
                    self.demand_failed = true;
                    let retry_demand =
                        retry_demand.unwrap_or_else(|| DecodeDemandSnapshot::capture(renderer));
                    if matches!(
                        memory,
                        Some(
                            viewer_render_core::MemoryAdmissionError::TemporarilyBlocked
                                | viewer_render_core::MemoryAdmissionError::ExceedsPolicy
                        )
                    ) {
                        self.publish_detail_availability(false);
                    }
                    if memory == Some(viewer_render_core::MemoryAdmissionError::TemporarilyBlocked)
                        && retry_demand.retiring_bytes > 0
                    {
                        self.retry_decode = Some((revision, retry_demand));
                        return;
                    }
                    if matches!(
                        memory,
                        Some(
                            viewer_render_core::MemoryAdmissionError::TemporarilyBlocked
                                | viewer_render_core::MemoryAdmissionError::ExceedsPolicy
                        )
                    ) {
                        return;
                    }
                    let mut retained = self.refinement_keys.clone();
                    retained.extend(self.preview_key);
                    renderer.retain_resources(&retained);
                    let code = match memory {
                        Some(viewer_render_core::MemoryAdmissionError::TemporarilyBlocked) => {
                            "image_render_memory_temporarily_blocked"
                        }
                        Some(viewer_render_core::MemoryAdmissionError::ExceedsPolicy) => {
                            "image_render_memory_policy_exceeded"
                        }
                        Some(_) => "image_render_memory_allocation_failed",
                        None => "image_render_decode_failed",
                    };
                    self.publish_failure(code, true);
                }
            }
        }
    }

    fn handle_command(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        command: AuthorizedImageRenderCommand,
    ) {
        match command {
            AuthorizedImageRenderCommand::Open {
                session_id,
                generation,
                source,
            } => self.open(renderer, session_id, generation, source),
            AuthorizedImageRenderCommand::SetSurface { layout } => self.resize(renderer, layout),
            AuthorizedImageRenderCommand::SetInputExclusions { .. } => {}
            AuthorizedImageRenderCommand::SetTool { tool } => {
                self.interaction.set_mode(tool);
                self.pending_draft_event = None;
                self.apply_interaction_scene(renderer);
            }
            AuthorizedImageRenderCommand::SetScene { scene } => {
                self.editor_anchor = scene_editor_anchor(&scene);
                self.scene = scene;
                if self.geometry_edit.as_ref().is_some_and(|edit| {
                    edit.completed
                        && self.scene.annotations().iter().any(|node| {
                            node.id == edit.annotation_id && node.geometry == edit.current
                        })
                }) {
                    self.geometry_edit = None;
                }
                self.apply_interaction_scene(renderer);
                self.queue_editor_placement();
            }
            AuthorizedImageRenderCommand::Camera { camera } => {
                self.camera = camera;
                self.update_transform(renderer);
            }
            AuthorizedImageRenderCommand::SetMagnifier { magnifier } => {
                self.magnifier_preferences = magnifier;
                self.update_magnifier(renderer);
            }
        }
    }

    fn open(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        session_id: viewer_render_core::RenderSessionId,
        generation: AssetGeneration,
        source: viewer_platform_macos::image_render::AuthorizedImageSource,
    ) {
        if self.generation.0 != 0 {
            self.provider.cancel_generation(self.generation);
        }
        self.session_id = Some(session_id);
        self.generation = generation;
        self.source_size = None;
        self.source = Some(source.clone());
        self.resource_revision = self.resource_revision.saturating_add(1);
        self.resource_plans.clear();
        self.detail_availability = None;
        self.demand_failed = false;
        self.pending_upload = None;
        self.pending_manifest = None;
        self.suspended_decode = false;
        self.retry_decode = None;
        self.refinement_keys.clear();
        self.preview_key = None;
        self.preview_bytes = 0;
        self.recovery = NativeRecoveryTracker::default();
        self.transform = None;
        self.magnifier_preferences = None;
        self.applied_magnifier = None;
        self.content_frames.begin_generation();
        self.magnifier_pointer = Some(LogicalPoint {
            x: self.viewport.logical_size.width / 2.0,
            y: self.viewport.logical_size.height / 2.0,
        });
        self.camera = CameraState::fit(Rotation::Deg0);
        self.scene = SceneSnapshot::empty(SceneRevision(0));
        self.rendered_scene = self.scene.clone();
        self.pending_rendered_scene = None;
        self.geometry_edit = None;
        self.pending_camera_event = None;
        self.pending_draft_event = None;
        self.pending_editor_placement_event = None;
        self.editor_anchor = None;
        if renderer.begin_asset_generation(generation).is_err() {
            self.publish_failure("image_render_generation_failed", true);
            return;
        }
        if renderer.apply_scene(&self.scene).is_err() {
            self.publish_failure("image_render_scene_failed", true);
        }
        let physical = self.viewport.physical_size();
        self.decode_results.clear();
        if self
            .decode_worker
            .submit(DecodeJob {
                session_id,
                generation,
                source,
                physical_width: physical.width.min(2048),
                physical_height: physical.height.min(2048),
                refinement: None,
                retry_demand: DecodeDemandSnapshot::capture(renderer),
            })
            .is_err()
        {
            self.publish_failure("image_render_decode_worker_unavailable", true);
        }
    }

    fn install_resource(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        session_id: viewer_render_core::RenderSessionId,
        generation: AssetGeneration,
        probe: ImageProbe,
        resource: PlatformDecodedResource,
    ) {
        if self.session_id != Some(session_id) || self.generation != generation {
            return;
        }
        let (source_width, source_height) = if matches!(probe.orientation, 5..=8) {
            (probe.height, probe.width)
        } else {
            (probe.width, probe.height)
        };
        let Ok(source_size) = SourceSize::new(source_width, source_height) else {
            self.publish_failure("image_render_source_invalid", false);
            return;
        };
        if resource.pixel_format != PixelFormat::Bgra8PremultipliedSrgb
            || resource.bytes_per_row != resource.width.saturating_mul(4)
        {
            self.publish_failure("image_render_resource_invalid", false);
            return;
        }
        let level = match resource.kind {
            DecodedResourceKind::Preview { level } => level,
            DecodedResourceKind::Tile(_) => {
                self.publish_failure("image_render_resource_invalid", false);
                return;
            }
        };
        let gpu_resource = DecodedResource::WholeImage {
            generation,
            level,
            source_size,
            width: resource.width,
            height: resource.height,
            pixels: resource.pixels,
        };
        self.pending_upload = Some(PendingActorUpload {
            resource: gpu_resource,
            handle: None,
            admission_snapshot: None,
            preview: Some(PendingPreview {
                session_id,
                generation,
                source_size,
                key: ResourceKey::WholeImage { level },
                bytes: u64::from(resource.width) * u64::from(resource.height) * 4,
            }),
        });
        self.progress_upload(renderer);
    }

    fn publish_completed_refinement(&mut self, renderer: &mut WgpuImageRenderer) {
        let Some(keys) = &self.pending_manifest else {
            return;
        };
        if self.pending_upload.is_some()
            || !keys.iter().all(|key| renderer.resource_key_is_ready(*key))
            || !self
                .latest_resource_keys()
                .iter()
                .all(|key| renderer.resource_key_is_ready(*key))
        {
            return;
        }
        self.refinement_keys = self.pending_manifest.take().unwrap();
        let mut retained = self.refinement_keys.clone();
        retained.extend(self.preview_key);
        renderer.retain_resources(&retained);
        self.publish_detail_availability(true);
    }

    fn progress_upload(&mut self, renderer: &mut WgpuImageRenderer) {
        self.retry_decode_after_release(renderer);
        let Some(mut pending) = self.pending_upload.take() else {
            self.publish_completed_refinement(renderer);
            return;
        };
        if pending.resource.generation() != self.generation {
            return;
        }
        if pending.handle.is_none() {
            let snapshot = renderer.memory().snapshot();
            let signature = (
                snapshot.combined_bytes,
                snapshot.gpu_bytes,
                snapshot.pressure_epoch,
            );
            if self.suspended_decode
                && pending.admission_snapshot.is_some()
                && snapshot.bytes_for_phase(viewer_render_core::AllocationPhase::Retiring) == 0
                && snapshot.bytes_for_class(viewer_render_core::AllocationClass::NativeDecode) == 0
                && pending.admission_snapshot == Some(signature)
            {
                self.demand_failed = true;
                self.suspended_decode = false;
                self.publish_detail_availability(false);
                return;
            }
            if pending.admission_snapshot == Some(signature) {
                self.pending_upload = Some(pending);
                return;
            }
            match renderer.upload_resource(pending.resource.clone()) {
                Ok(handle) => pending.handle = Some(handle),
                Err(RenderError::Upload(viewer_render_wgpu::UploadError::Memory(
                    viewer_render_core::MemoryAdmissionError::TemporarilyBlocked,
                ))) => {
                    self.publish_detail_availability(false);
                    pending.admission_snapshot = Some(signature);
                    // A worker's native mip must not depend on completion of a
                    // GPU batch that cannot currently fit. Cancel this delivery
                    // epoch; warm tiles remain cached for the later exact retry.
                    if !self.suspended_decode {
                        self.decode_worker.cancel_pending();
                        self.suspended_decode = true;
                        self.pending_manifest = None;
                        let mut retained = self.latest_resource_keys();
                        retained.extend(self.preview_key);
                        renderer.retain_resources(&retained);
                    }
                    self.pending_upload = Some(pending);
                    return;
                }
                Err(RenderError::Upload(viewer_render_wgpu::UploadError::Memory(
                    viewer_render_core::MemoryAdmissionError::ExceedsPolicy,
                ))) => {
                    self.decode_worker.cancel_pending();
                    self.demand_failed = true;
                    self.publish_detail_availability(false);
                    return;
                }
                Err(_) => {
                    self.decode_worker.cancel_pending();
                    self.demand_failed = true;
                    self.publish_failure("image_render_upload_failed", true);
                    return;
                }
            }
        }
        if !renderer.resource_is_ready(pending.handle.unwrap()) {
            self.pending_upload = Some(pending);
            return;
        }
        if let Some(preview) = pending.preview {
            self.preview_key = Some(preview.key);
            self.preview_bytes = preview.bytes;
            self.source_size = Some(preview.source_size);
            if !self.update_transform(renderer) {
                return;
            }
            self.content_frames.install(preview.generation);
            self.events.publish(ImageRenderEventDto::Ready {
                session_id: preview.session_id.0,
                asset_generation: preview.generation.0,
                width: preview.source_size.width,
                height: preview.source_size.height,
            });
        } else {
            self.content_frames.install(self.generation);
        }
        if self.suspended_decode {
            self.suspended_decode = false;
            self.demand_failed = true;
            self.schedule_refinement(renderer);
        }
        self.publish_completed_refinement(renderer);
    }

    fn retry_decode_after_release(&mut self, renderer: &mut WgpuImageRenderer) {
        let Some((revision, retry_demand)) = self.retry_decode else {
            return;
        };
        let snapshot = renderer.memory().snapshot();
        if snapshot.pressure_epoch == retry_demand.pressure_epoch
            && snapshot.combined_bytes >= retry_demand.combined_bytes
            && snapshot.gpu_bytes >= retry_demand.gpu_bytes
        {
            return;
        }
        self.retry_decode = None;
        if let Some(revision) = revision {
            if revision == self.resource_revision {
                self.schedule_refinement(renderer);
            }
        } else if let (Some(session_id), Some(source)) = (self.session_id, self.source.clone()) {
            let physical = self.viewport.physical_size();
            if self
                .decode_worker
                .submit(DecodeJob {
                    session_id,
                    generation: self.generation,
                    source,
                    physical_width: physical.width.min(2048),
                    physical_height: physical.height.min(2048),
                    refinement: None,
                    retry_demand: DecodeDemandSnapshot::capture(renderer),
                })
                .is_err()
            {
                self.publish_failure("image_render_decode_worker_unavailable", true);
            }
        }
    }

    fn latest_resource_keys(&self) -> Vec<ResourceKey> {
        self.resource_plans
            .iter()
            .flat_map(|plan| {
                plan.required_tiles
                    .iter()
                    .chain(&plan.prefetch_tiles)
                    .map(|tile| match plan.strategy {
                        TextureStrategy::SingleTexture => {
                            ResourceKey::WholeImage { level: plan.level }
                        }
                        TextureStrategy::Tiled { .. } => ResourceKey::Tile(*tile),
                    })
            })
            .collect()
    }

    fn resize(&mut self, renderer: &mut WgpuImageRenderer, layout: SurfaceLayout) {
        let Ok(viewport) = viewport_layout(layout) else {
            self.publish_failure("image_render_surface_invalid", false);
            return;
        };
        self.viewport = viewport;
        if renderer
            .resize(
                viewport.logical_size,
                viewport.physical_size(),
                viewport.scale_factor,
            )
            .is_err()
        {
            self.publish_failure("image_render_resize_failed", true);
            return;
        }
        self.update_transform(renderer);
    }

    fn update_transform(&mut self, renderer: &mut WgpuImageRenderer) -> bool {
        let Some(source_size) = self.source_size else {
            return false;
        };
        match TransformSnapshot::new(source_size, self.viewport, self.camera) {
            Ok(transform) => {
                if renderer.set_transform(transform).is_err() {
                    self.transform = None;
                    self.publish_failure("image_render_transform_failed", true);
                    return false;
                }
                self.transform = Some(transform);
                self.update_magnifier(renderer);
                self.schedule_refinement(renderer);
                self.queue_editor_placement();
                true
            }
            Err(_) => {
                self.transform = None;
                self.publish_failure("image_render_transform_invalid", false);
                false
            }
        }
    }

    fn handle_input(&mut self, renderer: &mut WgpuImageRenderer, input: NativeInput) {
        match input {
            NativeInput::Pointer(sample) => {
                self.magnifier_pointer = Some(sample.location);
                self.update_magnifier(renderer);
            }
            NativeInput::Hover(sample) => {
                self.magnifier_pointer = sample.active.then_some(sample.location);
                self.update_magnifier(renderer);
            }
            NativeInput::Cancel => {
                self.magnifier_pointer = None;
                self.update_magnifier(renderer);
            }
            NativeInput::Scroll(_) | NativeInput::Magnify(_) => {}
        }
        let Some(transform) = self.transform else {
            return;
        };
        let interaction_scene = self.pending_rendered_scene.as_ref().unwrap_or(&self.scene);
        for event in self
            .interaction
            .handle_input(input, &transform, interaction_scene)
        {
            self.publish_interaction(renderer, event);
        }
    }

    fn sync_rendered_scene(&mut self, renderer: &WgpuImageRenderer) {
        if !renderer.annotation_scene_upload_pending()
            && let Some(scene) = self.pending_rendered_scene.take()
        {
            self.rendered_scene = scene;
        }
    }

    fn update_magnifier(&mut self, renderer: &mut WgpuImageRenderer) {
        let config = self
            .magnifier_preferences
            .zip(self.magnifier_pointer)
            .zip(self.transform)
            .and_then(|((preferences, pointer), transform)| {
                magnifier_config_at_pointer(preferences, pointer, transform)
            });
        if config == self.applied_magnifier {
            return;
        }
        if renderer.set_magnifier(config).is_err() {
            self.publish_failure("image_render_magnifier_failed", true);
        } else {
            self.applied_magnifier = config;
            self.schedule_refinement(renderer);
        }
    }

    fn schedule_refinement(&mut self, renderer: &mut WgpuImageRenderer) {
        let (Some(source), Some(transform), Some(session_id)) =
            (&self.source, self.transform, self.session_id)
        else {
            return;
        };
        let Ok(mut plans) = refinement_plans(
            transform,
            self.applied_magnifier,
            renderer.max_texture_dimension_2d(),
            self.planning_budget(renderer),
            self.preview_bytes,
        ) else {
            self.resource_revision = self.resource_revision.saturating_add(1);
            self.resource_plans.clear();
            self.suspended_decode = false;
            self.retry_decode = None;
            self.pending_manifest = None;
            if self
                .pending_upload
                .as_ref()
                .is_some_and(|upload| upload.preview.is_none())
            {
                self.pending_upload = None;
                renderer.cancel_pending_upload();
            }
            self.decode_worker.cancel_pending();
            self.demand_failed = true;
            let snapshot = renderer.memory().snapshot();
            if snapshot.bytes_for_phase(viewer_render_core::AllocationPhase::Retiring) != 0
                || snapshot.bytes_for_class(viewer_render_core::AllocationClass::NativeDecode) != 0
            {
                self.retry_decode = Some((
                    Some(self.resource_revision),
                    DecodeDemandSnapshot::capture(renderer),
                ));
            } else {
                self.publish_detail_availability(false);
            }
            return;
        };
        if self.provider.memory().snapshot().pressure != viewer_render_core::PressureLevel::Normal {
            for plan in &mut plans {
                plan.prefetch_tiles.clear();
            }
        }
        if plans == self.resource_plans && !self.demand_failed {
            return;
        }
        self.resource_revision = self.resource_revision.saturating_add(1);
        self.resource_plans = plans.clone();
        renderer.touch_resources(&self.latest_resource_keys());
        // Suspension belongs to the cancelled worker epoch, not its successor.
        self.suspended_decode = false;
        self.retry_decode = None;
        self.demand_failed = false;
        self.pending_manifest = None;
        if self
            .pending_upload
            .as_ref()
            .is_some_and(|upload| upload.preview.is_none())
        {
            self.pending_upload = None;
            renderer.cancel_pending_upload();
        }
        let mut retained = self.refinement_keys.clone();
        retained.extend(self.latest_resource_keys());
        retained.extend(self.preview_key);
        renderer.retain_resources(&retained);
        if plans.is_empty() {
            self.decode_worker.cancel_pending();
            self.refinement_keys.clear();
            renderer.retain_resources(&self.preview_key.into_iter().collect::<Vec<_>>());
            self.publish_detail_availability(true);
            return;
        }
        let physical = self.viewport.physical_size();
        if self
            .decode_worker
            .submit(DecodeJob {
                session_id,
                generation: self.generation,
                source: source.clone(),
                physical_width: physical.width,
                physical_height: physical.height,
                refinement: Some((self.resource_revision, transform.source(), plans)),
                retry_demand: DecodeDemandSnapshot::capture(renderer),
            })
            .is_err()
        {
            self.publish_failure("image_render_decode_worker_unavailable", true);
        }
    }

    fn publish_interaction(&mut self, renderer: &mut WgpuImageRenderer, event: InteractionEvent) {
        let Some(session_id) = self.session_id else {
            return;
        };
        let generation = self.generation.0;
        let event = match event {
            InteractionEvent::GeometryEditStarted {
                annotation_id,
                geometry,
                handle,
            } => {
                self.geometry_edit = Some(NativeGeometryEdit {
                    annotation_id: annotation_id.clone(),
                    original: geometry.clone(),
                    current: geometry.clone(),
                    completed: false,
                });
                self.apply_interaction_scene(renderer);
                ImageRenderEventDto::GeometryEditStarted {
                    session_id: session_id.0,
                    asset_generation: generation,
                    annotation_id: annotation_id.as_str().to_owned(),
                    geometry: annotation_geometry_dto(geometry),
                    handle: handle.map(|handle| {
                        use viewer_render_core::AnnotationHandle;
                        match handle {
                            AnnotationHandle::Point => "point",
                            AnnotationHandle::Tail => "tail",
                            AnnotationHandle::Head => "head",
                            AnnotationHandle::NorthWest => "north_west",
                            AnnotationHandle::NorthEast => "north_east",
                            AnnotationHandle::SouthEast => "south_east",
                            AnnotationHandle::SouthWest => "south_west",
                        }
                        .to_owned()
                    }),
                }
            }
            InteractionEvent::GeometryEditChanged {
                annotation_id,
                geometry,
            } => {
                if let Some(edit) = self.geometry_edit.as_mut() {
                    edit.current = geometry.clone();
                }
                self.apply_interaction_scene(renderer);
                self.pending_draft_event = Some(ImageRenderEventDto::GeometryEditChanged {
                    session_id: session_id.0,
                    asset_generation: generation,
                    annotation_id: annotation_id.as_str().to_owned(),
                    geometry: annotation_geometry_dto(geometry),
                });
                return;
            }
            InteractionEvent::GeometryEditCompleted {
                annotation_id,
                geometry,
            } => {
                self.pending_draft_event = None;
                if let Some(edit) = self.geometry_edit.as_mut() {
                    edit.current = geometry.clone();
                    edit.completed = true;
                }
                self.apply_interaction_scene(renderer);
                self.editor_anchor = Some(annotation_editor_anchor(&geometry));
                self.queue_editor_placement();
                ImageRenderEventDto::GeometryEditCompleted {
                    session_id: session_id.0,
                    asset_generation: generation,
                    annotation_id: annotation_id.as_str().to_owned(),
                    geometry: annotation_geometry_dto(geometry),
                }
            }
            InteractionEvent::GeometryEditCancelled { annotation_id } => {
                self.pending_draft_event = None;
                if let Some(edit) = self.geometry_edit.as_mut() {
                    edit.current = edit.original.clone();
                    edit.completed = true;
                }
                self.apply_interaction_scene(renderer);
                ImageRenderEventDto::GeometryEditCancelled {
                    session_id: session_id.0,
                    asset_generation: generation,
                    annotation_id: annotation_id.as_str().to_owned(),
                }
            }
            InteractionEvent::CameraChanged(camera) => {
                self.camera = camera;
                self.update_transform(renderer);
                self.pending_camera_event = Some(ImageRenderEventDto::CameraChanged {
                    session_id: session_id.0,
                    asset_generation: generation,
                    camera: camera_dto(camera),
                });
                return;
            }
            InteractionEvent::DraftStarted(geometry) => {
                self.apply_native_draft(renderer, draft_annotation_geometry(geometry.clone()));
                ImageRenderEventDto::DraftStarted {
                    session_id: session_id.0,
                    asset_generation: generation,
                    geometry: draft_geometry_dto(geometry),
                }
            }
            InteractionEvent::DraftChanged(geometry) => {
                self.apply_native_draft(renderer, draft_annotation_geometry(geometry.clone()));
                self.pending_draft_event = Some(ImageRenderEventDto::DraftChanged {
                    session_id: session_id.0,
                    asset_generation: generation,
                    geometry: draft_geometry_dto(geometry),
                });
                return;
            }
            InteractionEvent::DraftCompleted(geometry) => {
                self.pending_draft_event = None;
                self.editor_anchor = Some(annotation_editor_anchor(&geometry));
                self.apply_native_draft(renderer, geometry.clone());
                ImageRenderEventDto::DraftCompleted {
                    session_id: session_id.0,
                    asset_generation: generation,
                    geometry: annotation_geometry_dto(geometry),
                }
            }
            InteractionEvent::DraftCancelled => {
                self.pending_draft_event = None;
                self.editor_anchor = None;
                self.apply_scene_projection(renderer, self.scene.clone(), false);
                ImageRenderEventDto::DraftCancelled {
                    session_id: session_id.0,
                    asset_generation: generation,
                }
            }
            InteractionEvent::SelectionChanged(annotation_id) => {
                self.apply_native_selection(renderer, annotation_id.as_ref());
                ImageRenderEventDto::SelectionChanged {
                    session_id: session_id.0,
                    asset_generation: generation,
                    annotation_id: annotation_id.map(|id| id.as_str().to_owned()),
                }
            }
            InteractionEvent::EditorPlacementChanged(position) => {
                self.pending_editor_placement_event =
                    Some(ImageRenderEventDto::EditorPlacementChanged {
                        session_id: session_id.0,
                        asset_generation: generation,
                        position: point_dto(position.x, position.y),
                    });
                return;
            }
        };
        self.events.publish(event);
    }

    fn apply_interaction_scene(&mut self, renderer: &mut WgpuImageRenderer) {
        let (projected, transient) = if let Some(edit) = &self.geometry_edit {
            let mut annotations = self.scene.annotations().to_vec();
            if let Some(node) = annotations
                .iter_mut()
                .find(|node| node.id == edit.annotation_id)
            {
                node.geometry = edit.current.clone();
                node.selected = true;
            }
            let projected = SceneSnapshot::new(
                self.scene.revision(),
                annotations,
                self.scene.draft().cloned(),
            )
            .ok()
            .map(|scene| scene.with_annotations_editable(self.scene.annotations_editable()));
            let Some(projected) = projected else {
                self.publish_failure("image_render_scene_failed", true);
                return;
            };
            (projected, true)
        } else {
            (self.scene.clone(), false)
        };
        self.apply_scene_projection(renderer, projected, transient);
    }

    fn apply_scene_projection(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        projected: SceneSnapshot,
        transient: bool,
    ) {
        let result = if transient {
            renderer.apply_transient_scene(&projected)
        } else {
            renderer.apply_scene(&projected)
        };
        match result {
            Ok(_) => {
                self.rendered_scene = projected;
                self.pending_rendered_scene = None;
            }
            Err(error) if is_temporary_memory_error(&error) => {
                // Keep the CPU source of truth for the next retry, but use the
                // last scene known to be resident for hit testing until the
                // renderer has admitted this projection.
                self.pending_rendered_scene = Some(projected);
            }
            Err(_) => self.publish_failure("image_render_scene_failed", true),
        }
    }

    fn apply_native_draft(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        geometry: AnnotationGeometry,
    ) {
        // Pointer-down and axis-only moves are legitimate intermediate states,
        // not failed annotations. They have no drawable area/segment yet. In
        // particular, do not send zero-length rectangle edges to the mesh pass
        // and turn a normal gesture into a terminal renderer failure.
        let drawable = match &geometry {
            AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => {
                rect.width > 0.0 && rect.height > 0.0
            }
            _ => true,
        };
        let draft = AnnotationNode::new(
            AnnotationId::new("__viewer_native_draft").expect("static annotation id is valid"),
            0,
            geometry,
        );
        let mut draft = match draft {
            Ok(draft) if drawable => draft,
            _ => {
                // Also clear a previous drawable sample when dragging back
                // through the origin, without rebuilding committed geometry.
                if renderer.clear_draft_overlay().is_err() {
                    self.publish_failure("image_render_scene_failed", true);
                }
                return;
            }
        };
        draft.draft = true;
        draft.style.dashed = true;
        let Ok(projected) = SceneSnapshot::new(self.scene.revision(), Vec::new(), Some(draft))
        else {
            return;
        };
        let projected = projected.with_annotations_editable(self.scene.annotations_editable());
        if let Err(error) = renderer.apply_draft_overlay(&projected)
            && !is_temporary_memory_error(&error)
        {
            self.publish_failure("image_render_scene_failed", true);
        }
    }

    fn queue_editor_placement(&mut self) {
        let Some(session_id) = self.session_id else {
            return;
        };
        let Some(position) = self
            .editor_anchor
            .zip(self.transform)
            .map(|(anchor, transform)| transform.image_to_view(anchor))
        else {
            return;
        };
        self.pending_editor_placement_event = Some(ImageRenderEventDto::EditorPlacementChanged {
            session_id: session_id.0,
            asset_generation: self.generation.0,
            position: point_dto(position.x, position.y),
        });
    }

    fn apply_native_selection(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        selected: Option<&AnnotationId>,
    ) {
        let mut annotations = self.scene.annotations().to_vec();
        for annotation in &mut annotations {
            annotation.selected = selected == Some(&annotation.id);
        }
        let Ok(projected) = SceneSnapshot::new(
            self.scene.revision(),
            annotations,
            self.scene.draft().cloned(),
        ) else {
            self.publish_failure("image_render_scene_failed", true);
            return;
        };
        let projected = projected.with_annotations_editable(self.scene.annotations_editable());
        self.apply_scene_projection(renderer, projected.clone(), true);
        // Selection is renderer-owned interaction state. The next native
        // boundary must observe it before React sends its scene projection.
        self.scene = projected;
    }

    fn flush_continuous_events(&mut self) {
        for event in [
            self.pending_camera_event.take(),
            self.pending_draft_event.take(),
            self.pending_editor_placement_event.take(),
        ]
        .into_iter()
        .flatten()
        {
            self.events.publish(event);
        }
    }

    fn handle_render_error(&mut self, renderer: &mut WgpuImageRenderer, error: RenderError) {
        if is_temporary_memory_error(&error) {
            // The renderer keeps the latest scene/overlay upload dirty and
            // schedules another frame. Memory pressure and GPU retirement
            // wakes are not renderer failures and must never trigger Web
            // fallback or terminate the native session.
            return;
        }
        let Some(surface_recovery) = error.recovery() else {
            self.recovery.record_terminal_failure();
            self.publish_failure("image_render_frame_failed", false);
            return;
        };
        if surface_recovery == SurfaceRecovery::WaitUntilVisible {
            renderer.recover_surface(surface_recovery);
            return;
        }
        if matches!(
            surface_recovery,
            SurfaceRecovery::ReportValidation | SurfaceRecovery::Terminate
        ) {
            self.recovery.record_terminal_failure();
            self.publish_failure("image_render_frame_failed", false);
            return;
        }
        if self.recovery.record_retryable_failure() == NativeRecoveryDecision::RecoverNative
            && renderer.recover_surface(surface_recovery)
        {
            if let Some(session_id) = self.session_id {
                self.events.publish(ImageRenderEventDto::Recovering {
                    session_id: session_id.0,
                    asset_generation: self.generation.0,
                    reason: surface_recovery_code(surface_recovery).to_owned(),
                });
            }
        } else {
            self.publish_failure("image_render_frame_failed", true);
        }
    }

    fn publish_detail_availability(&mut self, available: bool) {
        let Some(session_id) = self.session_id else {
            return;
        };
        let state = (self.resource_revision, available);
        if self.detail_availability == Some(state) {
            return;
        }
        self.detail_availability = Some(state);
        self.events
            .publish(ImageRenderEventDto::DetailAvailabilityChanged {
                session_id: session_id.0,
                asset_generation: self.generation.0,
                resource_revision: self.resource_revision,
                available,
            });
    }

    fn publish_failure(&self, code: &str, retryable: bool) {
        if let Some(session_id) = self.session_id {
            self.events.publish(ImageRenderEventDto::Failed {
                session_id: session_id.0,
                asset_generation: self.generation.0,
                code: code.to_owned(),
                retryable,
            });
        }
    }
}

fn is_temporary_memory_error(error: &RenderError) -> bool {
    matches!(
        error,
        RenderError::Memory(viewer_render_core::MemoryAdmissionError::TemporarilyBlocked)
            | RenderError::AnnotationMesh(viewer_render_wgpu::MeshError::Memory(
                viewer_render_core::MemoryAdmissionError::TemporarilyBlocked,
            ))
            | RenderError::GlyphAtlas(viewer_render_wgpu::GlyphAtlasError::Memory(
                viewer_render_core::MemoryAdmissionError::TemporarilyBlocked,
            ))
            | RenderError::Upload(viewer_render_wgpu::UploadError::Memory(
                viewer_render_core::MemoryAdmissionError::TemporarilyBlocked,
            ))
    )
}

fn surface_recovery_code(recovery: SurfaceRecovery) -> &'static str {
    match recovery {
        SurfaceRecovery::RetryNextFrame => "surface_timeout",
        SurfaceRecovery::WaitUntilVisible => "surface_occluded",
        SurfaceRecovery::Reconfigure => "surface_outdated",
        SurfaceRecovery::RecreateSurface => "surface_lost",
        SurfaceRecovery::ReportValidation => "surface_validation",
        SurfaceRecovery::Terminate => "gpu_out_of_memory",
    }
}

fn scene_editor_anchor(scene: &SceneSnapshot) -> Option<NormalizedPoint> {
    scene
        .draft()
        .or_else(|| scene.annotations().iter().find(|node| node.draft))
        .map(|node| annotation_editor_anchor(&node.geometry))
}

fn annotation_editor_anchor(geometry: &AnnotationGeometry) -> NormalizedPoint {
    match geometry {
        AnnotationGeometry::Point { position } => *position,
        AnnotationGeometry::Arrow { head, .. } => *head,
        AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => {
            NormalizedPoint {
                x: rect.x + rect.width / 2.0,
                y: rect.y + rect.height / 2.0,
            }
        }
        AnnotationGeometry::Stroke { points } => points
            .last()
            .copied()
            .expect("validated stroke geometry always contains points"),
    }
}

fn renderer_descriptor(
    handles: SurfaceHandles,
    layout: SurfaceLayout,
) -> Result<RendererDescriptor, String> {
    let viewport = viewport_layout(layout).map_err(|error| error.to_string())?;
    RendererDescriptor::with_surface(
        handles,
        viewport.logical_size,
        viewport.physical_size(),
        viewport.scale_factor,
    )
    .map_err(|error| error.to_string())
}

fn viewport_layout(layout: SurfaceLayout) -> Result<ViewportLayout, ImageRenderRuntimeError> {
    let logical_size = LogicalSize::new(layout.width, layout.height)
        .map_err(|_| ImageRenderRuntimeError::InvalidCommand)?;
    ViewportLayout::new(logical_size, layout.scale_factor, FIT_INSET)
        .map_err(|_| ImageRenderRuntimeError::InvalidCommand)
}

fn magnifier_config_at_pointer(
    preferences: ImageRenderMagnifierPreferences,
    pointer: LogicalPoint,
    transform: TransformSnapshot,
) -> Option<MagnifierConfig> {
    let focus = transform.view_to_image(pointer)?;
    let viewport = transform.viewport().logical_size;
    let center = LogicalPoint {
        x: place_magnifier_axis(pointer.x, viewport.width, preferences.width_px),
        y: place_magnifier_axis(pointer.y, viewport.height, preferences.height_px),
    };
    MagnifierConfig::new(
        focus,
        center,
        preferences.width_px,
        preferences.height_px,
        preferences.magnification,
        preferences.shape,
    )
    .ok()
}

fn place_magnifier_axis(pointer: f64, stage: f64, lens: f64) -> f64 {
    let positive_start = pointer + MAGNIFIER_POINTER_GAP;
    let negative_start = pointer - MAGNIFIER_POINTER_GAP - lens;
    let positive_fits = positive_start + lens <= stage;
    let negative_fits = negative_start >= 0.0;
    let use_positive = positive_fits || (!negative_fits && stage - pointer >= pointer);
    let start = if use_positive {
        positive_start
    } else {
        negative_start
    }
    .clamp(0.0, (stage - lens).max(0.0));
    start + lens / 2.0
}

fn camera_dto(camera: CameraState) -> ImageRenderCameraDto {
    ImageRenderCameraDto {
        mode: match camera.mode {
            viewer_render_core::CameraMode::Fit => ImageRenderCameraModeDto::Fit,
            viewer_render_core::CameraMode::Free => ImageRenderCameraModeDto::Free,
        },
        zoom: camera.zoom,
        rotation: match camera.rotation {
            Rotation::Deg0 => ImageRenderRotationDto::Deg0,
            Rotation::Deg90 => ImageRenderRotationDto::Deg90,
            Rotation::Deg180 => ImageRenderRotationDto::Deg180,
            Rotation::Deg270 => ImageRenderRotationDto::Deg270,
        },
        offset: point_dto(camera.offset.x, camera.offset.y),
    }
}

fn draft_annotation_geometry(geometry: DraftGeometry) -> AnnotationGeometry {
    match geometry {
        DraftGeometry::Point(position) => AnnotationGeometry::Point { position },
        DraftGeometry::Arrow { tail, head } => AnnotationGeometry::Arrow { tail, head },
        DraftGeometry::Brush(points) => AnnotationGeometry::Stroke { points },
        DraftGeometry::Rectangle { start, end } => AnnotationGeometry::Rectangle {
            rect: normalized_bounds(start, end),
        },
        DraftGeometry::Ellipse { start, end } => AnnotationGeometry::Ellipse {
            rect: normalized_bounds(start, end),
        },
    }
}

fn normalized_bounds(start: NormalizedPoint, end: NormalizedPoint) -> NormalizedRect {
    NormalizedRect {
        x: start.x.min(end.x),
        y: start.y.min(end.y),
        width: (start.x - end.x).abs(),
        height: (start.y - end.y).abs(),
    }
}

fn draft_geometry_dto(geometry: DraftGeometry) -> ImageRenderAnnotationGeometryDto {
    match geometry {
        DraftGeometry::Point(position) => ImageRenderAnnotationGeometryDto::Point {
            position: normalized_point_dto(position),
        },
        DraftGeometry::Arrow { tail, head } => ImageRenderAnnotationGeometryDto::Arrow {
            tail: normalized_point_dto(tail),
            head: normalized_point_dto(head),
        },
        DraftGeometry::Brush(points) => ImageRenderAnnotationGeometryDto::Stroke {
            points: points.into_iter().map(normalized_point_dto).collect(),
        },
        DraftGeometry::Rectangle { start, end } => ImageRenderAnnotationGeometryDto::Rectangle {
            rect: normalized_bounds_dto(start, end),
        },
        DraftGeometry::Ellipse { start, end } => ImageRenderAnnotationGeometryDto::Ellipse {
            rect: normalized_bounds_dto(start, end),
        },
    }
}

fn annotation_geometry_dto(geometry: AnnotationGeometry) -> ImageRenderAnnotationGeometryDto {
    match geometry {
        AnnotationGeometry::Point { position } => ImageRenderAnnotationGeometryDto::Point {
            position: normalized_point_dto(position),
        },
        AnnotationGeometry::Arrow { tail, head } => ImageRenderAnnotationGeometryDto::Arrow {
            tail: normalized_point_dto(tail),
            head: normalized_point_dto(head),
        },
        AnnotationGeometry::Rectangle { rect } => ImageRenderAnnotationGeometryDto::Rectangle {
            rect: normalized_rect_dto(rect),
        },
        AnnotationGeometry::Ellipse { rect } => ImageRenderAnnotationGeometryDto::Ellipse {
            rect: normalized_rect_dto(rect),
        },
        AnnotationGeometry::Stroke { points } => ImageRenderAnnotationGeometryDto::Stroke {
            points: points.into_iter().map(normalized_point_dto).collect(),
        },
    }
}

fn normalized_bounds_dto(
    start: NormalizedPoint,
    end: NormalizedPoint,
) -> crate::dto::ImageRenderNormalizedRectDto {
    crate::dto::ImageRenderNormalizedRectDto {
        x: start.x.min(end.x),
        y: start.y.min(end.y),
        width: (start.x - end.x).abs(),
        height: (start.y - end.y).abs(),
    }
}

fn normalized_rect_dto(rect: NormalizedRect) -> crate::dto::ImageRenderNormalizedRectDto {
    crate::dto::ImageRenderNormalizedRectDto {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn normalized_point_dto(point: NormalizedPoint) -> ImageRenderPointDto {
    point_dto(point.x, point.y)
}

const fn point_dto(x: f64, y: f64) -> ImageRenderPointDto {
    ImageRenderPointDto { x, y }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_render_core::{
        CameraMode, HoverSample, LogicalPoint, MagnifySample, Modifiers, PhysicalSize,
        PointerButton, PointerSample, ScrollSample,
    };

    #[test]
    fn driver_pressure_sink_changes_shared_admission_without_a_surface_and_ignores_retired_owner() {
        use viewer_render_core::{AllocationClass, MemoryAdmissionError, PressureLevel};
        let cache = tempfile::tempdir().unwrap();
        let driver = NativeImageRenderDriver::new(
            cache.path(),
            Arc::new(crate::image_render_events::RecordingImageRenderEvents::default()),
        );
        let memory = driver.memory.clone();
        let sink = driver.memory_pressure_sink();
        sink(PressureLevel::Warning);
        let warning = memory.snapshot();
        assert_eq!(warning.limits.gpu_bytes, 128 << 20);
        assert_eq!(warning.limits.combined_bytes, 256 << 20);
        assert_eq!(
            memory
                .try_reserve(AllocationClass::GpuTexture, 129 << 20, AssetGeneration(1))
                .unwrap_err(),
            MemoryAdmissionError::TemporarilyBlocked
        );
        sink(PressureLevel::Warning);
        assert_eq!(memory.snapshot().pressure_epoch, warning.pressure_epoch);
        sink(PressureLevel::Critical);
        assert_eq!(memory.snapshot().limits.gpu_bytes, 64 << 20);
        sink(PressureLevel::Normal);
        assert!(
            memory
                .try_reserve(AllocationClass::GpuTexture, 129 << 20, AssetGeneration(1))
                .is_ok()
        );
        sink(PressureLevel::Critical);
        for flags in [0x5, 0, 0x8, 0x9] {
            if let Some(level) =
                viewer_platform_macos::image_render::pressure_level_for_flags(flags)
            {
                sink(level);
            }
        }
        assert_eq!(
            memory.snapshot().pressure,
            PressureLevel::Critical,
            "coalesced Critical+Normal is not chronological Normal, and unknown data cannot restore admission"
        );
        drop(driver);
        sink(PressureLevel::Normal);
        assert_eq!(
            memory.snapshot().pressure,
            PressureLevel::Critical,
            "retired publisher cannot reset surviving ownership admission"
        );
    }

    #[test]
    fn exact_texture_pixels_cannot_consume_capacity_needed_by_resource_vertices() {
        let source = SourceSize::new(1024, 1024).unwrap();
        let viewport =
            ViewportLayout::new(LogicalSize::new(1024.0, 1024.0).unwrap(), 1.0, 1.0).unwrap();
        let transform =
            TransformSnapshot::new(source, viewport, CameraState::fit(Rotation::Deg0)).unwrap();
        let budget = MemoryBudget::new(16 << 20, 4 << 20, 1 << 20).unwrap();
        assert!(
            refinement_plans(transform, None, 8192, budget, 0).is_err(),
            "four 512 tiles also need their GPU vertex buffers"
        );
    }

    #[test]
    fn pressure_removing_last_coverage_closes_content_gate_until_a_real_replacement_is_ready() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        use viewer_render_core::{
            ImageMemoryPolicy, LiveMemoryLimits, PressureLevel, SharedPixels,
        };
        let normal = LiveMemoryLimits {
            combined_bytes: 32 << 20,
            gpu_bytes: 16 << 20,
        };
        let critical = LiveMemoryLimits {
            combined_bytes: 8 << 20,
            gpu_bytes: 2 << 20,
        };
        let (_cache, mut actor, mut renderer, _) =
            test_actor_with_policy(ImageMemoryPolicy::new(normal, normal, critical).unwrap());
        let generation = AssetGeneration(1);
        let source_size = SourceSize::new(1, 1).unwrap();
        let memory = actor.provider.memory().clone();
        actor.generation = generation;
        actor.source_size = Some(source_size);
        actor.preview_key = Some(ResourceKey::WholeImage { level: 0 });
        actor.content_frames.install(generation);
        let resource = || DecodedResource::WholeImage {
            generation,
            level: 0,
            source_size,
            width: 1,
            height: 1,
            pixels: SharedPixels::try_zeroed(&memory, generation, 4).unwrap(),
        };
        renderer.upload_resource(resource()).unwrap();
        drain_uploads(&mut actor, &mut renderer);
        let publisher = Arc::new(DriverPressure {
            memory: memory.clone(),
            wake: Mutex::new(None),
        });
        let sink = DriverPressure::sink(&publisher);
        sink(PressureLevel::Critical);
        actor.apply_memory_pressure(&mut renderer);
        assert!(!renderer.resource_key_is_ready(actor.preview_key.unwrap()));
        assert!(
            !actor.content_frames.can_publish(generation),
            "retired last coverage must not authorize a clear frame"
        );
        sink(PressureLevel::Normal);
        actor.apply_memory_pressure(&mut renderer);
        assert!(!actor.content_frames.can_publish(generation));
        actor.pending_upload = Some(PendingActorUpload {
            resource: resource(),
            handle: None,
            preview: None,
            admission_snapshot: None,
        });
        drain_uploads(&mut actor, &mut renderer);
        assert!(actor.content_frames.can_publish(generation));
    }

    #[test]
    fn pressure_cancels_a_full_actual_worker_fifo_and_releases_native_backing() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none()
            || std::env::var_os("VIEWER_RUN_LARGE_IMAGE_TESTS").is_none()
        {
            return;
        }
        use viewer_render_core::{AllocationClass, PressureLevel};
        let (cache, mut actor, mut renderer, events) = test_actor();
        let fixture = cache.path().join("pressure-4096.jpg");
        assert!(
            std::process::Command::new("/usr/bin/sips")
                .args(["--resampleHeightWidth", "4096", "4096"])
                .arg(viewer_test_support::image_fixtures::image_fixture(
                    "srgb.jpg"
                ))
                .arg("--out")
                .arg(&fixture)
                .output()
                .unwrap()
                .status
                .success()
        );
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                &fixture,
            )
            .unwrap();
        let generation = AssetGeneration(1);
        let session = viewer_render_core::RenderSessionId(1);
        actor.open(&mut renderer, session, generation, source.clone());
        actor.viewport =
            ViewportLayout::new(LogicalSize::new(4096.0, 4096.0).unwrap(), 1.0, 1.0).unwrap();
        let (probe, preview) = actor
            .provider
            .probe_and_request_preview(&source, generation, PreviewRequest::new(64, 64).unwrap())
            .unwrap();
        actor.install_resource(&mut renderer, session, generation, probe, preview);
        drain_uploads(&mut actor, &mut renderer);
        actor.decode_worker.shutdown();
        actor.decode_results = Arc::default();
        actor.decode_worker = DecodeWorker::start(
            actor.provider.clone(),
            actor.decode_results.clone(),
            DisplayTickSignal::default(),
        )
        .unwrap();
        actor.demand_failed = true;
        actor.schedule_refinement(&mut renderer);
        let old_revision = actor.resource_revision;
        let memory = actor.provider.memory().clone();
        let publisher = Arc::new(DriverPressure {
            memory: memory.clone(),
            wake: Mutex::new(None),
        });
        let sink = DriverPressure::sink(&publisher);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while actor.decode_results.queued_bytes() < handoff::HANDOFF_BYTES {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            memory
                .snapshot()
                .bytes_for_class(AllocationClass::NativeDecode)
                >= 64 << 20
        );
        sink(PressureLevel::Critical);
        let pressured = memory.snapshot();
        eprintln!(
            "pressure full FIFO: combined={} native={} decoded={} gpu={} current_combined_limit={}",
            pressured.combined_bytes,
            pressured.bytes_for_class(AllocationClass::NativeDecode),
            pressured.bytes_for_class(AllocationClass::DecodedPixels),
            pressured.gpu_bytes,
            pressured.limits.combined_bytes
        );
        assert!(
            memory.snapshot().combined_bytes > memory.snapshot().limits.combined_bytes,
            "lowering limits does not fake-release live decode storage"
        );
        actor.apply_memory_pressure(&mut renderer);
        assert_eq!(actor.decode_results.queued_bytes(), 0);
        while memory
            .snapshot()
            .bytes_for_class(AllocationClass::NativeDecode)
            != 0
        {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            memory
                .snapshot()
                .bytes_for_class(AllocationClass::DecodedPixels),
            0
        );
        eprintln!(
            "pressure relieved: combined={} gpu={} native=0 decoded=0",
            memory.snapshot().combined_bytes,
            memory.snapshot().gpu_bytes
        );
        assert!(renderer.resource_key_is_ready(actor.preview_key.unwrap()));
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::RefinementCompleted {
                session_id: session,
                generation,
                revision: old_revision,
                keys: vec![],
            },
        );
        assert!(actor.pending_manifest.is_none());
        actor.progress_upload(&mut renderer);
        assert!(events.take().iter().any(|event| matches!(
            event,
            ImageRenderEventDto::DetailAvailabilityChanged {
                available: false,
                ..
            }
        )));
        let revision = actor.resource_revision;
        for _ in 0..20 {
            sink(PressureLevel::Critical);
            actor.apply_memory_pressure(&mut renderer);
        }
        assert_eq!(actor.resource_revision, revision);
        assert!(lock(&actor.decode_worker.jobs.0).latest.is_none());
        actor.decode_worker.shutdown();
    }

    fn test_actor() -> (
        tempfile::TempDir,
        RendererActor,
        WgpuImageRenderer,
        Arc<crate::image_render_events::RecordingImageRenderEvents>,
    ) {
        test_actor_with_policy(viewer_render_core::ImageMemoryPolicy::baseline_8gb())
    }

    #[test]
    fn actor_pressure_sink_retires_real_detail_and_normal_restores_latest_without_input() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        use viewer_render_core::{
            AllocationClass, AllocationPhase, ImageMemoryPolicy, LiveMemoryLimits, PressureLevel,
            SharedPixels,
        };
        let normal = LiveMemoryLimits {
            combined_bytes: 64 << 20,
            gpu_bytes: 32 << 20,
        };
        let warning = LiveMemoryLimits {
            combined_bytes: 32 << 20,
            gpu_bytes: 8 << 20,
        };
        let critical = LiveMemoryLimits {
            combined_bytes: 16 << 20,
            gpu_bytes: 4 << 20,
        };
        let (_cache, mut actor, mut renderer, events) =
            test_actor_with_policy(ImageMemoryPolicy::new(normal, warning, critical).unwrap());
        let memory = actor.provider.memory().clone();
        let publisher = Arc::new(DriverPressure {
            memory: memory.clone(),
            wake: Mutex::new(None),
        });
        let sink = DriverPressure::sink(&publisher);
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        let generation = AssetGeneration(1);
        let size = SourceSize::new(1024, 768).unwrap();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = generation;
        actor.source = Some(source.clone());
        actor.source_size = Some(size);
        actor.viewport =
            ViewportLayout::new(LogicalSize::new(1024.0, 768.0).unwrap(), 1.0, 1.0).unwrap();
        actor.transform = Some(TransformSnapshot::new(size, actor.viewport, actor.camera).unwrap());
        let cached = actor
            .provider
            .request_preview(&source, generation, PreviewRequest::new(64, 64).unwrap())
            .unwrap();
        let cache_path = actor.provider.cache_entry_path(&cached.cache_key);
        drop(cached);
        for level in [0, 1, 8] {
            let (width, height) = if level == 8 { (1, 1) } else { (1024, 768) };
            renderer
                .upload_resource(DecodedResource::WholeImage {
                    generation,
                    level,
                    source_size: size,
                    width,
                    height,
                    pixels: SharedPixels::try_zeroed(
                        &memory,
                        generation,
                        u64::from(width * height * 4),
                    )
                    .unwrap(),
                })
                .unwrap();
            drain_uploads(&mut actor, &mut renderer);
        }
        let visible = ResourceKey::WholeImage { level: 0 };
        let old = ResourceKey::WholeImage { level: 1 };
        let preview = ResourceKey::WholeImage { level: 8 };
        actor.preview_key = Some(preview);
        actor.preview_bytes = 4;
        actor.refinement_keys = vec![visible];
        actor.resource_plans = vec![ResourcePlan {
            strategy: TextureStrategy::SingleTexture,
            level: 0,
            required_tiles: vec![viewer_render_core::TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }],
            prefetch_tiles: vec![],
        }];
        let camera = actor.camera;
        let saved = AnnotationNode::new(
            AnnotationId::new("saved-pressure").unwrap(),
            1,
            AnnotationGeometry::Point {
                position: NormalizedPoint::new(0.2, 0.3).unwrap(),
            },
        )
        .unwrap();
        let mut draft = AnnotationNode::new(
            AnnotationId::new("draft-pressure").unwrap(),
            2,
            AnnotationGeometry::Point {
                position: NormalizedPoint::new(0.6, 0.7).unwrap(),
            },
        )
        .unwrap();
        draft.draft = true;
        actor.scene = SceneSnapshot::new(SceneRevision(7), vec![saved], Some(draft)).unwrap();
        actor.editor_anchor = Some(NormalizedPoint::new(0.6, 0.7).unwrap());
        renderer.apply_scene(&actor.scene).unwrap();
        let scene = actor.scene.clone();
        let editor_anchor = actor.editor_anchor;
        sink(PressureLevel::Warning);
        actor.apply_memory_pressure(&mut renderer);
        assert!(
            !renderer.resource_key_is_ready(old),
            "platform sink must retire live obsolete GPU ownership, not disk files"
        );
        assert!(renderer.resource_key_is_ready(visible));
        assert!(renderer.resource_key_is_ready(preview));
        assert!(memory.snapshot().bytes_for_phase(AllocationPhase::Retiring) >= 3 << 20);
        assert!(
            cache_path.exists(),
            "RAM pressure must not destroy warm disk entries"
        );
        let revision = actor.resource_revision;
        for _ in 0..20 {
            sink(PressureLevel::Warning);
            actor.apply_memory_pressure(&mut renderer);
        }
        assert_eq!(actor.resource_revision, revision);
        renderer.poll_gpu_timings().unwrap();
        sink(PressureLevel::Critical);
        actor.apply_memory_pressure(&mut renderer);
        assert!(!renderer.resource_key_is_ready(visible));
        assert!(renderer.resource_key_is_ready(preview));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while memory.snapshot().bytes_for_phase(AllocationPhase::Retiring) != 0 {
            assert!(std::time::Instant::now() < deadline);
            renderer.poll_gpu_timings().unwrap();
        }
        assert!(memory.snapshot().gpu_bytes <= critical.gpu_bytes);
        actor.progress_upload(&mut renderer);
        assert!(events.take().iter().any(|event| matches!(
            event,
            ImageRenderEventDto::DetailAvailabilityChanged {
                available: false,
                ..
            }
        )));
        let signal = DisplayTickSignal::default();
        assert!(signal.bind_current_thread());
        *lock(&publisher.wake) = Some(signal.clone());
        let gpu_signal = signal.clone();
        actor.gpu_wake = Arc::new(move || gpu_signal.wake());
        actor.decode_worker.shutdown();
        actor.decode_results = Arc::default();
        actor.decode_worker = DecodeWorker::start(
            actor.provider.clone(),
            actor.decode_results.clone(),
            signal.clone(),
        )
        .unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let restored = Arc::new(AtomicBool::new(false));
        let failed_revision = actor.resource_revision;
        let observer = {
            let memory = memory.clone();
            let signal = signal.clone();
            let shutdown = shutdown.clone();
            let restored = restored.clone();
            let events = events.clone();
            thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut detail_restored = false;
                let mut terminal = false;
                while std::time::Instant::now() < deadline {
                    for event in events.take() {
                        detail_restored |= matches!(event, ImageRenderEventDto::DetailAvailabilityChanged { asset_generation: 1, resource_revision, available: true, .. } if resource_revision > failed_revision);
                        terminal |= matches!(event, ImageRenderEventDto::Failed { .. });
                    }
                    if memory
                        .snapshot()
                        .bytes_for_class(AllocationClass::GpuTexture)
                        >= (3 << 20) + 4
                        && memory
                            .snapshot()
                            .bytes_for_class(AllocationClass::DecodedPixels)
                            == 0
                        && detail_restored
                    {
                        restored.store(true, Ordering::Release);
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                shutdown.store(true, Ordering::Release);
                signal.wake();
                terminal
            })
        };
        sink(PressureLevel::Normal);
        let (_sender, receiver) = mpsc::channel();
        // No display ticks, input messages or test-side GPU polling. The actual
        // actor run loop is woken only by pressure, decode and completion work.
        actor.run(&mut renderer, receiver, signal, shutdown);
        assert!(
            !observer.join().unwrap(),
            "pressure/restoration must not terminate the native session"
        );
        assert!(
            restored.load(Ordering::Acquire),
            "Normal must restore exact latest demand without display/input"
        );
        assert!(renderer.resource_key_is_ready(visible));
        assert_eq!(actor.camera, camera);
        assert_eq!(actor.scene, scene);
        assert_eq!(actor.editor_anchor, editor_anchor);
        assert_eq!(
            memory
                .snapshot()
                .bytes_for_class(AllocationClass::NativeDecode),
            0
        );
        assert!(cache_path.exists());
    }

    fn test_actor_with_policy(
        policy: viewer_render_core::ImageMemoryPolicy,
    ) -> (
        tempfile::TempDir,
        RendererActor,
        WgpuImageRenderer,
        Arc<crate::image_render_events::RecordingImageRenderEvents>,
    ) {
        let cache = tempfile::tempdir().unwrap();
        let memory = viewer_render_core::ImageMemoryCoordinator::new(policy);
        let layout = SurfaceLayout {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 100.0,
            scale_factor: 1.0,
        };
        let mut renderer = WgpuImageRenderer::new(
            RendererDescriptor::headless(
                LogicalSize::new(100.0, 100.0).unwrap(),
                PhysicalSize {
                    width: 100,
                    height: 100,
                },
                1.0,
            )
            .unwrap()
            .with_memory(memory.clone()),
        )
        .unwrap();
        let provider = MacImageResourceProvider::with_memory(
            cache.path(),
            MemoryBudget::baseline_8gb(),
            memory,
        )
        .unwrap();
        let events = Arc::new(crate::image_render_events::RecordingImageRenderEvents::default());
        let mut actor = RendererActor::new(
            &mut renderer,
            layout,
            provider,
            events.clone(),
            DisplayTickSignal::default(),
            Arc::default(),
            Arc::default(),
        )
        .unwrap();
        actor.decode_worker.shutdown();
        actor.decode_results = Arc::default();
        actor.decode_worker = DecodeWorker {
            jobs: Arc::new((Mutex::default(), Condvar::new())),
            results: actor.decode_results.clone(),
            join: None,
        };
        (cache, actor, renderer, events)
    }

    fn drain_uploads(actor: &mut RendererActor, renderer: &mut WgpuImageRenderer) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while actor.pending_upload.is_some() || renderer.has_pending_upload() {
            assert!(
                std::time::Instant::now() < deadline,
                "bounded test upload did not finish"
            );
            renderer.poll_gpu_timings().unwrap();
            actor.progress_upload(renderer);
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn actor_delayed_preview_and_early_manifest_do_not_publish_partial_content() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        let session_id = viewer_render_core::RenderSessionId(1);
        let generation = AssetGeneration(1);
        actor.session_id = Some(session_id);
        actor.generation = generation;
        let (mut probe, mut resource) = actor
            .provider
            .probe_and_request_preview(&source, generation, PreviewRequest::new(32, 32).unwrap())
            .unwrap();
        probe.width = 1025;
        probe.height = 1025;
        probe.orientation = 1;
        resource.kind = DecodedResourceKind::Preview { level: 2 };
        resource.width = 1025;
        resource.height = 1025;
        resource.bytes_per_row = 4100;
        resource.pixels = viewer_render_core::SharedPixels::try_zeroed(
            actor.provider.memory(),
            generation,
            1025 * 1025 * 4,
        )
        .unwrap();
        actor.install_resource(
            &mut renderer,
            session_id,
            generation,
            probe,
            resource.clone(),
        );
        let preview_ready = renderer.resource_key_is_ready(ResourceKey::WholeImage { level: 2 });
        assert_eq!(actor.content_frames.can_publish(generation), preview_ready);
        let ready_before = events
            .take()
            .iter()
            .filter(|event| matches!(event, ImageRenderEventDto::Ready { .. }))
            .count();
        assert_eq!(ready_before, usize::from(preview_ready));
        if !preview_ready {
            assert!(
                renderer
                    .retained_scene_resources()
                    .image_handles()
                    .is_empty()
            );
        }
        drain_uploads(&mut actor, &mut renderer);
        assert!(actor.content_frames.can_publish(generation));
        assert_eq!(
            events
                .take()
                .iter()
                .filter(|event| matches!(event, ImageRenderEventDto::Ready { .. }))
                .count()
                + ready_before,
            1
        );
        let source_size = actor.source_size.unwrap();
        let old_key = ResourceKey::WholeImage { level: 1 };
        let old = renderer
            .upload_resource(DecodedResource::WholeImage {
                generation,
                level: 1,
                source_size,
                width: 1,
                height: 1,
                pixels: viewer_render_core::SharedPixels::try_zeroed(
                    actor.provider.memory(),
                    generation,
                    4,
                )
                .unwrap(),
            })
            .unwrap();
        drain_uploads(&mut actor, &mut renderer);
        assert!(renderer.resource_is_ready(old));
        actor.refinement_keys = vec![old_key];
        resource.kind = DecodedResourceKind::Preview { level: 0 };
        // Deterministically deliver the decoder manifest before starting GPU
        // upload; queue.submit may otherwise finish earlier bands immediately.
        actor.pending_upload = Some(PendingActorUpload {
            resource: gpu_resource(resource, source_size).unwrap(),
            handle: None,
            preview: None,
            admission_snapshot: None,
        });
        actor.publish_detail_availability(false);
        events.take();
        assert!(actor.pending_upload.is_some());
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::RefinementCompleted {
                session_id,
                generation,
                revision: actor.resource_revision,
                keys: vec![ResourceKey::WholeImage { level: 0 }],
            },
        );
        assert_eq!(
            actor.refinement_keys,
            vec![old_key],
            "decoder manifest is not GPU replacement readiness"
        );
        assert!(renderer.resource_key_is_ready(old_key));
        assert!(!events.take().iter().any(|event| matches!(
            event,
            ImageRenderEventDto::DetailAvailabilityChanged {
                available: true,
                ..
            }
        )));
        drain_uploads(&mut actor, &mut renderer);
        assert!(events.take().iter().any(|event| matches!(
            event,
            ImageRenderEventDto::DetailAvailabilityChanged {
                available: true,
                ..
            }
        )));
        assert_eq!(
            actor.refinement_keys,
            vec![ResourceKey::WholeImage { level: 0 }]
        );
        assert!(!renderer.resource_key_is_ready(old_key));
        assert!(renderer.resource_key_is_ready(ResourceKey::WholeImage { level: 2 }));
    }

    #[test]
    fn actor_capacity_retry_retires_obsolete_detail_and_preserves_preview() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let limits = viewer_render_core::LiveMemoryLimits {
            combined_bytes: 64 << 20,
            gpu_bytes: 24 << 20,
        };
        let (_cache, mut actor, mut renderer, events) = test_actor_with_policy(
            viewer_render_core::ImageMemoryPolicy::new(limits, limits, limits).unwrap(),
        );
        let generation = AssetGeneration(1);
        actor.generation = generation;
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        let source_size = SourceSize::new(1024, 768).unwrap();
        actor.source_size = Some(source_size);
        actor.source = Some(
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap(),
        );
        actor.viewport =
            ViewportLayout::new(LogicalSize::new(1024.0, 768.0).unwrap(), 1.0, 1.0).unwrap();
        actor.transform = Some(
            TransformSnapshot::new(
                source_size,
                actor.viewport,
                CameraState::fit(Rotation::Deg0),
            )
            .unwrap(),
        );
        let memory = actor.provider.memory().clone();
        let old_key = ResourceKey::WholeImage { level: 1 };
        renderer
            .upload_resource(DecodedResource::WholeImage {
                generation,
                level: 1,
                source_size,
                width: 2560,
                height: 2048,
                pixels: viewer_render_core::SharedPixels::try_zeroed(&memory, generation, 20 << 20)
                    .unwrap(),
            })
            .unwrap();
        drain_uploads(&mut actor, &mut renderer);
        renderer
            .upload_resource(DecodedResource::WholeImage {
                generation,
                level: 2,
                source_size,
                width: 1,
                height: 1,
                pixels: viewer_render_core::SharedPixels::try_zeroed(&memory, generation, 4)
                    .unwrap(),
            })
            .unwrap();
        drain_uploads(&mut actor, &mut renderer);
        actor.preview_key = Some(ResourceKey::WholeImage { level: 2 });
        actor.refinement_keys = vec![old_key];
        actor.resource_plans = vec![ResourcePlan {
            strategy: TextureStrategy::SingleTexture,
            level: 0,
            required_tiles: vec![viewer_render_core::TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }],
            prefetch_tiles: vec![],
        }];
        actor.pending_upload = Some(PendingActorUpload {
            resource: DecodedResource::WholeImage {
                generation,
                level: 0,
                source_size,
                width: 1024,
                height: 768,
                pixels: viewer_render_core::SharedPixels::try_zeroed(&memory, generation, 3 << 20)
                    .unwrap(),
            },
            handle: None,
            preview: None,
            admission_snapshot: None,
        });
        actor.progress_upload(&mut renderer);
        assert!(
            actor.pending_upload.is_some(),
            "admission must wait without blocking actor"
        );
        assert!(!renderer.resource_key_is_ready(old_key));
        assert!(renderer.resource_key_is_ready(actor.preview_key.unwrap()));
        assert!(
            memory
                .snapshot()
                .bytes_for_phase(viewer_render_core::AllocationPhase::Retiring)
                >= 3 << 20
        );
        drain_uploads(&mut actor, &mut renderer);
        assert!(renderer.resource_key_is_ready(ResourceKey::WholeImage { level: 0 }));
        assert!(renderer.resource_key_is_ready(actor.preview_key.unwrap()));
        let resumed = lock(&actor.decode_worker.jobs.0).latest.take().unwrap();
        let (revision, resumed_source, plans) = resumed.refinement.unwrap();
        assert!(revision > 0);
        assert_eq!(resumed_source, source_size);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].strategy, TextureStrategy::SingleTexture);
        assert_eq!(plans[0].level, 0);
        assert_eq!(
            plans[0].required_tiles,
            vec![viewer_render_core::TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }]
        );
        assert!(memory.snapshot().peak_gpu_bytes <= 24 << 20);
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::Failed { .. }))
        );
    }

    #[test]
    fn actor_superseding_blocked_plan_cancels_the_new_worker_epoch() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none()
            || std::env::var_os("VIEWER_RUN_LARGE_IMAGE_TESTS").is_none()
        {
            return;
        }
        use viewer_render_core::{AllocationClass, AllocationPhase, SharedPixels, TileCoordinate};
        let limits = viewer_render_core::LiveMemoryLimits {
            combined_bytes: 512 << 20,
            gpu_bytes: 16 << 20,
        };
        let (cache, mut actor, mut renderer, events) = test_actor_with_policy(
            viewer_render_core::ImageMemoryPolicy::new(limits, limits, limits).unwrap(),
        );
        let fixture = cache.path().join("supersede-4096.jpg");
        assert!(
            std::process::Command::new("/usr/bin/sips")
                .args(["--resampleHeightWidth", "4096", "4096"])
                .arg(viewer_test_support::image_fixtures::image_fixture(
                    "srgb.jpg"
                ))
                .arg("--out")
                .arg(&fixture)
                .output()
                .unwrap()
                .status
                .success()
        );
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                &fixture,
            )
            .unwrap();
        let generation = AssetGeneration(1);
        let session_id = viewer_render_core::RenderSessionId(1);
        let source_size = SourceSize::new(4096, 4096).unwrap();
        let memory = actor.provider.memory().clone();
        actor.session_id = Some(session_id);
        actor.generation = generation;
        actor.source_size = Some(source_size);
        actor.source = Some(source.clone());
        let obsolete: Vec<_> = (0..3)
            .flat_map(|y| (0..4).map(move |x| TileCoordinate { level: 1, x, y }))
            .take(11)
            .collect();
        let reusable = TileCoordinate {
            level: 0,
            x: 3,
            y: 3,
        };
        for tile in obsolete.iter().copied().chain([reusable]) {
            renderer
                .upload_resource(DecodedResource::Tile {
                    generation,
                    tile,
                    tile_size: 512,
                    sample_border: 0,
                    source_size,
                    width: 512,
                    height: 512,
                    pixels: SharedPixels::try_zeroed(&memory, generation, 1 << 20).unwrap(),
                })
                .unwrap();
            drain_uploads(&mut actor, &mut renderer);
        }
        let preview_key = ResourceKey::WholeImage { level: 3 };
        renderer
            .upload_resource(DecodedResource::WholeImage {
                generation,
                level: 3,
                source_size,
                width: 1,
                height: 1,
                pixels: SharedPixels::try_zeroed(&memory, generation, 4).unwrap(),
            })
            .unwrap();
        drain_uploads(&mut actor, &mut renderer);
        actor.preview_key = Some(preview_key);
        actor.refinement_keys = obsolete.iter().copied().map(ResourceKey::Tile).collect();
        let waiting = TileCoordinate {
            level: 0,
            x: 4,
            y: 3,
        };
        let a_plan = ResourcePlan {
            strategy: TextureStrategy::Tiled { tile_size: 512 },
            level: 0,
            required_tiles: vec![waiting],
            prefetch_tiles: vec![],
        };
        actor.resource_plans = vec![
            ResourcePlan {
                strategy: TextureStrategy::Tiled { tile_size: 512 },
                level: 1,
                required_tiles: obsolete.clone(),
                prefetch_tiles: vec![],
            },
            ResourcePlan {
                required_tiles: vec![reusable, waiting],
                ..a_plan.clone()
            },
        ];
        actor.resource_revision = 1;
        actor.decode_worker.shutdown();
        actor.decode_results = Arc::default();
        actor.decode_worker = DecodeWorker::start(
            actor.provider.clone(),
            actor.decode_results.clone(),
            DisplayTickSignal::default(),
        )
        .unwrap();
        actor
            .decode_worker
            .submit(DecodeJob {
                session_id,
                generation,
                source,
                physical_width: 4096,
                physical_height: 4096,
                refinement: Some((1, source_size, vec![a_plan])),
                retry_demand: DecodeDemandSnapshot::max(),
            })
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(result) = actor.decode_results.take() {
                assert!(matches!(result, DecodeResult::Refined { revision: 1, .. }));
                actor.handle_decode_result(&mut renderer, result);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(actor.pending_upload.is_some());
        assert!(
            obsolete
                .iter()
                .all(|tile| renderer.resource_key_is_ready(ResourceKey::Tile(*tile)))
        );
        while memory
            .snapshot()
            .bytes_for_class(AllocationClass::NativeDecode)
            != 0
        {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }

        // Actual camera scheduling supersedes A and starts B's fresh worker epoch.
        actor.viewport =
            ViewportLayout::new(LogicalSize::new(1024.0, 1024.0).unwrap(), 1.0, 1.0).unwrap();
        actor.handle_command(
            &mut renderer,
            AuthorizedImageRenderCommand::Camera {
                camera: CameraState {
                    mode: CameraMode::Free,
                    zoom: 4.0,
                    ..CameraState::fit(Rotation::Deg0)
                },
            },
        );
        let b_revision = actor.resource_revision;
        let b_plans = actor.resource_plans.clone();
        let b_epoch = lock(&actor.decode_worker.jobs.0).epoch;
        assert!(b_revision > 1);
        assert_eq!(b_plans[0].required_tiles.len(), 4);
        // Pause this actual consumer until B is holding a native mip behind
        // the full FIFO. No timing/cancellation/worker behavior is mocked.
        while actor.decode_results.queued_bytes() < handoff::HANDOFF_BYTES {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(
            memory
                .snapshot()
                .bytes_for_class(AllocationClass::NativeDecode)
                > 0
        );
        while actor.pending_upload.is_none() {
            let result = actor
                .decode_results
                .take()
                .expect("B FIFO has decoded tiles");
            assert!(
                matches!(result, DecodeResult::Refined { revision, .. } if revision == b_revision)
            );
            actor.handle_decode_result(&mut renderer, result);
        }
        assert!(
            lock(&actor.decode_worker.jobs.0).epoch > b_epoch,
            "B's first blocked upload must cancel B, not reuse A's suspension"
        );
        assert_eq!(actor.decode_results.queued_bytes(), 0);
        while memory
            .snapshot()
            .bytes_for_class(AllocationClass::NativeDecode)
            != 0
        {
            assert!(
                std::time::Instant::now() < deadline,
                "B's cancelled producer kept native backing"
            );
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            memory
                .snapshot()
                .bytes_for_class(AllocationClass::DecodedPixels),
            1 << 20
        );
        assert!(memory.snapshot().bytes_for_phase(AllocationPhase::Retiring) >= 11 << 20);
        assert!(
            obsolete
                .iter()
                .all(|tile| !renderer.resource_key_is_ready(ResourceKey::Tile(*tile)))
        );
        assert!(renderer.resource_key_is_ready(preview_key));
        assert!(renderer.resource_key_is_ready(ResourceKey::Tile(reusable)));
        let b_pending_key = actor.pending_upload.as_ref().unwrap().resource.key();
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::RefinementCompleted {
                session_id,
                generation,
                revision: 1,
                keys: vec![ResourceKey::Tile(waiting)],
            },
        );
        assert!(
            actor.pending_manifest.is_none(),
            "old A completion cannot publish into B"
        );
        drain_uploads(&mut actor, &mut renderer);
        assert!(renderer.resource_key_is_ready(b_pending_key));
        assert_eq!(actor.resource_plans, b_plans);
        assert!(
            actor.resource_revision > b_revision,
            "completed B upload must resume B's exact demand"
        );
        let resumed_revision = actor.resource_revision;
        loop {
            if let Some(result) = actor.decode_results.take() {
                assert!(
                    matches!(result, DecodeResult::Refined { revision, .. } if revision == resumed_revision)
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        actor.decode_worker.shutdown();
        assert_eq!(
            memory
                .snapshot()
                .bytes_for_class(AllocationClass::NativeDecode),
            0
        );
        assert!(memory.snapshot().peak_gpu_bytes <= 16 << 20);
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::Failed { .. }))
        );
    }

    #[test]
    fn actor_unplannable_supersession_does_not_reinstall_the_cancelled_upload() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source = Some(
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap(),
        );
        let source_size = SourceSize::new(100_000, 100_000).unwrap();
        actor.source_size = Some(source_size);
        actor.viewport =
            ViewportLayout::new(LogicalSize::new(100_000.0, 100_000.0).unwrap(), 1.0, 1.0).unwrap();
        let cancelled_key = ResourceKey::WholeImage { level: 8 };
        actor.pending_upload = Some(PendingActorUpload {
            resource: DecodedResource::WholeImage {
                generation: actor.generation,
                level: 8,
                source_size,
                width: 1,
                height: 1,
                pixels: viewer_render_core::SharedPixels::try_zeroed(
                    actor.provider.memory(),
                    actor.generation,
                    4,
                )
                .unwrap(),
            },
            handle: None,
            preview: None,
            admission_snapshot: None,
        });
        actor.handle_command(
            &mut renderer,
            AuthorizedImageRenderCommand::Camera {
                camera: CameraState::fit(Rotation::Deg0),
            },
        );
        assert!(
            events.take().iter().any(|event| matches!(
                event,
                ImageRenderEventDto::DetailAvailabilityChanged {
                    available: false,
                    ..
                }
            )),
            "the new full-resolution demand is outside policy"
        );
        actor.progress_upload(&mut renderer);
        assert!(
            !renderer.resource_key_is_ready(cancelled_key),
            "cancelled old demand cannot be installed after a failed supersession"
        );
        assert!(actor.pending_upload.is_none());
        assert!(lock(&actor.decode_worker.jobs.0).latest.is_none());
    }

    #[test]
    fn actor_decoder_admission_waits_for_gpu_retirement_then_retries_exact_demand() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source = Some(
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap(),
        );
        actor.source_size = Some(SourceSize::new(1024, 768).unwrap());
        actor.update_transform(&mut renderer);
        lock(&actor.decode_worker.jobs.0).latest.take();
        let plans = actor.resource_plans.clone();
        renderer
            .upload_resource(DecodedResource::WholeImage {
                generation: actor.generation,
                level: 2,
                source_size: actor.source_size.unwrap(),
                width: 1,
                height: 1,
                pixels: viewer_render_core::SharedPixels::try_zeroed(
                    actor.provider.memory(),
                    actor.generation,
                    4,
                )
                .unwrap(),
            })
            .unwrap();
        renderer.retain_resources(&[]);
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::DecodeFailed {
                session_id: actor.session_id.unwrap(),
                generation: actor.generation,
                revision: Some(actor.resource_revision),
                cancelled: false,
                memory: Some(viewer_render_core::MemoryAdmissionError::TemporarilyBlocked),
                retry_demand: None,
            },
        );
        actor.progress_upload(&mut renderer);
        assert!(lock(&actor.decode_worker.jobs.0).latest.is_none());
        assert!(actor.retry_decode.is_some());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while lock(&actor.decode_worker.jobs.0).latest.is_none() {
            assert!(std::time::Instant::now() < deadline);
            renderer.poll_gpu_timings().unwrap();
            actor.progress_upload(&mut renderer);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(actor.resource_plans, plans);
        assert!(actor.retry_decode.is_none());
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::Failed { .. }))
        );
    }

    #[test]
    fn actor_failed_demand_accepts_an_explicit_same_plan_retry() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source = Some(
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap(),
        );
        actor.source_size = Some(SourceSize::new(1024, 768).unwrap());
        actor.update_transform(&mut renderer);
        lock(&actor.decode_worker.jobs.0).latest.take();
        let plans = actor.resource_plans.clone();
        let revision = actor.resource_revision;
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::DecodeFailed {
                session_id: actor.session_id.unwrap(),
                generation: actor.generation,
                revision: Some(revision),
                cancelled: false,
                memory: Some(viewer_render_core::MemoryAdmissionError::TemporarilyBlocked),
                retry_demand: None,
            },
        );
        actor.handle_command(
            &mut renderer,
            AuthorizedImageRenderCommand::Camera {
                camera: actor.camera,
            },
        );
        assert_eq!(actor.resource_plans, plans);
        assert!(actor.resource_revision > revision);
        assert!(lock(&actor.decode_worker.jobs.0).latest.is_some());
    }

    #[test]
    fn actor_retry_demand_follows_submission_snapshot_even_after_release() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source = Some(
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap(),
        );
        actor.source_size = Some(SourceSize::new(1024, 768).unwrap());
        actor.update_transform(&mut renderer);
        lock(&actor.decode_worker.jobs.0).latest.take();
        let revision = actor.resource_revision;
        let plans = actor.resource_plans.clone();
        let snapshot = renderer.memory().snapshot();
        let stale_retry_demand = DecodeDemandSnapshot {
            combined_bytes: snapshot.combined_bytes.saturating_add(128),
            gpu_bytes: snapshot.gpu_bytes.saturating_add(128),
            retiring_bytes: 1,
            pressure_epoch: snapshot.pressure_epoch,
        };
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::DecodeFailed {
                session_id: actor.session_id.unwrap(),
                generation: actor.generation,
                revision: Some(revision),
                cancelled: false,
                memory: Some(viewer_render_core::MemoryAdmissionError::TemporarilyBlocked),
                retry_demand: Some(stale_retry_demand),
            },
        );
        assert!(actor.retry_decode.is_some());
        // The submission-time demand already exceeds current usage: release
        // happened before the failure reached the actor. No further GPU event
        // is needed, and exactly one fresh request must be submitted now.
        actor.progress_upload(&mut renderer);
        assert!(actor.retry_decode.is_none());
        assert_eq!(actor.resource_plans, plans);
        assert!(actor.resource_revision > revision);
        let job = lock(&actor.decode_worker.jobs.0).latest.take().unwrap();
        assert_eq!(job.refinement.unwrap().0, actor.resource_revision);
        actor.progress_upload(&mut renderer);
        assert!(lock(&actor.decode_worker.jobs.0).latest.is_none());
    }

    #[test]
    fn pending_scene_projection_syncs_only_after_renderer_admits_it() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, renderer, _events) = test_actor();
        let rendered = SceneSnapshot::empty(SceneRevision(3));
        let pending = SceneSnapshot::empty(SceneRevision(4));
        actor.rendered_scene = rendered.clone();
        actor.pending_rendered_scene = Some(pending.clone());
        actor.sync_rendered_scene(&renderer);
        assert_eq!(actor.rendered_scene, pending);
        assert!(actor.pending_rendered_scene.is_none());
    }

    #[test]
    fn actor_annotation_creation_accepts_incomplete_pointer_samples() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        for mode in [
            InteractionMode::Rectangle,
            InteractionMode::Ellipse,
            InteractionMode::Arrow,
            InteractionMode::Brush,
            InteractionMode::Point,
        ] {
            let (_cache, mut actor, mut renderer, events) = test_actor();
            actor.session_id = Some(viewer_render_core::RenderSessionId(1));
            actor.generation = AssetGeneration(1);
            actor.source_size = Some(SourceSize::new(100, 100).unwrap());
            actor.update_transform(&mut renderer);
            actor.interaction.set_mode(mode);
            let request = renderer.on_display_tick(1).unwrap();
            let (_, empty) = renderer.render_headless_capture(request).unwrap();
            for (phase, x, y) in [
                (PointerPhase::Down, 30.0, 30.0),
                (PointerPhase::Move, 50.0, 30.0),
                (PointerPhase::Move, 60.0, 60.0),
                // Dragging back through the origin is another incomplete draft.
                (PointerPhase::Move, 30.0, 30.0),
                (PointerPhase::Up, 70.0, 70.0),
            ] {
                actor.handle_input(
                    &mut renderer,
                    NativeInput::Pointer(PointerSample {
                        phase,
                        location: LogicalPoint { x, y },
                        button: PointerButton::Primary,
                        pressure: 0.0,
                        modifiers: Modifiers::default(),
                        timestamp_ns: 1,
                    }),
                );
                actor.flush_continuous_events();
                let emitted = events.take();
                assert!(
                    !emitted
                        .iter()
                        .any(|event| matches!(event, ImageRenderEventDto::Failed { .. })),
                    "{mode:?} {phase:?} must not fail the renderer: {emitted:?}"
                );
                if phase == PointerPhase::Up {
                    assert!(
                        emitted.iter().any(|event| matches!(
                            event,
                            ImageRenderEventDto::DraftCompleted { .. }
                        )),
                        "{mode:?} must complete"
                    );
                }
                if let Some(request) = renderer.on_display_tick(1) {
                    let (_, captured) = renderer.render_headless_capture(request).unwrap();
                    if x == 60.0 && y == 60.0 {
                        assert!(captured != empty, "{mode:?} drawable draft must render");
                    }
                    if x == 30.0
                        && y == 30.0
                        && matches!(
                            mode,
                            InteractionMode::Rectangle
                                | InteractionMode::Ellipse
                                | InteractionMode::Arrow
                        )
                    {
                        assert!(
                            captured == empty,
                            "{mode:?} incomplete draft must clear its previous overlay"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn actor_delivers_gpu_completion_on_idle_ticks_without_submitting_more_frames() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        #[derive(Default)]
        struct TimingEvents {
            support: Mutex<Option<(u64, viewer_render_wgpu::GpuTimingSupport)>>,
            samples: Mutex<Vec<viewer_render_wgpu::GpuFrameTiming>>,
        }
        impl ImageRenderEventPort for TimingEvents {
            fn publish(&self, _event: ImageRenderEventDto) {}
            fn gpu_timing_support(&self, id: u64, support: viewer_render_wgpu::GpuTimingSupport) {
                *lock(&self.support) = Some((id, support));
            }
            fn gpu_frame_completed(&self, timing: viewer_render_wgpu::GpuFrameTiming) {
                lock(&self.samples).push(timing);
            }
        }
        let (_cache, mut actor, mut renderer, _) = test_actor();
        let support = renderer.gpu_timing_support();
        let renderer_id = renderer.renderer_id();
        let events = Arc::new(TimingEvents::default());
        actor.events = events.clone();
        let source_size = SourceSize::new(1, 1).unwrap();
        renderer
            .upload_resource(DecodedResource::WholeImage {
                generation: AssetGeneration(7),
                level: 0,
                source_size,
                width: 1,
                height: 1,
                pixels: viewer_render_core::SharedPixels::try_copy_from_slice(
                    &viewer_render_core::ImageMemoryCoordinator::new(
                        viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
                    ),
                    AssetGeneration(1),
                    &[0, 0, 255, 255],
                )
                .unwrap(),
            })
            .unwrap();
        renderer
            .set_transform(
                TransformSnapshot::new(source_size, actor.viewport, actor.camera).unwrap(),
            )
            .unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let signal = DisplayTickSignal::default();
        assert!(signal.bind_current_thread());
        let (_sender, receiver) = mpsc::channel();
        let feeder = {
            let shutdown = shutdown.clone();
            let signal = signal.clone();
            let events = events.clone();
            thread::spawn(move || {
                // The actor must drain an asynchronous query even after its
                // only dirty frame. No follow-up camera/scene command wakes it.
                for tick in 1..=120 {
                    signal.publish(tick * 16_666_667);
                    thread::sleep(Duration::from_millis(16));
                    if !lock(&events.samples).is_empty() {
                        break;
                    }
                }
                shutdown.store(true, Ordering::Release);
                signal.wake();
            })
        };
        actor.run(&mut renderer, receiver, signal, shutdown);
        feeder.join().unwrap();
        assert_eq!(*lock(&events.support), Some((renderer_id, support)));
        if support == viewer_render_wgpu::GpuTimingSupport::Available {
            let samples = lock(&events.samples);
            assert_eq!(samples.len(), 1, "only the original dirty frame may render");
            assert_eq!(samples[0].renderer_id, renderer_id);
            assert_eq!(samples[0].frame_index, 0);
            assert_eq!(samples[0].generation, AssetGeneration(7));
            assert!(samples[0].gpu_time_ns > 0);
        }
    }

    #[test]
    fn actor_camera_resize_and_magnifier_schedule_refinement() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        actor.open(
            &mut renderer,
            viewer_render_core::RenderSessionId(1),
            AssetGeneration(1),
            source.clone(),
        );
        lock(&actor.decode_worker.jobs.0).latest.take();
        let (probe, resource) = actor
            .provider
            .probe_and_request_preview(
                &source,
                AssetGeneration(1),
                PreviewRequest::new(100, 100).unwrap(),
            )
            .unwrap();
        actor.install_resource(
            &mut renderer,
            viewer_render_core::RenderSessionId(1),
            AssetGeneration(1),
            probe,
            resource,
        );
        lock(&actor.decode_worker.jobs.0).latest.take();
        actor.handle_command(
            &mut renderer,
            AuthorizedImageRenderCommand::Camera {
                camera: CameraState {
                    mode: CameraMode::Free,
                    zoom: 2.0,
                    rotation: Rotation::Deg0,
                    offset: LogicalPoint::ZERO,
                },
            },
        );
        assert!(
            lock(&actor.decode_worker.jobs.0).latest.take().is_some(),
            "camera zoom must request sharper resources"
        );
        actor.resize(
            &mut renderer,
            SurfaceLayout {
                left: 0.0,
                top: 0.0,
                width: 250.0,
                height: 250.0,
                scale_factor: 1.0,
            },
        );
        assert!(
            lock(&actor.decode_worker.jobs.0).latest.take().is_some(),
            "resize must reconsider physical resolution"
        );
        actor.handle_command(
            &mut renderer,
            AuthorizedImageRenderCommand::SetMagnifier {
                magnifier: Some(ImageRenderMagnifierPreferences {
                    width_px: 80.0,
                    height_px: 80.0,
                    magnification: 4.0,
                    shape: viewer_render_wgpu::MagnifierShape::RoundedRectangle,
                }),
            },
        );
        assert!(
            lock(&actor.decode_worker.jobs.0).latest.take().is_some(),
            "magnifier must request its source region"
        );
    }

    #[test]
    fn actor_occlusion_does_not_exhaust_surface_recovery() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        for tick in 1..=10 {
            actor.handle_render_error(
                &mut renderer,
                RenderError::Surface(viewer_render_wgpu::SurfaceAcquireFailure::Occluded),
            );
            assert!(
                renderer.on_display_tick(tick).is_some(),
                "occlusion must preserve retry progress"
            );
        }
        assert_eq!(actor.recovery.consecutive_failures(), 0);
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::Failed { .. }))
        );
    }

    #[test]
    fn actor_empty_refinement_keeps_coverage_without_false_failure() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source_size = Some(SourceSize::new(10, 10).unwrap());
        actor.resource_revision = 2;
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::RefinementCompleted {
                session_id: viewer_render_core::RenderSessionId(1),
                generation: AssetGeneration(1),
                revision: 2,
                keys: vec![],
            },
        );
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::Failed { .. })),
            "empty off-image request is not a decode failure"
        );
    }

    #[test]
    fn actor_retryable_failure_schedules_next_tick_then_escalates_once() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        renderer.on_display_tick(1);
        actor.handle_render_error(
            &mut renderer,
            RenderError::Surface(viewer_render_wgpu::SurfaceAcquireFailure::Timeout),
        );
        assert!(renderer.on_display_tick(2).is_some());
        actor.handle_render_error(
            &mut renderer,
            RenderError::Surface(viewer_render_wgpu::SurfaceAcquireFailure::Timeout),
        );
        assert!(renderer.on_display_tick(3).is_none());
        let emitted = events.take();
        assert_eq!(
            emitted
                .iter()
                .filter(|event| matches!(event, ImageRenderEventDto::Recovering { .. }))
                .count(),
            1
        );
        assert_eq!(
            emitted
                .iter()
                .filter(|event| matches!(
                    event,
                    ImageRenderEventDto::Failed {
                        retryable: true,
                        ..
                    }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn actor_initial_preview_is_bounded_even_for_huge_viewports() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        actor.viewport = ViewportLayout::new(
            LogicalSize::new(10_000.0, 10_000.0).unwrap(),
            2.0,
            FIT_INSET,
        )
        .unwrap();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        actor.open(
            &mut renderer,
            viewer_render_core::RenderSessionId(1),
            AssetGeneration(1),
            source,
        );
        let job = lock(&actor.decode_worker.jobs.0).latest.take().unwrap();
        assert!(
            u64::from(job.physical_width) * u64::from(job.physical_height) * 4 <= 16 * 1024 * 1024
        );
    }

    #[test]
    fn actor_panning_completely_off_image_cancels_refinement_without_empty_decode() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        actor.open(
            &mut renderer,
            viewer_render_core::RenderSessionId(1),
            AssetGeneration(1),
            source.clone(),
        );
        let (probe, preview) = actor
            .provider
            .probe_and_request_preview(
                &source,
                AssetGeneration(1),
                PreviewRequest::new(100, 100).unwrap(),
            )
            .unwrap();
        actor.install_resource(
            &mut renderer,
            viewer_render_core::RenderSessionId(1),
            AssetGeneration(1),
            probe,
            preview,
        );
        assert!(lock(&actor.decode_worker.jobs.0).latest.is_some());
        actor.handle_command(
            &mut renderer,
            AuthorizedImageRenderCommand::Camera {
                camera: CameraState {
                    mode: CameraMode::Free,
                    zoom: 1.0,
                    rotation: Rotation::Deg0,
                    offset: LogicalPoint {
                        x: 10_000.0,
                        y: 10_000.0,
                    },
                },
            },
        );
        assert!(
            lock(&actor.decode_worker.jobs.0).latest.is_none(),
            "off-image views have no decode demand"
        );
    }

    #[test]
    fn actor_refinement_retains_preview_and_discards_stale_generation_and_revision() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        let session_id = viewer_render_core::RenderSessionId(1);
        let generation = AssetGeneration(1);
        actor.open(&mut renderer, session_id, generation, source.clone());
        let (probe, preview) = actor
            .provider
            .probe_and_request_preview(&source, generation, PreviewRequest::new(100, 100).unwrap())
            .unwrap();
        actor.install_resource(&mut renderer, session_id, generation, probe, preview);
        let preview_bytes = renderer.gpu_resource_bytes();
        let plan = ResourcePlan {
            strategy: TextureStrategy::Tiled { tile_size: 512 },
            level: 0,
            required_tiles: vec![viewer_render_core::TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }],
            prefetch_tiles: vec![],
        };
        let resource = actor
            .provider
            .request_tiles(&source, generation, &plan)
            .unwrap()
            .remove(0);
        let revision = actor.resource_revision;
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::Refined {
                session_id,
                generation: AssetGeneration(0),
                revision,
                resource: Box::new(resource.clone()),
            },
        );
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::Refined {
                session_id,
                generation,
                revision: revision - 1,
                resource: Box::new(resource.clone()),
            },
        );
        assert_eq!(renderer.gpu_resource_bytes(), preview_bytes);
        for x in 0..10 {
            // Different visible regions replace the retained tile set. Their
            // memory cannot accumulate as the camera moves across an image.
            let mut next = resource.clone();
            next.kind = DecodedResourceKind::Tile(viewer_render_core::TileCoordinate {
                level: 0,
                x: x % 2,
                y: 0,
            });
            actor.handle_decode_result(
                &mut renderer,
                DecodeResult::Refined {
                    session_id,
                    generation,
                    revision,
                    resource: Box::new(next.clone()),
                },
            );
            actor.handle_decode_result(
                &mut renderer,
                DecodeResult::RefinementCompleted {
                    session_id,
                    generation,
                    revision,
                    keys: vec![resource_key(&next)],
                },
            );
            drain_uploads(&mut actor, &mut renderer);
            assert_eq!(renderer.retained_scene_resources().image_handles().len(), 2);
            // The first tile has a one-pixel sample gutter on its right and
            // bottom edges. Retaining one tile must include those real bytes.
            assert_eq!(renderer.gpu_resource_bytes(), preview_bytes + 513 * 513 * 4);
        }
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::Failed { .. }))
        );
    }

    #[test]
    fn actor_large_source_uses_bounded_main_and_magnifier_tiles() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source = Some(
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap(),
        );
        actor.source_size = Some(SourceSize::new(20_000, 15_000).unwrap());
        actor.camera = CameraState {
            mode: CameraMode::Free,
            zoom: 8.0,
            rotation: Rotation::Deg90,
            offset: LogicalPoint::ZERO,
        };
        actor.magnifier_preferences = Some(ImageRenderMagnifierPreferences {
            width_px: 80.0,
            height_px: 80.0,
            magnification: 4.0,
            shape: viewer_render_wgpu::MagnifierShape::RoundedRectangle,
        });
        actor.update_transform(&mut renderer);
        let job = lock(&actor.decode_worker.jobs.0).latest.take().unwrap();
        let (_, _, plans) = job.refinement.unwrap();
        assert_eq!(plans.len(), 2);
        assert!(
            plans
                .iter()
                .all(|plan| matches!(plan.strategy, TextureStrategy::Tiled { tile_size: 512 }))
        );
        assert!(plans[1].level < plans[0].level);
        assert!(
            plans
                .iter()
                .map(|plan| plan.required_tiles.len() + plan.prefetch_tiles.len())
                .sum::<usize>()
                <= 48
        );
    }

    #[test]
    fn actor_eight_k_physical_viewport_keeps_requested_level_when_budget_fits() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source = Some(
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap(),
        );
        actor.source_size = Some(SourceSize::new(7680, 4320).unwrap());
        actor.viewport =
            ViewportLayout::new(LogicalSize::new(3840.0, 2160.0).unwrap(), 2.0, FIT_INSET).unwrap();
        actor.magnifier_pointer = Some(LogicalPoint {
            x: 1920.0,
            y: 1080.0,
        });
        actor.magnifier_preferences = Some(ImageRenderMagnifierPreferences {
            width_px: 200.0,
            height_px: 200.0,
            magnification: 4.0,
            shape: viewer_render_wgpu::MagnifierShape::RoundedRectangle,
        });
        assert!(actor.update_transform(&mut renderer));
        let job = lock(&actor.decode_worker.jobs.0).latest.take().unwrap();
        let (_, _, plans) = job.refinement.unwrap();
        assert!(
            plans.iter().all(|plan| plan.level == 0),
            "a pixel-matched 8K image fits the resource budget without degrading LOD"
        );
        assert_eq!(
            plans.len(),
            1,
            "overlapping same-level lens requests share tiles"
        );
        assert_eq!(plans[0].required_tiles.len(), 135);
    }

    #[test]
    fn background_worker_returns_planned_tiles_and_cancels_superseded_work() {
        let cache = tempfile::tempdir().unwrap();
        let provider =
            MacImageResourceProvider::new(cache.path(), MemoryBudget::baseline_8gb()).unwrap();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        let probe = provider.probe(&source).unwrap();
        let source_size = SourceSize::new(probe.width, probe.height).unwrap();
        let plan = ResourcePlan {
            strategy: TextureStrategy::Tiled { tile_size: 512 },
            level: 0,
            required_tiles: vec![viewer_render_core::TileCoordinate {
                level: 0,
                x: 0,
                y: 0,
            }],
            prefetch_tiles: vec![],
        };
        let job = DecodeJob {
            session_id: viewer_render_core::RenderSessionId(1),
            generation: AssetGeneration(1),
            source,
            physical_width: 100,
            physical_height: 100,
            refinement: Some((7, source_size, vec![plan.clone()])),
            retry_demand: DecodeDemandSnapshot::max(),
        };
        assert!(matches!(
            decode_refinement(
                &provider,
                &job,
                source_size,
                &[plan],
                &|| true,
                &mut |_| Ok(())
            ),
            Err(viewer_platform_macos::image_render::ImageResourceError::Cancelled)
        ));
        let results = Arc::new(DecodeResultSlot::default());
        let mut worker =
            DecodeWorker::start(provider, results.clone(), DisplayTickSignal::default()).unwrap();
        worker.submit(job).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(result) = results.take() {
                break result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "background decode did not finish"
            );
            thread::sleep(Duration::from_millis(1));
        };
        worker.shutdown();
        match result {
            DecodeResult::Refined {
                revision, resource, ..
            } => {
                assert_eq!(revision, 7);
                assert!(matches!(resource.kind, DecodedResourceKind::Tile(_)));
                assert_eq!((resource.width, resource.height), (513, 513));
            }
            _ => panic!("expected decoded tile refinement"),
        }
    }

    #[test]
    fn actor_stale_result_releases_actual_pixel_storage_without_upload() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                viewer_test_support::image_fixtures::image_fixture("srgb.jpg"),
            )
            .unwrap();
        let resource = actor
            .provider
            .request_preview(
                &source,
                AssetGeneration(1),
                PreviewRequest::new(32, 32).unwrap(),
            )
            .unwrap();
        let memory = actor.provider.memory().clone();
        assert_eq!(
            memory
                .snapshot()
                .bytes_for_class(viewer_render_core::AllocationClass::DecodedPixels),
            3072
        );
        actor.handle_decode_result(
            &mut renderer,
            DecodeResult::Refined {
                session_id: viewer_render_core::RenderSessionId(99),
                generation: AssetGeneration(1),
                revision: 1,
                resource: Box::new(resource),
            },
        );
        assert_eq!(
            memory
                .snapshot()
                .bytes_for_class(viewer_render_core::AllocationClass::DecodedPixels),
            0
        );
        assert_eq!(renderer.gpu_resource_bytes(), 0);
    }

    #[test]
    fn paused_actual_worker_shutdown_releases_native_backing_and_queued_pixels() {
        if std::env::var_os("VIEWER_RUN_LARGE_IMAGE_TESTS").is_none() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let fixture = root.path().join("shutdown.jpg");
        assert!(
            std::process::Command::new("/usr/bin/sips")
                .args(["--resampleHeightWidth", "1024", "4096"])
                .arg(viewer_test_support::image_fixtures::image_fixture(
                    "srgb.jpg"
                ))
                .arg("--out")
                .arg(&fixture)
                .output()
                .unwrap()
                .status
                .success()
        );
        let provider =
            MacImageResourceProvider::new(&root.path().join("cache"), MemoryBudget::baseline_8gb())
                .unwrap();
        let memory = provider.memory().clone();
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                &fixture,
            )
            .unwrap();
        let results = Arc::new(DecodeResultSlot::default());
        let mut worker =
            DecodeWorker::start(provider, results.clone(), DisplayTickSignal::default()).unwrap();
        let plan = ResourcePlan {
            strategy: TextureStrategy::Tiled { tile_size: 512 },
            level: 0,
            required_tiles: (0..2)
                .flat_map(|y| {
                    (0..8).map(move |x| viewer_render_core::TileCoordinate { level: 0, x, y })
                })
                .collect(),
            prefetch_tiles: vec![],
        };
        worker
            .submit(DecodeJob {
                session_id: viewer_render_core::RenderSessionId(1),
                generation: AssetGeneration(1),
                source,
                physical_width: 4096,
                physical_height: 1024,
                refinement: Some((1, SourceSize::new(4096, 1024).unwrap(), vec![plan])),
                retry_demand: DecodeDemandSnapshot::max(),
            })
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while results.queued_bytes() < handoff::HANDOFF_BYTES {
            assert!(
                std::time::Instant::now() < deadline,
                "worker never reached backpressure"
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert!(
            memory
                .snapshot()
                .bytes_for_class(viewer_render_core::AllocationClass::NativeDecode)
                > 0
        );
        let start = std::time::Instant::now();
        worker.shutdown();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "shutdown must wake the blocked producer"
        );
        assert_eq!(memory.snapshot().combined_bytes, 0);
        assert!(results.take().is_none());
    }

    #[test]
    fn actor_geometry_cancel_restores_original_after_staged_scene_updates() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source_size = Some(SourceSize::new(100, 100).unwrap());
        actor.update_transform(&mut renderer);
        let id = AnnotationId::new("editable").unwrap();
        let original = AnnotationGeometry::Point {
            position: NormalizedPoint { x: 0.2, y: 0.2 },
        };
        let changed = AnnotationGeometry::Point {
            position: NormalizedPoint { x: 0.8, y: 0.8 },
        };
        let mut node = AnnotationNode::new(id.clone(), 1, original.clone()).unwrap();
        node.selected = true;
        actor.scene = SceneSnapshot::new(SceneRevision(1), vec![node.clone()], None).unwrap();
        actor.apply_interaction_scene(&mut renderer);
        let request = renderer.on_display_tick(1).unwrap();
        let (_, before) = renderer.render_headless_capture(request).unwrap();
        actor.publish_interaction(
            &mut renderer,
            InteractionEvent::GeometryEditStarted {
                annotation_id: id.clone(),
                geometry: original,
                handle: None,
            },
        );
        actor.publish_interaction(
            &mut renderer,
            InteractionEvent::GeometryEditChanged {
                annotation_id: id.clone(),
                geometry: changed.clone(),
            },
        );
        let request = renderer.on_display_tick(2).unwrap();
        let (_, moving) = renderer.render_headless_capture(request).unwrap();
        assert!(
            before != moving,
            "same-revision native edit must move the rendered geometry"
        );
        node.geometry = changed;
        actor.handle_command(
            &mut renderer,
            AuthorizedImageRenderCommand::SetScene {
                scene: SceneSnapshot::new(SceneRevision(2), vec![node], None).unwrap(),
            },
        );
        actor.publish_interaction(
            &mut renderer,
            InteractionEvent::GeometryEditCancelled { annotation_id: id },
        );
        let request = renderer.on_display_tick(3).unwrap();
        let (_, restored) = renderer.render_headless_capture(request).unwrap();
        assert!(
            before == restored,
            "cancel must use the captured original, not the staged scene"
        );
        actor.flush_continuous_events();
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::GeometryEditChanged { .. }))
        );
    }

    #[test]
    fn actor_consumes_complete_eight_k_stream_beyond_handoff_capacity() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none()
            || std::env::var_os("VIEWER_RUN_LARGE_IMAGE_TESTS").is_none()
        {
            return;
        }
        let (cache, mut actor, mut renderer, events) = test_actor();
        let fixture = cache.path().join("stream-8k.jpg");
        assert!(
            std::process::Command::new("/usr/bin/sips")
                .args(["--resampleHeightWidth", "4320", "7680"])
                .arg(viewer_test_support::image_fixtures::image_fixture(
                    "srgb.jpg"
                ))
                .arg("--out")
                .arg(&fixture)
                .output()
                .unwrap()
                .status
                .success()
        );
        let source =
            viewer_platform_macos::image_render::AuthorizedImageSource::authorize_for_process(
                &fixture,
            )
            .unwrap();
        let session_id = viewer_render_core::RenderSessionId(1);
        let generation = AssetGeneration(1);
        actor.open(&mut renderer, session_id, generation, source.clone());
        let (probe, preview) = actor
            .provider
            .probe_and_request_preview(&source, generation, PreviewRequest::new(100, 100).unwrap())
            .unwrap();
        actor.install_resource(&mut renderer, session_id, generation, probe, preview);
        actor.decode_worker.shutdown();
        actor.decode_results = Arc::default();
        actor.decode_worker = DecodeWorker::start(
            actor.provider.clone(),
            actor.decode_results.clone(),
            DisplayTickSignal::default(),
        )
        .unwrap();
        actor.viewport =
            ViewportLayout::new(LogicalSize::new(7680.0, 4320.0).unwrap(), 1.0, 1.0).unwrap();
        assert!(actor.update_transform(&mut renderer));
        assert_eq!(actor.resource_plans[0].level, 0);
        assert_eq!(actor.resource_plans[0].required_tiles.len(), 135);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        // Deliberately pause the actual actor consumer until the producer fills
        // the FIFO. Its one current tile is the only extra decoded allocation.
        while actor.decode_results.queued_bytes() < handoff::HANDOFF_BYTES {
            assert!(
                std::time::Instant::now() < deadline,
                "worker did not fill byte handoff"
            );
            thread::sleep(Duration::from_millis(2));
        }
        let memory = actor.provider.memory().clone();
        assert!(
            memory
                .snapshot()
                .bytes_for_class(viewer_render_core::AllocationClass::DecodedPixels)
                <= handoff::HANDOFF_BYTES + 512 * 512 * 4
        );
        let mut count = 0;
        let mut bytes = 0;
        loop {
            renderer.poll_gpu_timings().unwrap();
            actor.progress_upload(&mut renderer);
            if actor.pending_upload.is_none()
                && let Some(result) = actor.decode_results.take()
            {
                let completed = matches!(&result, DecodeResult::RefinementCompleted { .. });
                if let DecodeResult::Refined { resource, .. } = &result {
                    count += 1;
                    bytes += resource.pixels.len();
                    let shared = resource.pixels.clone();
                    let gpu =
                        gpu_resource((**resource).clone(), SourceSize::new(7680, 4320).unwrap())
                            .unwrap();
                    if let DecodedResource::Tile { pixels, .. } = gpu {
                        assert_eq!(pixels.as_ptr(), shared.as_ptr());
                    }
                }
                assert!(
                    !matches!(&result, DecodeResult::DecodeFailed { .. }),
                    "full 8K stream must not fail"
                );
                actor.handle_decode_result(&mut renderer, result);
                if completed {
                    break;
                }
            } else {
                thread::sleep(Duration::from_millis(1));
            }
            assert!(
                std::time::Instant::now() < deadline,
                "actor did not consume completion manifest"
            );
        }
        actor.decode_worker.shutdown();
        assert_eq!((count, bytes), (135, 7680 * 4320 * 4));
        assert_eq!(actor.refinement_keys.len(), 135);
        assert_eq!(
            renderer.retained_scene_resources().image_handles().len(),
            136
        );
        assert_eq!(
            memory
                .snapshot()
                .bytes_for_class(viewer_render_core::AllocationClass::DecodedPixels),
            0
        );
        assert!(memory.snapshot().gpu_bytes >= 7680 * 4320 * 4);
        assert!(memory.snapshot().peak_gpu_bytes <= 256 << 20);
        assert!(memory.snapshot().peak_combined_bytes <= 512 << 20);
        eprintln!(
            "8K actor streamed {count} tiles / {bytes} bytes; accounted peak={} bytes; GPU peak={} bytes; live GPU={} bytes",
            memory.snapshot().peak_combined_bytes,
            memory.snapshot().peak_gpu_bytes,
            memory.snapshot().gpu_bytes
        );
        assert!(
            !events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::Failed { .. }))
        );
    }

    #[test]
    fn actor_selection_is_available_to_the_next_native_pointer_boundary() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, events) = test_actor();
        actor.session_id = Some(viewer_render_core::RenderSessionId(1));
        actor.generation = AssetGeneration(1);
        actor.source_size = Some(SourceSize::new(100, 100).unwrap());
        actor.update_transform(&mut renderer);
        let geometry = AnnotationGeometry::Point {
            position: NormalizedPoint { x: 0.2, y: 0.2 },
        };
        actor.scene = SceneSnapshot::new(
            SceneRevision(1),
            vec![AnnotationNode::new(AnnotationId::new("editable").unwrap(), 1, geometry).unwrap()],
            None,
        )
        .unwrap();
        for phase in [PointerPhase::Down, PointerPhase::Up, PointerPhase::Down] {
            actor.handle_input(
                &mut renderer,
                NativeInput::Pointer(PointerSample {
                    phase,
                    location: LogicalPoint { x: 23.0, y: 23.0 },
                    button: PointerButton::Primary,
                    pressure: 0.0,
                    modifiers: Modifiers::default(),
                    timestamp_ns: 1,
                }),
            );
        }
        assert!(
            events
                .take()
                .iter()
                .any(|event| matches!(event, ImageRenderEventDto::GeometryEditStarted { .. })),
            "native selection cannot wait for a React roundtrip before the next drag"
        );
    }

    #[test]
    fn actor_selection_preserves_read_only_scene_permission() {
        if std::env::var_os("VIEWER_RUN_METAL_TESTS").is_none() {
            return;
        }
        let (_cache, mut actor, mut renderer, _events) = test_actor();
        actor.scene = SceneSnapshot::empty(SceneRevision(1)).with_annotations_editable(false);
        actor.apply_native_selection(&mut renderer, None);
        assert!(
            !actor.scene.annotations_editable(),
            "a native selection must not upgrade read-only scene permissions"
        );
    }

    fn pointer(phase: PointerPhase, x: f64) -> NativeInput {
        NativeInput::Pointer(PointerSample {
            phase,
            location: LogicalPoint { x, y: 12.0 },
            button: PointerButton::Primary,
            pressure: 0.0,
            modifiers: Modifiers::default(),
            timestamp_ns: x.max(0.0) as u64,
        })
    }

    #[test]
    fn saturated_continuous_input_preserves_pointer_boundaries() {
        let inputs = NativeInputAccumulator::default();
        assert!(inputs.push(pointer(PointerPhase::Down, 0.0)));
        for x in 1..=10_000 {
            assert!(!inputs.push(pointer(PointerPhase::Move, f64::from(x))));
        }
        assert!(inputs.push(pointer(PointerPhase::Up, 10_001.0)));

        let boundaries = inputs.take_boundaries();
        assert_eq!(boundaries.len(), 3);
        assert!(matches!(
            boundaries[0],
            NativeInput::Pointer(sample) if sample.phase == PointerPhase::Down
        ));
        assert!(matches!(
            boundaries[1],
            NativeInput::Pointer(sample) if sample.phase == PointerPhase::Move
        ));
        assert!(matches!(
            boundaries[2],
            NativeInput::Pointer(sample) if sample.phase == PointerPhase::Up
        ));
        assert!(inputs.take_continuous().is_empty());
    }

    #[test]
    fn quick_click_boundaries_stay_ordered_and_overflow_cancels_capture() {
        let inputs = NativeInputAccumulator::default();
        assert!(inputs.push(pointer(PointerPhase::Down, 1.0)));
        assert!(inputs.push(pointer(PointerPhase::Up, 2.0)));
        assert!(inputs.push(pointer(PointerPhase::Down, 3.0)));
        assert!(inputs.push(pointer(PointerPhase::Up, 4.0)));
        let phases: Vec<_> = inputs
            .take_boundaries()
            .into_iter()
            .filter_map(|input| match input {
                NativeInput::Pointer(sample) => Some(sample.phase),
                _ => None,
            })
            .collect();
        assert_eq!(
            phases,
            [
                PointerPhase::Down,
                PointerPhase::Up,
                PointerPhase::Down,
                PointerPhase::Up,
            ]
        );

        assert!(!inputs.push(NativeInput::Scroll(ScrollSample {
            location: LogicalPoint::ZERO,
            delta: LogicalPoint { x: 2.0, y: 3.0 },
            modifiers: Modifiers::default(),
            timestamp_ns: 5,
        })));

        for x in 0..=INPUT_BOUNDARY_CAPACITY {
            assert!(inputs.push(pointer(PointerPhase::Down, x as f64)));
        }
        let overflowed = inputs.take_boundaries();
        assert!(overflowed.len() <= INPUT_BOUNDARY_CAPACITY);
        assert!(
            overflowed
                .iter()
                .any(|input| matches!(input, NativeInput::Cancel))
        );
        assert!(inputs.take_continuous().is_empty());
    }

    #[test]
    fn cancel_discards_all_stale_continuous_camera_input() {
        let inputs = NativeInputAccumulator::default();
        assert!(!inputs.push(NativeInput::Scroll(ScrollSample {
            location: LogicalPoint::ZERO,
            delta: LogicalPoint { x: 5.0, y: 7.0 },
            modifiers: Modifiers::default(),
            timestamp_ns: 1,
        })));
        assert!(!inputs.push(NativeInput::Magnify(MagnifySample {
            location: LogicalPoint::ZERO,
            factor: 1.5,
            timestamp_ns: 2,
        })));

        assert!(inputs.push(NativeInput::Cancel));

        assert!(matches!(
            inputs.take_boundaries().as_slice(),
            [NativeInput::Cancel]
        ));
        assert!(inputs.take_continuous().is_empty());
    }

    #[test]
    fn hover_samples_are_coalesced_without_crossing_the_ui_bridge() {
        let inputs = NativeInputAccumulator::default();
        assert!(!inputs.push(NativeInput::Hover(HoverSample {
            location: LogicalPoint { x: 20.0, y: 30.0 },
            active: true,
            timestamp_ns: 1,
        })));
        assert!(!inputs.push(NativeInput::Hover(HoverSample {
            location: LogicalPoint { x: 40.0, y: 50.0 },
            active: false,
            timestamp_ns: 2,
        })));

        assert_eq!(
            inputs.take_continuous(),
            vec![NativeInput::Hover(HoverSample {
                location: LogicalPoint { x: 40.0, y: 50.0 },
                active: false,
                timestamp_ns: 2,
            })]
        );
    }

    #[test]
    fn native_magnifier_tracks_pointer_and_preserves_rectangular_size() {
        let transform = TransformSnapshot::new(
            SourceSize::new(1_000, 600).unwrap(),
            ViewportLayout::new(LogicalSize::new(1_000.0, 600.0).unwrap(), 2.0, FIT_INSET).unwrap(),
            CameraState::fit(Rotation::Deg0),
        )
        .unwrap();
        assert_eq!(
            transform.physical_viewport(),
            PhysicalSize {
                width: 2_000,
                height: 1_200,
            }
        );
        let preferences = ImageRenderMagnifierPreferences {
            width_px: 300.0,
            height_px: 200.0,
            magnification: 2.0,
            shape: viewer_render_wgpu::MagnifierShape::RoundedRectangle,
        };

        let first = magnifier_config_at_pointer(
            preferences,
            LogicalPoint { x: 500.0, y: 300.0 },
            transform,
        )
        .unwrap();
        let second = magnifier_config_at_pointer(
            preferences,
            LogicalPoint { x: 800.0, y: 300.0 },
            transform,
        )
        .unwrap();

        assert_eq!((first.width_px, first.height_px), (300.0, 200.0));
        assert_ne!(first.focus, second.focus);
        assert_ne!(first.center, second.center);
        assert!(magnifier_config_at_pointer(preferences, LogicalPoint::ZERO, transform,).is_none());
    }

    #[test]
    fn content_frame_gate_rejects_clear_and_stale_frames() {
        let mut gate = ContentFrameGate::default();
        gate.begin_generation();
        assert!(!gate.can_publish(AssetGeneration(4)));

        gate.install(AssetGeneration(4));
        assert!(!gate.can_publish(AssetGeneration(3)));
        assert!(gate.can_publish(AssetGeneration(4)));

        gate.begin_generation();
        assert!(!gate.can_publish(AssetGeneration(4)));
        assert!(!gate.can_publish(AssetGeneration(5)));
    }

    #[test]
    fn renderer_controls_are_lossless_latest_value_slots() {
        let controls = RendererControlAccumulator::default();
        for width in 1..=10_000 {
            controls.set_resize(SurfaceLayout {
                left: 0.0,
                top: 0.0,
                width: f64::from(width),
                height: 720.0,
                scale_factor: 2.0,
            });
        }
        controls.set_tool(InteractionMode::Brush);
        controls.set_tool(InteractionMode::Rectangle);

        let latest = controls.take();
        assert_eq!(latest.resize.expect("latest resize").width, 10_000.0);
        assert_eq!(latest.tool, Some(InteractionMode::Rectangle));
        let drained = controls.take();
        assert!(drained.resize.is_none());
        assert!(drained.tool.is_none());
    }

    #[test]
    fn stalled_initializer_latch_blocks_every_retry_without_spawning_again() {
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let spawned = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let worker_spawned = Arc::clone(&spawned);
        let join = thread::spawn(move || {
            worker_spawned.fetch_add(1, Ordering::Relaxed);
            let _ = release_receiver.recv();
        });
        while spawned.load(Ordering::Relaxed) == 0 {
            thread::yield_now();
        }
        let mut state = NativeDriverState {
            initializer: Some(RendererActorInitializer {
                wake: DisplayTickSignal::default(),
                shutdown: Arc::new(AtomicBool::new(false)),
                join: Some(join),
            }),
            ..NativeDriverState::default()
        };

        for _ in 0..10_000 {
            assert!(state.initializer_blocks_start());
        }
        assert_eq!(spawned.load(Ordering::Relaxed), 1);

        release_sender.send(()).unwrap();
        state.initializer.as_mut().unwrap().join().unwrap();
    }

    #[test]
    fn pre_surface_commands_coalesce_to_latest_bounded_state() {
        let mut pending = PendingRendererState::default();
        for zoom in 1..=10_000 {
            pending.record(AuthorizedImageRenderCommand::Camera {
                camera: CameraState {
                    mode: CameraMode::Free,
                    zoom: f64::from(zoom),
                    rotation: Rotation::Deg0,
                    offset: LogicalPoint::ZERO,
                },
            });
        }

        let commands = pending.take_ordered();
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            commands.last(),
            Some(AuthorizedImageRenderCommand::Camera { camera }) if camera.zoom == 10_000.0
        ));
    }
}
