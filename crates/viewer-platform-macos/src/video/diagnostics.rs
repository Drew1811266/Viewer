use serde::Serialize;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static ACTIVE_CLIENTS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_RENDER_CONTEXTS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_SURFACES: AtomicUsize = AtomicUsize::new(0);
static RENDERED_FRAMES: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRenderDiagnostics {
    pub hwdec: String,
    pub video_output: String,
    pub active_clients: usize,
    pub active_render_contexts: usize,
    pub active_surfaces: usize,
    pub rendered_frames: u64,
}

#[derive(Debug, Default)]
pub struct VideoDiagnosticsCounters;

impl VideoDiagnosticsCounters {
    pub fn snapshot(
        &self,
        hwdec: impl Into<String>,
        video_output: impl Into<String>,
    ) -> VideoRenderDiagnostics {
        VideoRenderDiagnostics::snapshot(hwdec, video_output)
    }
}

impl VideoRenderDiagnostics {
    pub fn snapshot(hwdec: impl Into<String>, video_output: impl Into<String>) -> Self {
        Self {
            hwdec: hwdec.into(),
            video_output: video_output.into(),
            active_clients: ACTIVE_CLIENTS.load(Ordering::Acquire),
            active_render_contexts: ACTIVE_RENDER_CONTEXTS.load(Ordering::Acquire),
            active_surfaces: ACTIVE_SURFACES.load(Ordering::Acquire),
            rendered_frames: RENDERED_FRAMES.load(Ordering::Acquire),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ResourceKind {
    Client,
    RenderContext,
    Surface,
}

pub(crate) struct ResourceLease(ResourceKind);

impl ResourceLease {
    pub(crate) fn acquire(kind: ResourceKind) -> Self {
        counter(kind).fetch_add(1, Ordering::AcqRel);
        Self(kind)
    }
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        counter(self.0).fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) fn record_rendered_frame() {
    RENDERED_FRAMES.fetch_add(1, Ordering::AcqRel);
}

fn counter(kind: ResourceKind) -> &'static AtomicUsize {
    match kind {
        ResourceKind::Client => &ACTIVE_CLIENTS,
        ResourceKind::RenderContext => &ACTIVE_RENDER_CONTEXTS,
        ResourceKind::Surface => &ACTIVE_SURFACES,
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourceKind, ResourceLease, VideoRenderDiagnostics, record_rendered_frame};

    #[test]
    fn resource_counts_return_to_baseline_when_leases_drop() {
        let baseline = VideoRenderDiagnostics::snapshot("", "");
        {
            let _client = ResourceLease::acquire(ResourceKind::Client);
            let _context = ResourceLease::acquire(ResourceKind::RenderContext);
            let _surface = ResourceLease::acquire(ResourceKind::Surface);
            let active = VideoRenderDiagnostics::snapshot("videotoolbox", "libmpv");
            assert_eq!(active.active_clients, baseline.active_clients + 1);
            assert_eq!(
                active.active_render_contexts,
                baseline.active_render_contexts + 1
            );
            assert_eq!(active.active_surfaces, baseline.active_surfaces + 1);
        }
        let released = VideoRenderDiagnostics::snapshot("", "");
        assert_eq!(released.active_clients, baseline.active_clients);
        assert_eq!(
            released.active_render_contexts,
            baseline.active_render_contexts
        );
        assert_eq!(released.active_surfaces, baseline.active_surfaces);
    }

    #[test]
    fn rendered_frame_count_is_monotonic() {
        let before = VideoRenderDiagnostics::snapshot("", "").rendered_frames;
        record_rendered_frame();
        assert_eq!(
            VideoRenderDiagnostics::snapshot("", "").rendered_frames,
            before + 1
        );
    }
}
