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
use std::{path::Path, sync::Arc};
use viewer_application::{
    MAX_REVIEW_ARTIFACT_BYTES, NumberedImageAnnotation, PreparedReviewAsset,
    REVIEW_ANNOTATION_MAX_EDGE, ReviewArtifactError as Error, ReviewTaskCancellation,
    review_evidence::*,
    review_workspace::{EvidenceAnnotation, EvidenceRef, PreparedEvidenceFile},
};
use viewer_domain::{image::ImageFormat, review::ReviewMedia};

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
}
impl MacReviewEvidenceRenderer {
    pub fn new(root: &Path) -> Result<Self, Error> {
        Ok(Self {
            root: Root::open(root)?,
            limit: MAX_REVIEW_ARTIFACT_BYTES,
            hook: Arc::new(|_| {}),
        })
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
        if asset.failure.is_some()
            || asset.asset.evidence.blake3.is_none()
            || asset.asset.source_entity_id != Some(asset.entity_id)
        {
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
        scratch.encode("base.png", &image)?;
        let png = scratch.read("base.png", limit)?;
        let reference = reference(&image, &png)?;
        // Encoding can take time too; refuse a source changed during that interval.
        source.verify(&asset)?;
        check(&cancellation, &hook, Checkpoint::Return)?;
        BoundReviewImage::from_verified_png(asset.asset, reference, EvidenceRole::Base, png)
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
            scratch.encode("annotated.png", &rendered)?;
            let png = scratch.read("annotated.png", limit)?;
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
fn reference(image: &CGImage, png: &[u8]) -> Result<EvidenceRef, Error> {
    Ok(EvidenceRef {
        blake3: *blake3::hash(png).as_bytes(),
        size_bytes: png.len() as u64,
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
    async fn capture_base(
        &self,
        asset: PreparedReviewAsset,
        cancellation: ReviewTaskCancellation,
    ) -> Result<BoundReviewImage, Error> {
        let root = self.root.clone();
        let hook = self.hook.clone();
        let limit = self.limit;
        tokio::task::spawn_blocking(move || Self::capture(root, limit, hook, asset, cancellation))
            .await
            .map_err(|_| Error::Unavailable)?
    }
    async fn render(&self, request: ReviewEvidenceRequest) -> Result<ReviewEvidenceResult, Error> {
        let root = self.root.clone();
        let hook = self.hook.clone();
        let limit = self.limit;
        tokio::task::spawn_blocking(move || Self::render_sync(root, limit, hook, request))
            .await
            .map_err(|_| Error::Unavailable)?
    }
}

#[cfg(test)]
mod tests;
