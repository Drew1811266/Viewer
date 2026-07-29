use super::{encode::encode_png, image_io::ImageIoBackend};
use async_trait::async_trait;
use block2::RcBlock;
use objc2::{AnyThread, rc::Retained};
use objc2_core_foundation::CGSize;
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_quick_look_thumbnailing::{
    QLThumbnailGenerationRequest, QLThumbnailGenerationRequestRepresentationTypes,
    QLThumbnailGenerator, QLThumbnailRepresentation,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use tokio::sync::{RwLock, Semaphore, oneshot};
use viewer_application::ImageRequestCancellation;
use viewer_application::{ImageArtifact, ImageBackend, ImageError, ImagePort, ImageRequest};
use viewer_domain::{SessionId, image::ImageProbe};

const QUICK_LOOK_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONCURRENT_RENDERS: usize = 4;
static GLOBAL_RENDER_LIMIT: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn global_render_limit() -> Arc<Semaphore> {
    Arc::clone(GLOBAL_RENDER_LIMIT.get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT_RENDERS))))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderedDimensions {
    pub width: u32,
    pub height: u32,
}

#[async_trait]
pub trait QuickLookThumbnailBackend: Send + Sync {
    async fn thumbnail(
        &self,
        request: &ImageRequest,
        destination: &Path,
    ) -> Result<RenderedDimensions, ImageError>;

    async fn cancel_session(&self, session_id: SessionId);
}

#[async_trait]
pub trait ImageIoRenderBackend: Send + Sync {
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError>;

    async fn render(
        &self,
        request: &ImageRequest,
        destination: &Path,
    ) -> Result<RenderedDimensions, ImageError>;
}

#[async_trait]
impl ImageIoRenderBackend for ImageIoBackend {
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError> {
        let backend = *self;
        let source = source.to_path_buf();
        tokio::task::spawn_blocking(move || backend.probe_sync(source))
            .await
            .map_err(|error| ImageError::Io(format!("Image I/O worker failed: {error}")))?
    }

    async fn render(
        &self,
        request: &ImageRequest,
        destination: &Path,
    ) -> Result<RenderedDimensions, ImageError> {
        let backend = *self;
        let source = request.source.clone();
        let kind = request.kind;
        let destination = destination.to_path_buf();
        let (width, height) =
            tokio::task::spawn_blocking(move || backend.render_sync(source, kind, destination))
                .await
                .map_err(|error| ImageError::Io(format!("Image I/O worker failed: {error}")))??;
        Ok(RenderedDimensions { width, height })
    }
}

pub struct MacImagePort<Q = QuickLookBackend, I = ImageIoBackend> {
    quick_look: Arc<Q>,
    image_io: Arc<I>,
    cache_root: PathBuf,
    cancelled_sessions: Arc<RwLock<HashSet<SessionId>>>,
    render_limit: Arc<Semaphore>,
}

impl MacImagePort<QuickLookBackend, ImageIoBackend> {
    pub fn new(cache_root: impl Into<PathBuf>) -> Result<Self, ImageError> {
        Ok(Self::with_backends_and_render_limit(
            cache_root,
            QuickLookBackend::new()?,
            ImageIoBackend::default(),
            global_render_limit(),
        ))
    }
}

impl<Q, I> MacImagePort<Q, I> {
    pub fn with_backends(cache_root: impl Into<PathBuf>, quick_look: Q, image_io: I) -> Self {
        Self::with_backends_and_render_limit(
            cache_root,
            quick_look,
            image_io,
            Arc::new(Semaphore::new(MAX_CONCURRENT_RENDERS)),
        )
    }

    fn with_backends_and_render_limit(
        cache_root: impl Into<PathBuf>,
        quick_look: Q,
        image_io: I,
        render_limit: Arc<Semaphore>,
    ) -> Self {
        Self {
            quick_look: Arc::new(quick_look),
            image_io: Arc::new(image_io),
            cache_root: cache_root.into(),
            cancelled_sessions: Arc::new(RwLock::new(HashSet::new())),
            render_limit,
        }
    }

    fn artifact_destination(
        cache_root: &Path,
        request: &ImageRequest,
        backend: ImageBackend,
    ) -> Result<PathBuf, ImageError> {
        let session_directory = cache_root.join(request.session_id.to_string());
        fs::create_dir_all(&session_directory).map_err(|error| {
            ImageError::Io(format!(
                "create image session cache {}: {error}",
                session_directory.display()
            ))
        })?;
        let backend_label = match backend {
            ImageBackend::QuickLook => "quick-look",
            ImageBackend::ImageIo => "image-io",
        };
        Ok(session_directory.join(format!(
            "{}-{backend_label}-{}.png",
            request.entity_id,
            uuid::Uuid::new_v4()
        )))
    }

    async fn is_cancelled(
        cancelled_sessions: &RwLock<HashSet<SessionId>>,
        session_id: SessionId,
    ) -> bool {
        cancelled_sessions.read().await.contains(&session_id)
    }
}

struct CancelRequestOnDrop {
    cancellation: ImageRequestCancellation,
    delivery: Arc<RenderDelivery>,
    armed: bool,
}

impl CancelRequestOnDrop {
    fn new(cancellation: ImageRequestCancellation, delivery: Arc<RenderDelivery>) -> Self {
        Self {
            cancellation,
            delivery,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CancelRequestOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.cancellation.cancel();
            self.delivery.cancel();
        }
    }
}

enum RenderDeliveryState {
    Active,
    Completed(Option<PathBuf>),
    Delivered,
    Cancelled,
}

struct RenderDelivery {
    state: Mutex<RenderDeliveryState>,
}

