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
    NativeInputSink, PixelFormat, PreviewRequest, SurfaceLayout, system_ordinal_glyph_atlas,
};
use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, AssetGeneration, CameraState, DraftGeometry,
    InteractionController, InteractionEvent, InteractionMode, LogicalSize, MemoryBudget,
    NativeInput, NormalizedPoint, NormalizedRect, PointerPhase, Rotation, SceneRevision,
    SceneSnapshot, SourceSize, TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::{DecodedResource, RendererDescriptor, SurfaceHandles, WgpuImageRenderer};

use super::{AuthorizedImageRenderCommand, ImageRenderDriver, ImageRenderRuntimeError};
use crate::{
    dto::{
        ImageRenderAnnotationGeometryDto, ImageRenderCameraDto, ImageRenderCameraModeDto,
        ImageRenderEventDto, ImageRenderPointDto, ImageRenderRotationDto,
    },
    image_render_events::ImageRenderEventPort,
};

const FIT_INSET: f64 = 0.94;
const ACTOR_START_TIMEOUT: Duration = Duration::from_secs(15);
const ACTOR_MAILBOX_CAPACITY: usize = 256;
const INPUT_BOUNDARY_CAPACITY: usize = 32;

pub struct NativeImageRenderDriver {
    cache_root: PathBuf,
    budget: MemoryBudget,
    events: Arc<dyn ImageRenderEventPort>,
    state: Mutex<NativeDriverState>,
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
    pub fn new(cache_root: &Path, events: Arc<dyn ImageRenderEventPort>) -> Self {
        let budget = MemoryBudget::baseline_8gb();
        Self {
            cache_root: cache_root.to_path_buf(),
            budget,
            events,
            state: Mutex::new(NativeDriverState::default()),
        }
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
        let provider = MacImageResourceProvider::new(&self.cache_root, self.budget)
            .map_err(|_| ImageRenderRuntimeError::DriverUnavailable)?;
        let mounted = run_on_main(move |main_thread| {
            let host = MacImageRenderHost::mount(&window, layout)
                .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
            let handles = host
                .surface()
                .renderer_surface_handles()
                .map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
            let glyph_atlas =
                system_ordinal_glyph_atlas().map_err(|_| ImageRenderRuntimeError::DriverFailed)?;
            Ok::<_, ImageRenderRuntimeError>((
                MainThreadBound::new(host, main_thread),
                handles,
                glyph_atlas,
            ))
        })?;
        let (mut host, handles, glyph_atlas) = mounted;
        let display_signal = DisplayTickSignal::default();
        let mut actor = match RendererActorHandle::start(
            handles,
            layout,
            display_signal.clone(),
            provider,
            Arc::clone(&self.events),
            glyph_atlas,
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
                        WgpuImageRenderer::new(descriptor)
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
    },
}

#[derive(Default)]
struct NativeInputAccumulator {
    state: Mutex<NativeInputState>,
}

#[derive(Default)]
struct NativeInputState {
    boundaries: VecDeque<NativeInput>,
    pointer_move: Option<NativeInput>,
    scroll: Option<NativeInput>,
    magnify: Option<NativeInput>,
}

impl NativeInputState {
    fn push_boundary(&mut self, input: NativeInput) {
        if self.boundaries.len() >= INPUT_BOUNDARY_CAPACITY {
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
}

#[derive(Default)]
struct DecodeJobState {
    latest: Option<DecodeJob>,
    shutdown: bool,
}

#[derive(Default)]
struct DecodeResultSlot(Mutex<Option<DecodeResult>>);

impl DecodeResultSlot {
    fn publish(&self, result: DecodeResult) {
        *lock(&self.0) = Some(result);
    }

    fn take(&self) -> Option<DecodeResult> {
        lock(&self.0).take()
    }

    fn clear(&self) {
        lock(&self.0).take();
    }
}

struct DecodeWorker {
    jobs: Arc<(Mutex<DecodeJobState>, Condvar)>,
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
        let join = thread::Builder::new()
            .name("viewer-image-decode".into())
            .spawn(move || {
                loop {
                    let job = {
                        let (mutex, available) = worker_jobs.as_ref();
                        let mut state = lock(mutex);
                        while state.latest.is_none() && !state.shutdown {
                            state = available
                                .wait(state)
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                        }
                        if state.shutdown {
                            return;
                        }
                        state.latest.take().expect("decode job is available")
                    };
                    let request =
                        PreviewRequest::new(job.physical_width.max(1), job.physical_height.max(1));
                    let result = request.and_then(|request| {
                        provider.probe_and_request_preview(&job.source, job.generation, request)
                    });
                    let result = match result {
                        Ok((probe, resource)) => DecodeResult::Decoded {
                            session_id: job.session_id,
                            generation: job.generation,
                            probe,
                            resource: Box::new(resource),
                        },
                        Err(error) => DecodeResult::DecodeFailed {
                            session_id: job.session_id,
                            generation: job.generation,
                            cancelled: matches!(
                                error,
                                viewer_platform_macos::image_render::ImageResourceError::Cancelled
                            ),
                        },
                    };
                    results.publish(result);
                    wake.wake();
                }
            })
            .map_err(|error| format!("decode worker initialization failed: {error}"))?;
        Ok(Self {
            jobs,
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
        available.notify_one();
        Ok(())
    }

    fn shutdown(&mut self) {
        let (mutex, available) = self.jobs.as_ref();
        {
            let mut state = lock(mutex);
            state.shutdown = true;
            state.latest = None;
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

struct RendererActor {
    provider: MacImageResourceProvider,
    _memory_pressure: MacMemoryPressureMonitor,
    decode_worker: DecodeWorker,
    decode_results: Arc<DecodeResultSlot>,
    inputs: Arc<NativeInputAccumulator>,
    controls: Arc<RendererControlAccumulator>,
    events: Arc<dyn ImageRenderEventPort>,
    viewport: ViewportLayout,
    session_id: Option<viewer_render_core::RenderSessionId>,
    generation: AssetGeneration,
    source_size: Option<SourceSize>,
    camera: CameraState,
    transform: Option<TransformSnapshot>,
    scene: SceneSnapshot,
    interaction: InteractionController,
    pending_camera_event: Option<ImageRenderEventDto>,
    pending_draft_event: Option<ImageRenderEventDto>,
    pending_editor_placement_event: Option<ImageRenderEventDto>,
}

impl RendererActor {
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
        let memory_pressure = MacMemoryPressureMonitor::install(provider.memory_pressure_sink());
        let decode_results = Arc::new(DecodeResultSlot::default());
        let decode_worker =
            DecodeWorker::start(provider.clone(), Arc::clone(&decode_results), wake)?;
        Ok(Self {
            provider,
            _memory_pressure: memory_pressure,
            decode_worker,
            decode_results,
            inputs,
            controls,
            events,
            viewport,
            session_id: None,
            generation: AssetGeneration(0),
            source_size: None,
            camera: CameraState::fit(Rotation::Deg0),
            transform: None,
            scene,
            interaction: InteractionController::new(InteractionMode::Browse),
            pending_camera_event: None,
            pending_draft_event: None,
            pending_editor_placement_event: None,
        })
    }

    fn run(
        &mut self,
        renderer: &mut WgpuImageRenderer,
        receiver: mpsc::Receiver<RendererMessage>,
        display_signal: DisplayTickSignal,
        shutdown: Arc<AtomicBool>,
    ) {
        while !shutdown.load(Ordering::Acquire) {
            let mut did_work = false;
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
            if let Some(result) = self.decode_results.take() {
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
                            if let Some(session_id) = self.session_id {
                                self.events.publish(ImageRenderEventDto::FramePresented {
                                    session_id: session_id.0,
                                    asset_generation: self.generation.0,
                                    scene_revision: receipt.scene_revision.0,
                                    frame_index: receipt.frame_index,
                                });
                            }
                        }
                        Ok(_) => {}
                        Err(_) => self.publish_failure("image_render_frame_failed", true),
                    }
                }
            }
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
            } => {
                if !cancelled
                    && self.session_id == Some(session_id)
                    && self.generation == generation
                {
                    self.publish_failure("image_render_decode_failed", true);
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
                if renderer.apply_scene(&self.scene).is_err() {
                    self.publish_failure("image_render_scene_failed", true);
                }
            }
            AuthorizedImageRenderCommand::SetScene { scene } => {
                self.scene = scene;
                if renderer.apply_scene(&self.scene).is_err() {
                    self.publish_failure("image_render_scene_failed", true);
                }
            }
            AuthorizedImageRenderCommand::Camera { camera } => {
                self.camera = camera;
                self.update_transform(renderer);
            }
            AuthorizedImageRenderCommand::SetMagnifier { magnifier } => {
                if renderer.set_magnifier(magnifier).is_err() {
                    self.publish_failure("image_render_magnifier_failed", true);
                }
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
        self.transform = None;
        self.camera = CameraState::fit(Rotation::Deg0);
        self.scene = SceneSnapshot::empty(SceneRevision(0));
        self.pending_camera_event = None;
        self.pending_draft_event = None;
        self.pending_editor_placement_event = None;
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
                physical_width: physical.width,
                physical_height: physical.height,
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
            pixels: resource.pixels.as_ref().to_vec(),
        };
        if renderer.upload_resource(gpu_resource).is_err() {
            self.publish_failure("image_render_upload_failed", true);
            return;
        }
        self.source_size = Some(source_size);
        self.update_transform(renderer);
        self.events.publish(ImageRenderEventDto::Ready {
            session_id: session_id.0,
            asset_generation: generation.0,
            width: source_size.width,
            height: source_size.height,
        });
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

    fn update_transform(&mut self, renderer: &mut WgpuImageRenderer) {
        let Some(source_size) = self.source_size else {
            return;
        };
        match TransformSnapshot::new(source_size, self.viewport, self.camera) {
            Ok(transform) => {
                self.transform = Some(transform);
                if renderer.set_transform(transform).is_err() {
                    self.publish_failure("image_render_transform_failed", true);
                }
            }
            Err(_) => self.publish_failure("image_render_transform_invalid", false),
        }
    }

    fn handle_input(&mut self, renderer: &mut WgpuImageRenderer, input: NativeInput) {
        let Some(transform) = self.transform else {
            return;
        };
        for event in self
            .interaction
            .handle_input(input, &transform, &self.scene)
        {
            self.publish_interaction(renderer, event);
        }
    }

    fn publish_interaction(&mut self, renderer: &mut WgpuImageRenderer, event: InteractionEvent) {
        let Some(session_id) = self.session_id else {
            return;
        };
        let generation = self.generation.0;
        let event = match event {
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
                self.apply_native_draft(renderer, geometry.clone());
                ImageRenderEventDto::DraftCompleted {
                    session_id: session_id.0,
                    asset_generation: generation,
                    geometry: annotation_geometry_dto(geometry),
                }
            }
            InteractionEvent::DraftCancelled => {
                self.pending_draft_event = None;
                if renderer.apply_scene(&self.scene).is_err() {
                    self.publish_failure("image_render_scene_failed", true);
                }
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

    fn apply_native_draft(&self, renderer: &mut WgpuImageRenderer, geometry: AnnotationGeometry) {
        let Ok(mut draft) = AnnotationNode::new(
            AnnotationId::new("__viewer_native_draft").expect("static annotation id is valid"),
            0,
            geometry,
        ) else {
            return;
        };
        draft.draft = true;
        draft.style.dashed = true;
        let Ok(projected) = SceneSnapshot::new(self.scene.revision(), Vec::new(), Some(draft))
        else {
            return;
        };
        if renderer.apply_draft_overlay(&projected).is_err() {
            self.publish_failure("image_render_scene_failed", true);
        }
    }

    fn apply_native_selection(
        &self,
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
        if renderer.apply_transient_scene(&projected).is_err() {
            self.publish_failure("image_render_scene_failed", true);
        }
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
        CameraMode, LogicalPoint, MagnifySample, Modifiers, PointerButton, PointerSample,
        ScrollSample,
    };

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
