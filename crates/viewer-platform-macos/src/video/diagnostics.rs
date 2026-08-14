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
    pub mistimed_frames: u64,
    pub decoder_dropped_frames: u64,
    pub interaction: VideoInteractionDiagnostics,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInteractionDiagnostics {
    pub geometry_published: u64,
    pub geometry_applied: u64,
    pub geometry_replaced: u64,
    pub preview_published: u64,
    pub preview_issued: u64,
    pub commit_issued: u64,
    pub stale_completions_rejected: u64,
    pub latest_geometry_sequence: u64,
    pub latest_seek_request_id: u64,
}

#[derive(Debug, Default)]
pub struct VideoDiagnosticsCounters {
    geometry_published: AtomicU64,
    geometry_applied: AtomicU64,
    geometry_replaced: AtomicU64,
    preview_published: AtomicU64,
    preview_issued: AtomicU64,
    commit_issued: AtomicU64,
    stale_completions_rejected: AtomicU64,
    latest_geometry_sequence: AtomicU64,
    latest_seek_request_id: AtomicU64,
}

impl VideoDiagnosticsCounters {
    pub fn snapshot(
        &self,
        hwdec: impl Into<String>,
        video_output: impl Into<String>,
    ) -> VideoRenderDiagnostics {
        let mut snapshot = VideoRenderDiagnostics::snapshot(hwdec, video_output);
        snapshot.interaction = self.interaction_snapshot();
        snapshot
    }

    pub(crate) fn record_geometry_publication(&self, sequence: u64, replaced: bool) {
        self.geometry_published.fetch_add(1, Ordering::AcqRel);
        if replaced {
            self.geometry_replaced.fetch_add(1, Ordering::AcqRel);
        }
        self.latest_geometry_sequence
            .fetch_max(sequence, Ordering::AcqRel);
    }

    pub(crate) fn record_geometry_application(&self) {
        self.geometry_applied.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn record_preview_publication(&self, request_id: u64, _replaced: bool) {
        self.preview_published.fetch_add(1, Ordering::AcqRel);
        self.latest_seek_request_id
            .fetch_max(request_id, Ordering::AcqRel);
    }

    pub(crate) fn record_commit_publication(&self, request_id: u64, _replaced: bool) {
        self.latest_seek_request_id
            .fetch_max(request_id, Ordering::AcqRel);
    }

    pub(crate) fn record_preview_issue(&self) {
        self.preview_issued.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn record_commit_issue(&self) {
        self.commit_issued.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn record_stale_completion_rejection(&self) {
        self.stale_completions_rejected
            .fetch_add(1, Ordering::AcqRel);
    }

    pub fn interaction_snapshot(&self) -> VideoInteractionDiagnostics {
        VideoInteractionDiagnostics {
            geometry_published: self.geometry_published.load(Ordering::Acquire),
            geometry_applied: self.geometry_applied.load(Ordering::Acquire),
            geometry_replaced: self.geometry_replaced.load(Ordering::Acquire),
            preview_published: self.preview_published.load(Ordering::Acquire),
            preview_issued: self.preview_issued.load(Ordering::Acquire),
            commit_issued: self.commit_issued.load(Ordering::Acquire),
            stale_completions_rejected: self.stale_completions_rejected.load(Ordering::Acquire),
            latest_geometry_sequence: self.latest_geometry_sequence.load(Ordering::Acquire),
            latest_seek_request_id: self.latest_seek_request_id.load(Ordering::Acquire),
        }
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
            mistimed_frames: 0,
            decoder_dropped_frames: 0,
            interaction: VideoInteractionDiagnostics::default(),
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
    use super::{
        ResourceKind, ResourceLease, VideoDiagnosticsCounters, VideoRenderDiagnostics,
        record_rendered_frame,
    };

    #[test]
    fn interaction_metrics_report_bounded_work() {
        let counters = VideoDiagnosticsCounters::default();
        for sequence in 1..=120 {
            counters.record_geometry_publication(sequence, sequence > 1);
        }
        counters.record_geometry_application();
        for request_id in 1..=120 {
            counters.record_preview_publication(request_id, request_id > 1);
        }
        counters.record_preview_issue();
        counters.record_commit_publication(121, true);
        counters.record_commit_issue();

        let metrics = counters.interaction_snapshot();
        assert_eq!(metrics.geometry_published, 120);
        assert_eq!(metrics.geometry_applied, 1);
        assert_eq!(metrics.geometry_replaced, 119);
        assert_eq!(metrics.preview_published, 120);
        assert_eq!(metrics.preview_issued, 1);
        assert_eq!(metrics.commit_issued, 1);
        assert_eq!(metrics.latest_geometry_sequence, 120);
        assert_eq!(metrics.latest_seek_request_id, 121);
    }

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