impl RenderDelivery {
    fn new() -> Self {
        Self {
            state: Mutex::new(RenderDeliveryState::Active),
        }
    }

    fn finish(
        &self,
        result: Result<ImageArtifact, ImageError>,
    ) -> Result<ImageArtifact, ImageError> {
        let artifact_path = result
            .as_ref()
            .ok()
            .map(|artifact| artifact.cache_path.clone());
        let mut state = self.state.lock().unwrap();
        match &*state {
            RenderDeliveryState::Active => {
                *state = RenderDeliveryState::Completed(artifact_path);
                result
            }
            RenderDeliveryState::Cancelled => {
                if let Some(path) = artifact_path {
                    let _ = fs::remove_file(path);
                }
                Err(ImageError::Cancelled)
            }
            RenderDeliveryState::Completed(_) | RenderDeliveryState::Delivered => Err(
                ImageError::Io("image render delivery completed more than once".into()),
            ),
        }
    }

    fn deliver(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        if matches!(&*state, RenderDeliveryState::Completed(_)) {
            *state = RenderDeliveryState::Delivered;
            true
        } else {
            false
        }
    }

    fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        let previous = std::mem::replace(&mut *state, RenderDeliveryState::Cancelled);
        if let RenderDeliveryState::Completed(Some(path)) = previous {
            let _ = fs::remove_file(path);
        }
    }
}

async fn render_owned<Q, I>(
    quick_look: Arc<Q>,
    image_io: Arc<I>,
    cache_root: PathBuf,
    cancelled_sessions: Arc<RwLock<HashSet<SessionId>>>,
    render_limit: Arc<Semaphore>,
    request: ImageRequest,
) -> Result<ImageArtifact, ImageError>
where
    Q: QuickLookThumbnailBackend + 'static,
    I: ImageIoRenderBackend + 'static,
{
    if request.cancellation.is_cancelled()
        || MacImagePort::<Q, I>::is_cancelled(&cancelled_sessions, request.session_id).await
    {
        return Err(ImageError::Cancelled);
    }
    let _permit = tokio::select! {
        biased;
        _ = request.cancellation.cancelled() => return Err(ImageError::Cancelled),
        permit = render_limit.acquire_owned() => {
            permit.map_err(|_| ImageError::Io("image render scheduler is unavailable".into()))?
        }
    };
    if request.cancellation.is_cancelled()
        || MacImagePort::<Q, I>::is_cancelled(&cancelled_sessions, request.session_id).await
    {
        return Err(ImageError::Cancelled);
    }

    let (destination, dimensions, backend) = if matches!(
        request.kind,
        viewer_domain::image::ImageRepresentationKind::Thumbnail { .. }
    ) {
        let quick_look_destination = MacImagePort::<Q, I>::artifact_destination(
            &cache_root,
            &request,
            ImageBackend::QuickLook,
        )?;
        match quick_look
            .thumbnail(&request, &quick_look_destination)
            .await
        {
            Ok(dimensions) => (quick_look_destination, dimensions, ImageBackend::QuickLook),
            Err(ImageError::Cancelled) => {
                let _ = fs::remove_file(quick_look_destination);
                return Err(ImageError::Cancelled);
            }
            Err(_) => {
                let _ = fs::remove_file(quick_look_destination);
                if request.cancellation.is_cancelled()
                    || MacImagePort::<Q, I>::is_cancelled(&cancelled_sessions, request.session_id)
                        .await
                {
                    return Err(ImageError::Cancelled);
                }
                let image_io_destination = MacImagePort::<Q, I>::artifact_destination(
                    &cache_root,
                    &request,
                    ImageBackend::ImageIo,
                )?;
                let dimensions = match image_io.render(&request, &image_io_destination).await {
                    Ok(dimensions) => dimensions,
                    Err(error) => {
                        let _ = fs::remove_file(image_io_destination);
                        return Err(error);
                    }
                };
                (image_io_destination, dimensions, ImageBackend::ImageIo)
            }
        }
    } else {
        let image_io_destination = MacImagePort::<Q, I>::artifact_destination(
            &cache_root,
            &request,
            ImageBackend::ImageIo,
        )?;
        let dimensions = match image_io.render(&request, &image_io_destination).await {
            Ok(dimensions) => dimensions,
            Err(error) => {
                let _ = fs::remove_file(image_io_destination);
                return Err(error);
            }
        };
        (image_io_destination, dimensions, ImageBackend::ImageIo)
    };

    if request.cancellation.is_cancelled()
        || MacImagePort::<Q, I>::is_cancelled(&cancelled_sessions, request.session_id).await
    {
        let _ = fs::remove_file(&destination);
        return Err(ImageError::Cancelled);
    }

    Ok(ImageArtifact {
        cache_path: destination,
        mime: "image/png",
        width: dimensions.width,
        height: dimensions.height,
        backend,
    })
}

#[async_trait]
impl<Q, I> ImagePort for MacImagePort<Q, I>
where
    Q: QuickLookThumbnailBackend + 'static,
    I: ImageIoRenderBackend + 'static,
{
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError> {
        self.image_io.probe(source).await
    }

    async fn render(&self, request: ImageRequest) -> Result<ImageArtifact, ImageError> {
        let delivery = Arc::new(RenderDelivery::new());
        let mut cancel_on_drop =
            CancelRequestOnDrop::new(request.cancellation.clone(), Arc::clone(&delivery));
        let inner_delivery = Arc::clone(&delivery);
        let quick_look = Arc::clone(&self.quick_look);
        let image_io = Arc::clone(&self.image_io);
        let cache_root = self.cache_root.clone();
        let cancelled_sessions = Arc::clone(&self.cancelled_sessions);
        let render_limit = Arc::clone(&self.render_limit);
        let rendering = tokio::spawn(async move {
            let result = render_owned(
                quick_look,
                image_io,
                cache_root,
                cancelled_sessions,
                render_limit,
                request,
            )
            .await;
            inner_delivery.finish(result)
        });
        let result = rendering
            .await
            .map_err(|error| ImageError::Io(format!("image render coordinator failed: {error}")))?;
        if !delivery.deliver() {
            return Err(ImageError::Cancelled);
        }
        cancel_on_drop.disarm();
        result
    }

    async fn cancel_session(&self, session_id: SessionId) {
        self.cancelled_sessions.write().await.insert(session_id);
        self.quick_look.cancel_session(session_id).await;
    }
}

