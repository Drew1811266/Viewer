mod scratch;
mod source;
use super::{
    image_io::{
        open_image_bytes, open_image_source, oriented_dimensions, probe_image_source,
        thumbnail_from_image_source,
    },
    review_annotation::{draw_annotations, map_image_error},
};
use async_trait::async_trait;
use objc2_core_graphics::CGImage;
use scratch::{Root, Scratch};
use source::CapturedSource;
use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::Semaphore;
use viewer_application::{
    MAX_REVIEW_ARTIFACT_BYTES, NumberedImageAnnotation, PreparedReviewAsset,
    REVIEW_ANNOTATION_MAX_EDGE, ReviewArtifactError as Error, ReviewTaskCancellation,
    review_evidence::*,
    review_workspace::{EvidenceAnnotation, EvidenceRef, PreparedEvidenceFile},
};
use viewer_domain::{AssetVersionId, image::ImageFormat, review::ReviewMedia};

const MAX_PREWARMED_BASES: usize = 128;

fn image_encode_permits() -> Arc<Semaphore> {
    static PERMITS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    PERMITS.get_or_init(|| Arc::new(Semaphore::new(2))).clone()
}

#[derive(Default)]
struct BaseEvidenceCache {
    entries: HashMap<AssetVersionId, BoundReviewImage>,
    order: VecDeque<AssetVersionId>,
    retained_bytes: u64,
}

impl BaseEvidenceCache {
    fn get(&mut self, asset: &PreparedReviewAsset, limit: u64) -> Option<BoundReviewImage> {
        let id = asset.asset.id;
        let image = self
            .entries
            .get(&id)
            .filter(|image| image.asset() == &asset.asset && image.reference().size_bytes <= limit)
            .cloned();
        if image.is_some() {
            self.order.retain(|candidate| *candidate != id);
            self.order.push_back(id);
        } else if self.entries.contains_key(&id) {
            self.remove(id);
        }
        image
    }

