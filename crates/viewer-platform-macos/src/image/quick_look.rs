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
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use tokio::sync::{RwLock, Semaphore, oneshot};
use viewer_application::{ImageArtifact, ImageBackend, ImageError, ImagePort, ImageRequest};
use viewer_domain::{SessionId, image::ImageProbe};

const QUICK_LOOK_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONCURRENT_RENDERS: usize = 4;

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
    quick_look: Q,
    image_io: I,
    cache_root: PathBuf,
    cancelled_sessions: RwLock<HashSet<SessionId>>,
    render_limit: Semaphore,
}

impl MacImagePort<QuickLookBackend, ImageIoBackend> {
    pub fn new(cache_root: impl Into<PathBuf>) -> Result<Self, ImageError> {
        Ok(Self::with_backends(
            cache_root,
            QuickLookBackend::new()?,
            ImageIoBackend::default(),
        ))
    }
}

impl<Q, I> MacImagePort<Q, I> {
    pub fn with_backends(cache_root: impl Into<PathBuf>, quick_look: Q, image_io: I) -> Self {
        Self {
            quick_look,
            image_io,
            cache_root: cache_root.into(),
            cancelled_sessions: RwLock::new(HashSet::new()),
            render_limit: Semaphore::new(MAX_CONCURRENT_RENDERS),
        }
    }

    fn artifact_destination(
        &self,
        request: &ImageRequest,
        backend: ImageBackend,
    ) -> Result<PathBuf, ImageError> {
        let session_directory = self.cache_root.join(request.session_id.to_string());
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

    async fn is_cancelled(&self, session_id: SessionId) -> bool {
        self.cancelled_sessions.read().await.contains(&session_id)
    }
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
        if request.cancellation.is_cancelled() || self.is_cancelled(request.session_id).await {
            return Err(ImageError::Cancelled);
        }
        let _permit = tokio::select! {
            biased;
            _ = request.cancellation.cancelled() => return Err(ImageError::Cancelled),
            permit = self.render_limit.acquire() => {
                permit.map_err(|_| ImageError::Io("image render scheduler is unavailable".into()))?
            }
        };
        if request.cancellation.is_cancelled() || self.is_cancelled(request.session_id).await {
            return Err(ImageError::Cancelled);
        }

        let (destination, dimensions, backend) = if matches!(
            request.kind,
            viewer_domain::image::ImageRepresentationKind::Thumbnail { .. }
        ) {
            let quick_look_destination =
                self.artifact_destination(&request, ImageBackend::QuickLook)?;
            match self
                .quick_look
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
                    let image_io_destination =
                        self.artifact_destination(&request, ImageBackend::ImageIo)?;
                    let dimensions = self
                        .image_io
                        .render(&request, &image_io_destination)
                        .await?;
                    (image_io_destination, dimensions, ImageBackend::ImageIo)
                }
            }
        } else {
            let image_io_destination =
                self.artifact_destination(&request, ImageBackend::ImageIo)?;
            let dimensions = self
                .image_io
                .render(&request, &image_io_destination)
                .await?;
            (image_io_destination, dimensions, ImageBackend::ImageIo)
        };

        if request.cancellation.is_cancelled() || self.is_cancelled(request.session_id).await {
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

        let result = tokio::select! {
            biased;
            _ = request.cancellation.cancelled() => {
                cancelled.store(true, Ordering::Release);
                let _ = self.commands.send(QuickLookCommand::CancelRequest { request_id });
                Err(ImageError::Cancelled)
            }
            result = tokio::time::timeout(self.timeout, response) => match result {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err(ImageError::Io(
                    "Quick Look worker ended before responding".into(),
                )),
                Err(_) => {
                    cancelled.store(true, Ordering::Release);
                    let _ = self
                        .commands
                        .send(QuickLookCommand::CancelRequest { request_id });
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
        ImageIoRenderBackend, MacImagePort, QuickLookBackend, QuickLookThumbnailBackend,
        RenderedDimensions,
    };
    use async_trait::async_trait;
    use std::{
        fs,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
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

    #[derive(Clone)]
    struct BlockingImageIo {
        entered_total: Arc<AtomicUsize>,
        in_flight: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        release: Arc<Semaphore>,
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
}