#[derive(Clone)]
pub struct QuickLookBackend {
    commands: mpsc::Sender<QuickLookCommand>,
    next_request_id: Arc<AtomicU64>,
    pending: Arc<Mutex<HashMap<u64, PendingQuickLookRequest>>>,
    _worker_lifetime: Arc<QuickLookWorkerLifetime>,
    timeout: Duration,
}

struct PendingQuickLookRequest {
    session_id: SessionId,
    cancelled: Arc<AtomicBool>,
}

struct QuickLookWorkerLifetime {
    commands: mpsc::Sender<QuickLookCommand>,
}

impl Drop for QuickLookWorkerLifetime {
    fn drop(&mut self) {
        let _ = self.commands.send(QuickLookCommand::Shutdown);
    }
}

impl QuickLookBackend {
    pub fn new() -> Result<Self, ImageError> {
        let (commands, receiver) = mpsc::channel();
        let callback_commands = commands.clone();
        thread::Builder::new()
            .name("viewer-quick-look".into())
            .spawn(move || run_quick_look_executor(receiver, callback_commands))
            .map_err(|error| {
                ImageError::Io(format!("start dedicated Quick Look worker: {error}"))
            })?;
        Ok(Self {
            commands: commands.clone(),
            next_request_id: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            _worker_lifetime: Arc::new(QuickLookWorkerLifetime {
                commands: commands.clone(),
            }),
            timeout: QUICK_LOOK_TIMEOUT,
        })
    }

    fn request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[async_trait]
impl QuickLookThumbnailBackend for QuickLookBackend {
    async fn thumbnail(
        &self,
        request: &ImageRequest,
        destination: &Path,
    ) -> Result<RenderedDimensions, ImageError> {
        if request.cancellation.is_cancelled() {
            return Err(ImageError::Cancelled);
        }
        let (physical_max_pixels, scale_milli) = match request.kind {
            viewer_domain::image::ImageRepresentationKind::Thumbnail {
                max_pixels,
                scale_milli,
            } if max_pixels > 0 && scale_milli > 0 => (max_pixels, scale_milli),
            _ => return Err(ImageError::Unsupported),
        };
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ImageError::Io(format!(
                    "create Quick Look destination directory {}: {error}",
                    parent.display()
                ))
            })?;
        }

        let request_id = self.request_id();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                request_id,
                PendingQuickLookRequest {
                    session_id: request.session_id,
                    cancelled: Arc::clone(&cancelled),
                },
            );
        let (responder, response) = oneshot::channel();
        if self
            .commands
            .send(QuickLookCommand::Generate {
                request_id,
                session_id: request.session_id,
                source: request.source.clone(),
                destination: destination.to_path_buf(),
                physical_max_pixels,
                scale_milli,
                cancelled: Arc::clone(&cancelled),
                responder,
            })
            .is_err()
        {
            self.pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&request_id);
            return Err(ImageError::Io("Quick Look worker is unavailable".into()));
        }

        tokio::pin!(response);
        let result = tokio::select! {
            biased;
            _ = request.cancellation.cancelled() => {
                cancelled.store(true, Ordering::Release);
                let _ = self.commands.send(QuickLookCommand::CancelRequest { request_id });
                let _ = response.as_mut().await;
                Err(ImageError::Cancelled)
            }
            result = tokio::time::timeout(self.timeout, response.as_mut()) => match result {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err(ImageError::Io(
                    "Quick Look worker ended before responding".into(),
                )),
                Err(_) => {
                    cancelled.store(true, Ordering::Release);
                    let _ = self
                        .commands
                        .send(QuickLookCommand::CancelRequest { request_id });
                    let _ = response.as_mut().await;
                    Err(ImageError::Io("Quick Look request timed out".into()))
                }
            },
        };
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&request_id);
        result
    }

    async fn cancel_session(&self, session_id: SessionId) {
        for pending in self
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .filter(|pending| pending.session_id == session_id)
        {
            pending.cancelled.store(true, Ordering::Release);
        }
        let (acknowledge, acknowledged) = oneshot::channel();
        if self
            .commands
            .send(QuickLookCommand::CancelSession {
                session_id,
                acknowledge,
            })
            .is_ok()
        {
            let _ = acknowledged.await;
        }
    }
}

enum QuickLookCommand {
    Generate {
        request_id: u64,
        session_id: SessionId,
        source: PathBuf,
        destination: PathBuf,
        physical_max_pixels: u32,
        scale_milli: u16,
        cancelled: Arc<AtomicBool>,
        responder: oneshot::Sender<Result<RenderedDimensions, ImageError>>,
    },
    CancelRequest {
        request_id: u64,
    },
    CancelSession {
        session_id: SessionId,
        acknowledge: oneshot::Sender<()>,
    },
    Completed {
        request_id: u64,
        result: Result<RenderedDimensions, ImageError>,
    },
    Shutdown,
}

struct ActiveQuickLookRequest {
    session_id: SessionId,
    request: Retained<QLThumbnailGenerationRequest>,
    cancelled: Arc<AtomicBool>,
    responder: oneshot::Sender<Result<RenderedDimensions, ImageError>>,
}