    fn insert(&mut self, image: BoundReviewImage) {
        let id = image.asset().id;
        self.remove(id);
        self.retained_bytes = self
            .retained_bytes
            .saturating_add(image.reference().size_bytes);
        self.entries.insert(id, image);
        self.order.push_back(id);
        while self.entries.len() > MAX_PREWARMED_BASES
            || self.retained_bytes > MAX_REVIEW_ARTIFACT_BYTES
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.retained_bytes = self
                    .retained_bytes
                    .saturating_sub(removed.reference().size_bytes);
            }
        }
    }

    fn remove(&mut self, id: AssetVersionId) {
        self.order.retain(|candidate| *candidate != id);
        if let Some(removed) = self.entries.remove(&id) {
            self.retained_bytes = self
                .retained_bytes
                .saturating_sub(removed.reference().size_bytes);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Checkpoint {
    CaptureOpened,
    CaptureCopied,
    CaptureDecoded,
    RenderDecoded,
    RenderDrawn,
    RenderEncoded,
    Return,
}
type Hook = Arc<dyn Fn(Checkpoint) + Send + Sync>;
pub struct MacReviewEvidenceRenderer {
    root: Arc<Root>,
    limit: u64,
    hook: Hook,
    base_cache: Mutex<BaseEvidenceCache>,
}
impl MacReviewEvidenceRenderer {
    pub fn new(root: &Path) -> Result<Self, Error> {
        Ok(Self {
            root: Root::open(root)?,
            limit: MAX_REVIEW_ARTIFACT_BYTES,
            hook: Arc::new(|_| {}),
            base_cache: Mutex::new(BaseEvidenceCache::default()),
        })
    }

    fn cached_base(&self, asset: &PreparedReviewAsset) -> Result<Option<BoundReviewImage>, Error> {
        self.base_cache
            .lock()
            .map_err(|_| Error::Unavailable)
            .map(|mut cache| cache.get(asset, self.limit))
    }

    fn retain_base(&self, image: BoundReviewImage) -> Result<(), Error> {
        self.base_cache
            .lock()
            .map_err(|_| Error::Unavailable)?
            .insert(image);
        Ok(())
    }

    async fn capture_fresh(
        &self,
        asset: PreparedReviewAsset,
        cancellation: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, Error> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let permit = image_encode_permits()
            .acquire_owned()
            .await
            .map_err(|_| Error::Unavailable)?;
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let root = self.root.clone();
        let hook = self.hook.clone();
        let limit = self.limit;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            Self::capture(root, limit, hook, asset, cancellation)
        })
        .await
        .map_err(|_| Error::Unavailable)?
    }
    fn capture(
        root: Arc<Root>,
        limit: u64,
        hook: Hook,
        asset: PreparedReviewAsset,
        cancellation: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, Error> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if asset.failure.is_some() || asset.asset.evidence.blake3.is_none() {
            return Err(Error::InvalidRequest);
        }
        let expected = match asset.asset.media {
            ReviewMedia::Image {
                width: Some(w),
                height: Some(h),
            } if w > 0 && h > 0 => (w, h),
            _ => return Err(Error::InvalidRequest),
        };
        let scratch = Scratch::new(root)?;
        // Copy/hash in bounded chunks. ImageIO only sees the private copy, never
        // reopens the source path after the user may have overwritten it.
        let source = CapturedSource::copy(&asset, &scratch, &cancellation, &hook)?;
        let image_source =
            open_image_source(&scratch.path.join("source.dat")).map_err(map_image_error)?;
        let probe = probe_image_source(&image_source).map_err(map_image_error)?;
        if !(1..=8).contains(&probe.orientation) || oriented_dimensions(&probe) != expected {
            return Err(Error::SourceChanged);
        }
        let image = thumbnail_from_image_source(&image_source, REVIEW_ANNOTATION_MAX_EDGE)
            .map_err(map_image_error)?;
        check(&cancellation, &hook, Checkpoint::CaptureDecoded)?;
        source.verify(&asset)?;
        let png = scratch.encode("base.png", &image, limit)?;
        let reference = reference(&image, &png)?;
        // Encoding can take time too; refuse a source changed during that interval.
        source.verify(&asset)?;
        check(&cancellation, &hook, Checkpoint::Return)?;
        BoundReviewImage::from_verified_png(asset.asset, reference, EvidenceRole::Base, png.bytes)
    }
    fn render_sync(
        root: Arc<Root>,
        limit: u64,
        hook: Hook,
        request: ReviewEvidenceRequest,
    ) -> Result<ReviewEvidenceResult, Error> {
        request.validate()?;
        check(&request.cancellation, &hook, Checkpoint::RenderDecoded)?;
        if *blake3::hash(request.base.png()).as_bytes() != request.base.blake3() {
            return Err(Error::InvalidRequest);
        }
        let image_source = open_image_bytes(request.base.png()).map_err(map_image_error)?;
        let probe = probe_image_source(&image_source).map_err(map_image_error)?;
        let base_ref = request.base.reference().clone();
        if probe.format != ImageFormat::Png
            || probe.orientation != 1
            || (probe.width, probe.height) != (base_ref.width, base_ref.height)
        {
            return Err(Error::InvalidRequest);
        }
        let image = thumbnail_from_image_source(&image_source, base_ref.width.max(base_ref.height))
            .map_err(map_image_error)?;
        let scratch = Scratch::new(root)?;
        let base_path = scratch.write("base.png", request.base.png())?;
        let mut files = vec![PreparedEvidenceFile {
            path: base_path,
            reference: base_ref.clone(),
        }];
        let mut annotated_ref = None;
        if !request.annotations.is_empty() {
            // Reuse only the native geometry/marker primitive. Legacy validation
            // requires unique FeedbackId; v3 deliberately permits multiple targets.
            let annotations = request
                .annotations
                .iter()
                .map(|a| NumberedImageAnnotation {
                    ordinal: a.ordinal,
                    feedback_id: a.key.feedback_id,
                    anchor: a.anchor.clone(),
                })
                .collect::<Vec<_>>();
            let rendered = draw_annotations(&image, base_ref.width, base_ref.height, &annotations)?;
            check(&request.cancellation, &hook, Checkpoint::RenderDrawn)?;
            let png = scratch.encode("annotated.png", &rendered, limit)?;
            let reference = reference(&rendered, &png)?;
            files.push(PreparedEvidenceFile {
                path: scratch.path.join("annotated.png"),
                reference: reference.clone(),
            });
            annotated_ref = Some(reference);
        }
        check(&request.cancellation, &hook, Checkpoint::RenderEncoded)?;
        scratch.verify()?;
        let annotations = request
            .annotations
            .iter()
            .map(|a| EvidenceAnnotation {
                ordinal: a.ordinal,
                key: a.key,
            })
            .collect();
        check(&request.cancellation, &hook, Checkpoint::Return)?;
        Ok(ReviewEvidenceResult {
            base_ref,
            annotated_ref,
            annotations,
            staging: Arc::new(Staging {
                _scratch: scratch,
                files,
            }),
        })
    }
}
struct Staging {
    _scratch: Scratch,
    files: Vec<PreparedEvidenceFile>,
}
impl ReviewEvidenceStaging for Staging {
    fn files(&self) -> &[PreparedEvidenceFile] {
        &self.files
    }
}
fn reference(image: &CGImage, png: &super::encode::EncodedPng) -> Result<EvidenceRef, Error> {
    Ok(EvidenceRef {
        blake3: png.digest,
        size_bytes: png.size_bytes,
        width: CGImage::width(Some(image))
            .try_into()
            .map_err(|_| Error::LimitExceeded)?,
        height: CGImage::height(Some(image))
            .try_into()
            .map_err(|_| Error::LimitExceeded)?,
    })
}
fn check(
    cancellation: &ReviewTaskCancellation,
    hook: &Hook,
    point: Checkpoint,
) -> Result<(), Error> {
    hook(point);
    if cancellation.is_cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
#[async_trait]
impl ReviewEvidencePort for MacReviewEvidenceRenderer {
    async fn prewarm_base_evidence(
        &self,
        asset: PreparedReviewAsset,
        cancellation: ReviewTaskCancellation,
    ) -> Result<(), Error> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if self.cached_base(&asset)?.is_none() {
            let image = self.capture_fresh(asset, cancellation).await?;
            self.retain_base(image)?;
        }
        Ok(())
    }

    async fn capture_prewarmed_base(
        &self,
        asset: PreparedReviewAsset,
        cancellation: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, Error> {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if let Some(cached) = self.cached_base(&asset)? {
            return Ok(cached);
        }
        self.capture_fresh(asset, cancellation).await
    }

    async fn capture_base(
        &self,
        asset: PreparedReviewAsset,
        cancellation: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, Error> {
        self.capture_fresh(asset, cancellation).await
    }
    async fn render(&self, request: ReviewEvidenceRequest) -> Result<ReviewEvidenceResult, Error> {
        if request.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let permit = image_encode_permits()
            .acquire_owned()
            .await
            .map_err(|_| Error::Unavailable)?;
        if request.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let root = self.root.clone();
        let hook = self.hook.clone();
        let limit = self.limit;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            Self::render_sync(root, limit, hook, request)
        })
        .await
        .map_err(|_| Error::Unavailable)?
    }
}

#[cfg(test)]
mod tests;