struct QuickLookRequestStart {
    callback_commands: mpsc::Sender<QuickLookCommand>,
    request_id: u64,
    source: PathBuf,
    destination: PathBuf,
    physical_max_pixels: u32,
    scale_milli: u16,
    cancelled: Arc<AtomicBool>,
}

fn run_quick_look_executor(
    receiver: mpsc::Receiver<QuickLookCommand>,
    callback_commands: mpsc::Sender<QuickLookCommand>,
) {
    // SAFETY: The shared generator is obtained and then used exclusively on
    // this dedicated worker thread. Objective-C callbacks only send plain Rust
    // completion values back through the command channel.
    let generator = unsafe { QLThumbnailGenerator::sharedGenerator() };
    let mut active = HashMap::<u64, ActiveQuickLookRequest>::new();

    while let Ok(command) = receiver.recv() {
        match command {
            QuickLookCommand::Generate {
                request_id,
                session_id,
                source,
                destination,
                physical_max_pixels,
                scale_milli,
                cancelled,
                responder,
            } => match start_quick_look_request(
                &generator,
                QuickLookRequestStart {
                    callback_commands: callback_commands.clone(),
                    request_id,
                    source,
                    destination,
                    physical_max_pixels,
                    scale_milli,
                    cancelled: Arc::clone(&cancelled),
                },
            ) {
                Ok(request) => {
                    active.insert(
                        request_id,
                        ActiveQuickLookRequest {
                            session_id,
                            request,
                            cancelled,
                            responder,
                        },
                    );
                }
                Err(error) => {
                    let _ = responder.send(Err(error));
                }
            },
            QuickLookCommand::CancelRequest { request_id } => {
                if let Some(active_request) = active.get(&request_id) {
                    cancel_active_request(&generator, active_request);
                }
            }
            QuickLookCommand::CancelSession {
                session_id,
                acknowledge,
            } => {
                for active_request in active
                    .values()
                    .filter(|request| request.session_id == session_id)
                {
                    cancel_active_request(&generator, active_request);
                }
                let _ = acknowledge.send(());
            }
            QuickLookCommand::Completed { request_id, result } => {
                if let Some(active_request) = active.remove(&request_id) {
                    let _ = active_request.responder.send(result);
                }
            }
            QuickLookCommand::Shutdown => {
                for active_request in active.values() {
                    cancel_active_request(&generator, active_request);
                }
                break;
            }
        }
    }
}

fn start_quick_look_request(
    generator: &QLThumbnailGenerator,
    start: QuickLookRequestStart,
) -> Result<Retained<QLThumbnailGenerationRequest>, ImageError> {
    let QuickLookRequestStart {
        callback_commands,
        request_id,
        source,
        destination,
        physical_max_pixels,
        scale_milli,
        cancelled,
    } = start;
    let source_string = source.to_str().ok_or_else(|| {
        ImageError::Io("Quick Look does not support this non-Unicode path".into())
    })?;
    let source_string = NSString::from_str(source_string);
    let source_url = NSURL::fileURLWithPath(&source_string);
    let scale = f64::from(scale_milli) / 1_000.0;
    let logical_max = f64::from(physical_max_pixels) / scale;

    // SAFETY: The URL is a local file URL, the requested size and scale are
    // finite positive values validated by the public adapter, and the request
    // is retained in the worker's active map until its callback completes.
    let request = unsafe {
        QLThumbnailGenerationRequest::initWithFileAtURL_size_scale_representationTypes(
            QLThumbnailGenerationRequest::alloc(),
            &source_url,
            CGSize {
                width: logical_max,
                height: logical_max,
            },
            scale,
            QLThumbnailGenerationRequestRepresentationTypes::Thumbnail,
        )
    };
    // SAFETY: Disabling icon mode requests the raw, undecorated thumbnail and
    // does not invalidate the retained request.
    unsafe { request.setIconMode(false) };

    let callback_cancelled = Arc::clone(&cancelled);
    let callback = RcBlock::new(
        move |representation: *mut QLThumbnailRepresentation, error: *mut NSError| {
            let result = finish_quick_look_request(
                representation,
                error,
                &destination,
                physical_max_pixels,
                &callback_cancelled,
            );
            let _ = callback_commands.send(QuickLookCommand::Completed { request_id, result });
        },
    );

    // SAFETY: The callback has the exact Objective-C block signature required
    // by Quick Look, captures only thread-safe Rust state, and Quick Look copies
    // it for the asynchronous operation before this call returns.
    unsafe {
        generator.generateBestRepresentationForRequest_completionHandler(&request, &callback)
    };
    Ok(request)
}

fn cancel_active_request(
    generator: &QLThumbnailGenerator,
    active_request: &ActiveQuickLookRequest,
) {
    active_request.cancelled.store(true, Ordering::Release);
    // SAFETY: The request belongs to this generator and stays retained in the
    // active map until the completion callback is processed.
    unsafe { generator.cancelRequest(&active_request.request) };
}

fn finish_quick_look_request(
    representation: *mut QLThumbnailRepresentation,
    error: *mut NSError,
    destination: &Path,
    physical_max_pixels: u32,
    cancelled: &AtomicBool,
) -> Result<RenderedDimensions, ImageError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(ImageError::Cancelled);
    }

    // SAFETY: Quick Look guarantees both raw pointers are valid for the
    // duration of this callback when non-null. They are never retained or
    // accessed after this function returns.
    let representation = unsafe { representation.as_ref() };
    let Some(representation) = representation else {
        // SAFETY: See the pointer lifetime guarantee above.
        let description = unsafe { error.as_ref() }
            .map(|error| error.localizedDescription().to_string())
            .unwrap_or_else(|| "no representation or NSError".into());
        return Err(ImageError::Io(format!("Quick Look failed: {description}")));
    };

    // SAFETY: The representation remains valid throughout the callback and
    // returns a retained Core Graphics image.
    let image = unsafe { representation.CGImage() };
    let width =
        u32::try_from(CGImage::width(Some(&image))).map_err(|_| ImageError::BudgetExceeded)?;
    let height =
        u32::try_from(CGImage::height(Some(&image))).map_err(|_| ImageError::BudgetExceeded)?;
    if width > physical_max_pixels || height > physical_max_pixels {
        return Err(ImageError::BudgetExceeded);
    }

    encode_png(&image, destination)?;
    if cancelled.load(Ordering::Acquire) {
        let _ = fs::remove_file(destination);
        return Err(ImageError::Cancelled);
    }

    Ok(RenderedDimensions { width, height })
}

#[cfg(test)]
mod tests {
    use super::{
        ImageIoRenderBackend, MAX_CONCURRENT_RENDERS, MacImagePort, QuickLookBackend,
        QuickLookCommand, QuickLookThumbnailBackend, QuickLookWorkerLifetime, RenderedDimensions,
    };
    use async_trait::async_trait;
    use std::{
        collections::HashMap,
        fs,
        future::Future,
        path::Path,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, AtomicUsize, Ordering},
            mpsc,
        },
        task::Poll,
        time::Duration,
    };
    use tokio::sync::{Notify, Semaphore};
    use viewer_application::{ImageError, ImagePort, ImageRequest, ImageRequestCancellation};
    use viewer_domain::{EntityId, ImageRequestId, SessionId, image::ImageRepresentationKind};
    use viewer_test_support::image_fixtures::{image_fixture, png_dimensions};

    struct FailingQuickLook;

    #[async_trait]
    impl QuickLookThumbnailBackend for FailingQuickLook {
        async fn thumbnail(
            &self,
            _request: &ImageRequest,
            _destination: &Path,
        ) -> Result<RenderedDimensions, ImageError> {
            Err(ImageError::Io("Quick Look failed".into()))
        }

        async fn cancel_session(&self, _session_id: SessionId) {}
    }

    struct SuccessfulImageIo;

    #[async_trait]
    impl ImageIoRenderBackend for SuccessfulImageIo {
        async fn probe(
            &self,
            _source: &Path,
        ) -> Result<viewer_domain::image::ImageProbe, ImageError> {
            unreachable!("probe is not used by this coordinator test")
        }

        async fn render(
            &self,
            _request: &ImageRequest,
            destination: &Path,
        ) -> Result<RenderedDimensions, ImageError> {
            fs::write(destination, b"mock PNG").unwrap();
            Ok(RenderedDimensions {
                width: 160,
                height: 120,
            })
        }
    }

    #[derive(Clone, Default)]
    struct BlockingQuickLook {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    #[derive(Clone, Default)]
    struct FailingAfterSessionCancellationQuickLook {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    #[derive(Clone, Default)]
    struct CountingImageIo {
        renders: Arc<AtomicUsize>,
    }

    #[derive(Clone)]
    struct BlockingImageIo {
        entered_total: Arc<AtomicUsize>,
        in_flight: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        release: Arc<Semaphore>,
    }

    #[derive(Default)]
    struct ControlledImageIo {
        requests: Mutex<HashMap<ImageRequestId, Arc<ControlledImageIoRequest>>>,
    }

    #[derive(Default)]
    struct ControlledImageIoRequest {
        started: Notify,
        release: Notify,
        destination: Mutex<Option<std::path::PathBuf>>,
    }

    impl ControlledImageIo {
        fn request(&self) -> (ImageRequest, Arc<ControlledImageIoRequest>) {
            let request = original_request();
            let control = Arc::new(ControlledImageIoRequest::default());
            self.requests
                .lock()
                .unwrap()
                .insert(request.request_id, Arc::clone(&control));
            (request, control)
        }
    }

    #[async_trait]
    impl ImageIoRenderBackend for Arc<ControlledImageIo> {
        async fn probe(
            &self,
            _source: &Path,
        ) -> Result<viewer_domain::image::ImageProbe, ImageError> {
            unreachable!("probe is not used by this coordinator test")
        }

        async fn render(
            &self,
            request: &ImageRequest,
            destination: &Path,
        ) -> Result<RenderedDimensions, ImageError> {
            let control = Arc::clone(
                self.requests
                    .lock()
                    .unwrap()
                    .get(&request.request_id)
                    .unwrap(),
            );
            *control.destination.lock().unwrap() = Some(destination.to_path_buf());
            control.started.notify_one();
            control.release.notified().await;
            fs::write(destination, b"mock PNG").unwrap();
            Ok(RenderedDimensions {
                width: 160,
                height: 120,
            })
        }
    }

    impl Default for BlockingImageIo {
        fn default() -> Self {
            Self {
                entered_total: Arc::new(AtomicUsize::new(0)),
                in_flight: Arc::new(AtomicUsize::new(0)),
                peak: Arc::new(AtomicUsize::new(0)),
                release: Arc::new(Semaphore::new(0)),
            }
        }
    }

    #[async_trait]
    impl ImageIoRenderBackend for BlockingImageIo {
        async fn probe(
            &self,
            _source: &Path,
        ) -> Result<viewer_domain::image::ImageProbe, ImageError> {
            unreachable!("probe is not used by this coordinator test")
        }

        async fn render(
            &self,
            _request: &ImageRequest,
            destination: &Path,
        ) -> Result<RenderedDimensions, ImageError> {
            self.entered_total.fetch_add(1, Ordering::SeqCst);
            let current = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(current, Ordering::SeqCst);
            self.release.acquire().await.unwrap().forget();
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            fs::write(destination, b"mock PNG").unwrap();
            Ok(RenderedDimensions {
                width: 160,
                height: 120,
            })
        }
    }

    #[async_trait]
    impl ImageIoRenderBackend for CountingImageIo {
        async fn probe(
            &self,
            _source: &Path,
        ) -> Result<viewer_domain::image::ImageProbe, ImageError> {
            unreachable!("probe is not used by this coordinator test")
        }

        async fn render(
            &self,
            _request: &ImageRequest,
            destination: &Path,
        ) -> Result<RenderedDimensions, ImageError> {
            self.renders.fetch_add(1, Ordering::SeqCst);
            fs::write(destination, b"unexpected fallback PNG").unwrap();
            Ok(RenderedDimensions {
                width: 160,
                height: 120,
            })
        }
    }

    #[async_trait]
    impl QuickLookThumbnailBackend for BlockingQuickLook {
        async fn thumbnail(
            &self,
            _request: &ImageRequest,
            destination: &Path,
        ) -> Result<RenderedDimensions, ImageError> {
            self.started.notify_one();
            self.release.notified().await;
            fs::write(destination, b"stale mock PNG").unwrap();
            Ok(RenderedDimensions {
                width: 160,
                height: 120,
            })
        }

        async fn cancel_session(&self, _session_id: SessionId) {
            self.release.notify_waiters();
        }
    }

    #[async_trait]
    impl QuickLookThumbnailBackend for FailingAfterSessionCancellationQuickLook {
        async fn thumbnail(
            &self,
            _request: &ImageRequest,
            _destination: &Path,
        ) -> Result<RenderedDimensions, ImageError> {
            self.started.notify_one();
            self.release.notified().await;
            Err(ImageError::Io(
                "Quick Look callback failed after cancellation".into(),
            ))
        }

        async fn cancel_session(&self, _session_id: SessionId) {}
    }

    fn thumbnail_request() -> ImageRequest {
        ImageRequest {
            request_id: ImageRequestId::new(),
            cancellation: ImageRequestCancellation::new(),
            session_id: SessionId::new(),
            entity_id: EntityId::new(),
            source: Path::new("tests/fixtures/images/srgb.jpg").to_path_buf(),
            kind: ImageRepresentationKind::Thumbnail {
                max_pixels: 256,
                scale_milli: 1_000,
            },
        }
    }

    fn original_request() -> ImageRequest {
        let mut request = thumbnail_request();
        request.kind = ImageRepresentationKind::Original100Percent;
        request
    }

    fn controlled_quick_look_backend(
        timeout: Duration,
    ) -> (QuickLookBackend, mpsc::Receiver<QuickLookCommand>) {
        let (commands, receiver) = mpsc::channel();
        (
            QuickLookBackend {
                commands: commands.clone(),
                next_request_id: Arc::new(AtomicU64::new(1)),
                pending: Arc::new(Mutex::new(HashMap::new())),
                _worker_lifetime: Arc::new(QuickLookWorkerLifetime { commands }),
                timeout,
            },
            receiver,
        )
    }

    async fn wait_for_entries(backend: &BlockingImageIo, count: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while backend.entered_total.load(Ordering::SeqCst) < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("expected renders to enter the backend");
    }

    #[tokio::test]
    async fn quick_look_failure_falls_back_to_image_io() {
        let cache = tempfile::tempdir().unwrap();
        let port = MacImagePort::with_backends(cache.path(), FailingQuickLook, SuccessfulImageIo);
        let artifact = port.render(thumbnail_request()).await.unwrap();
        assert_eq!(artifact.backend, viewer_application::ImageBackend::ImageIo);
        assert_eq!((artifact.width, artifact.height), (160, 120));
    }

    #[tokio::test]
    async fn cancel_session_rejects_pending_thumbnail() {
        let cache = tempfile::tempdir().unwrap();
        let quick_look = BlockingQuickLook::default();
        let port = Arc::new(MacImagePort::with_backends(
            cache.path(),
            quick_look.clone(),
            SuccessfulImageIo,
        ));
        let request = thumbnail_request();
        let session_id = request.session_id;
        let rendering = {
            let port = Arc::clone(&port);
            tokio::spawn(async move { port.render(request).await })
        };
        quick_look.started.notified().await;
        port.cancel_session(session_id).await;
        assert!(matches!(
            rendering.await.unwrap(),
            Err(ImageError::Cancelled)
        ));
        let session_directory = cache.path().join(session_id.to_string());
        assert_eq!(fs::read_dir(session_directory).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn cancelled_session_never_falls_back_to_image_io_or_recreates_its_cache() {
        let cache = tempfile::tempdir().unwrap();
        let quick_look = FailingAfterSessionCancellationQuickLook::default();
        let image_io = CountingImageIo::default();
        let port = Arc::new(MacImagePort::with_backends(
            cache.path(),
            quick_look.clone(),
            image_io.clone(),
        ));
        let request = thumbnail_request();
        let session_id = request.session_id;
        let rendering = {
            let port = Arc::clone(&port);
            tokio::spawn(async move { port.render(request).await })
        };
        quick_look.started.notified().await;

        port.cancel_session(session_id).await;
        fs::remove_dir_all(cache.path()).unwrap();
        quick_look.release.notify_one();
        assert!(matches!(
            rendering.await.unwrap(),
            Err(ImageError::Cancelled)
        ));

        assert_eq!(
            image_io.renders.load(Ordering::SeqCst),
            0,
            "a cancelled Quick Look failure must not enter the Image I/O fallback"
        );
        assert!(
            !cache.path().exists(),
            "cancelled fallback must not recreate the cleaned session cache"
        );
    }

    #[tokio::test]
    async fn real_quick_look_thumbnail_respects_requested_physical_pixels() {
        let output_directory = tempfile::tempdir().unwrap();
        let destination = output_directory.path().join("quick-look.png");
        let mut request = thumbnail_request();
        request.source = image_fixture("srgb.jpg");
        request.kind = ImageRepresentationKind::Thumbnail {
            max_pixels: 256,
            scale_milli: 2_000,
        };

        let dimensions = QuickLookBackend::new()
            .unwrap()
            .thumbnail(&request, &destination)
            .await
            .unwrap();

        assert!(dimensions.width <= 256 && dimensions.height <= 256);
        assert_eq!(
            png_dimensions(&destination).unwrap(),
            (dimensions.width, dimensions.height)
        );
    }

    #[tokio::test]
    async fn real_quick_look_rejects_an_already_cancelled_request() {
        let output_directory = tempfile::tempdir().unwrap();
        let destination = output_directory.path().join("cancelled-quick-look.png");
        let mut request = thumbnail_request();
        request.source = image_fixture("srgb.jpg");
        request.cancellation.cancel();

        let result = QuickLookBackend::new()
            .unwrap()
            .thumbnail(&request, &destination)
            .await;

        assert!(matches!(result, Err(ImageError::Cancelled)));
        assert!(!destination.exists());
    }

    #[tokio::test]
    async fn quick_look_request_cancellation_waits_for_worker_completion() {
        let output_directory = tempfile::tempdir().unwrap();
        let destination = output_directory.path().join("cancelled-quick-look.png");
        let (backend, commands) = controlled_quick_look_backend(Duration::from_secs(60));
        let request = thumbnail_request();
        let cancellation = request.cancellation.clone();
        let rendering = backend.thumbnail(&request, &destination);
        tokio::pin!(rendering);

        let first_poll =
            std::future::poll_fn(|context| Poll::Ready(rendering.as_mut().poll(context))).await;
        assert!(first_poll.is_pending());
        let (request_id, responder) = match commands.recv().unwrap() {
            QuickLookCommand::Generate {
                request_id,
                responder,
                ..
            } => (request_id, responder),
            _ => panic!("first command should generate the thumbnail"),
        };

        cancellation.cancel();
        let cancellation_poll =
            std::future::poll_fn(|context| Poll::Ready(rendering.as_mut().poll(context))).await;
        assert!(
            cancellation_poll.is_pending(),
            "cancellation must keep waiting for the native completion callback"
        );
        assert!(matches!(
            commands.recv().unwrap(),
            QuickLookCommand::CancelRequest {
                request_id: cancelled_id
            } if cancelled_id == request_id
        ));

        responder.send(Err(ImageError::Cancelled)).unwrap();
        let completed =
            std::future::poll_fn(|context| Poll::Ready(rendering.as_mut().poll(context))).await;
        assert!(matches!(completed, Poll::Ready(Err(ImageError::Cancelled))));
    }

    #[tokio::test]
    async fn quick_look_timeout_waits_for_worker_completion() {
        let output_directory = tempfile::tempdir().unwrap();
        let destination = output_directory.path().join("timed-out-quick-look.png");
        let (backend, commands) = controlled_quick_look_backend(Duration::ZERO);
        let request = thumbnail_request();
        let rendering = backend.thumbnail(&request, &destination);
        tokio::pin!(rendering);

        let first_poll =
            std::future::poll_fn(|context| Poll::Ready(rendering.as_mut().poll(context))).await;
        assert!(first_poll.is_pending());
        let (request_id, responder) = match commands.recv().unwrap() {
            QuickLookCommand::Generate {
                request_id,
                responder,
                ..
            } => (request_id, responder),
            _ => panic!("first command should generate the thumbnail"),
        };
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let timeout_poll =
                    std::future::poll_fn(|context| Poll::Ready(rendering.as_mut().poll(context)))
                        .await;
                assert!(
                    timeout_poll.is_pending(),
                    "timeout must keep waiting for the native completion callback"
                );
                match commands.try_recv() {
                    Ok(QuickLookCommand::CancelRequest {
                        request_id: cancelled_id,
                    }) => {
                        assert_eq!(cancelled_id, request_id);
                        break;
                    }
                    Ok(_) => panic!("timeout should only send request cancellation"),
                    Err(mpsc::TryRecvError::Empty) => tokio::task::yield_now().await,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        panic!("controlled Quick Look executor disconnected")
                    }
                }
            }
        })
        .await
        .expect("timeout should request native cancellation");

        responder.send(Err(ImageError::Cancelled)).unwrap();
        let completed =
            std::future::poll_fn(|context| Poll::Ready(rendering.as_mut().poll(context))).await;
        assert!(
            matches!(completed, Poll::Ready(Err(ImageError::Io(message))) if message.contains("timed out"))
        );
    }

    #[tokio::test]
    async fn render_concurrency_never_exceeds_four_native_backends() {
        let cache = tempfile::tempdir().unwrap();
        let image_io = BlockingImageIo::default();
        let port = Arc::new(MacImagePort::with_backends(
            cache.path(),
            FailingQuickLook,
            image_io.clone(),
        ));
        let renders = (0..8)
            .map(|_| {
                let port = Arc::clone(&port);
                tokio::spawn(async move { port.render(original_request()).await })
            })
            .collect::<Vec<_>>();

        wait_for_entries(&image_io, 4).await;
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(image_io.peak.load(Ordering::SeqCst), 4);

        image_io.release.add_permits(8);
        for render in renders {
            render.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn two_ports_share_one_render_limit_across_all_native_jobs() {
        let first_cache = tempfile::tempdir().unwrap();
        let second_cache = tempfile::tempdir().unwrap();
        let image_io = BlockingImageIo::default();
        let shared_limit = Arc::new(Semaphore::new(4));
        let first_port = Arc::new(MacImagePort::with_backends_and_render_limit(
            first_cache.path(),
            FailingQuickLook,
            image_io.clone(),
            Arc::clone(&shared_limit),
        ));
        let second_port = Arc::new(MacImagePort::with_backends_and_render_limit(
            second_cache.path(),
            FailingQuickLook,
            image_io.clone(),
            Arc::clone(&shared_limit),
        ));
        let renders = (0..8)
            .map(|index| {
                let port = if index % 2 == 0 {
                    Arc::clone(&first_port)
                } else {
                    Arc::clone(&second_port)
                };
                tokio::spawn(async move { port.render(original_request()).await })
            })
            .collect::<Vec<_>>();

        wait_for_entries(&image_io, 4).await;
        assert_eq!(image_io.peak.load(Ordering::SeqCst), 4);
        assert_eq!(shared_limit.available_permits(), 0);

        image_io.release.add_permits(8);
        for render in renders {
            render.await.unwrap().unwrap();
        }
        assert_eq!(image_io.peak.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn production_ports_use_the_process_global_render_limit() {
        let first_cache = tempfile::tempdir().unwrap();
        let second_cache = tempfile::tempdir().unwrap();
        let first_port = MacImagePort::new(first_cache.path()).unwrap();
        let second_port = MacImagePort::new(second_cache.path()).unwrap();

        assert!(Arc::ptr_eq(
            &first_port.render_limit,
            &second_port.render_limit
        ));
    }

    #[tokio::test]
    async fn cancelling_a_request_waiting_for_a_render_permit_skips_the_backend() {
        let cache = tempfile::tempdir().unwrap();
        let image_io = BlockingImageIo::default();
        let port = Arc::new(MacImagePort::with_backends(
            cache.path(),
            FailingQuickLook,
            image_io.clone(),
        ));
        let blockers = (0..4)
            .map(|_| {
                let port = Arc::clone(&port);
                tokio::spawn(async move { port.render(original_request()).await })
            })
            .collect::<Vec<_>>();
        wait_for_entries(&image_io, 4).await;
        let cancelled_request = original_request();
        let cancellation = cancelled_request.cancellation.clone();
        let waiting = {
            let port = Arc::clone(&port);
            tokio::spawn(async move { port.render(cancelled_request).await })
        };
        tokio::task::yield_now().await;

        cancellation.cancel();

        let result = tokio::time::timeout(Duration::from_millis(100), waiting)
            .await
            .expect("cancelled permit waiter should finish")
            .unwrap();
        assert!(matches!(result, Err(ImageError::Cancelled)));
        assert_eq!(image_io.entered_total.load(Ordering::SeqCst), 4);

        image_io.release.add_permits(4);
        for blocker in blockers {
            blocker.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn dropping_an_outer_render_keeps_its_native_permit_until_backend_cleanup() {
        let cache = tempfile::tempdir().unwrap();
        let image_io = Arc::new(ControlledImageIo::default());
        let port = Arc::new(MacImagePort::with_backends(
            cache.path(),
            FailingQuickLook,
            Arc::clone(&image_io),
        ));
        let requests = (0..5).map(|_| image_io.request()).collect::<Vec<_>>();
        let mut renders = requests
            .iter()
            .take(4)
            .map(|(request, _)| {
                let port = Arc::clone(&port);
                let request = request.clone();
                tokio::spawn(async move { port.render(request).await })
            })
            .collect::<Vec<_>>();
        for (_, control) in requests.iter().take(4) {
            control.started.notified().await;
        }
        assert_eq!(port.render_limit.available_permits(), 0);

        let cancelled = renders.remove(0);
        cancelled.abort();
        assert!(cancelled.await.unwrap_err().is_cancelled());

        assert_eq!(
            port.render_limit.available_permits(),
            0,
            "outer future cancellation must not release a live native job permit"
        );
        let fifth = {
            let port = Arc::clone(&port);
            let request = requests[4].0.clone();
            tokio::spawn(async move { port.render(request).await })
        };
        requests[0].1.release.notify_one();
        requests[4].1.started.notified().await;
        let cancelled_destination = requests[0].1.destination.lock().unwrap().clone().unwrap();
        assert!(!cancelled_destination.exists());

        for (_, control) in requests.iter().skip(1) {
            control.release.notify_one();
        }
        for render in renders {
            render.await.unwrap().unwrap();
        }
        fifth.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn dropping_a_completed_but_undelivered_render_removes_its_artifact() {
        let cache = tempfile::tempdir().unwrap();
        let image_io = Arc::new(ControlledImageIo::default());
        let port =
            MacImagePort::with_backends(cache.path(), FailingQuickLook, Arc::clone(&image_io));
        let (request, control) = image_io.request();
        let destination;
        {
            let rendering = port.render(request);
            tokio::pin!(rendering);
            let first_poll =
                std::future::poll_fn(|context| Poll::Ready(rendering.as_mut().poll(context))).await;
            assert!(first_poll.is_pending());
            control.started.notified().await;
            destination = control.destination.lock().unwrap().clone().unwrap();
            control.release.notify_one();
            tokio::time::timeout(Duration::from_secs(1), async {
                while port.render_limit.available_permits() != MAX_CONCURRENT_RENDERS {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("owned render should complete without polling its outer future");
            assert!(destination.exists());
        }

        assert!(
            !destination.exists(),
            "dropping the undelivered outer future must remove its generated artifact"
        );
    }
}
