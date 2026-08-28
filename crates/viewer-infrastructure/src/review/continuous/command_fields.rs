use serde::Serialize;
use std::io::{self, Write};
use viewer_application::review_workspace::ReviewWorkspaceError;
use viewer_domain::review::{FeedbackAnchor, continuous::*};

pub(super) struct Encoder {
    hash: blake3::Hasher,
    bytes: usize,
    limited: bool,
}
impl Encoder {
    pub fn new() -> Self {
        Self {
            hash: blake3::Hasher::new(),
            bytes: 0,
            limited: false,
        }
    }
    pub fn finish(self) -> [u8; 32] {
        *self.hash.finalize().as_bytes()
    }
    pub fn field(&mut self, value: &impl Serialize) -> Result<(), ReviewWorkspaceError> {
        serde_json::to_writer(&mut *self, value).map_err(|_| {
            ReviewWorkspaceError::Domain(if self.limited {
                ContinuousReviewError::LimitExceeded
            } else {
                ContinuousReviewError::InvalidData
            })
        })?;
        self.write_all(&[0])
            .map_err(|_| ReviewWorkspaceError::Domain(ContinuousReviewError::LimitExceeded))
    }
    pub fn limit_targets(&self, count: usize) -> Result<(), ReviewWorkspaceError> {
        if count > 100_000_000 {
            Err(ContinuousReviewError::LimitExceeded.into())
        } else {
            Ok(())
        }
    }
    pub fn key(&mut self, key: &TargetVersionKey) -> Result<(), ReviewWorkspaceError> {
        self.field(&(
            key.feedback_id,
            key.text_revision_id,
            key.target_id,
            key.target_revision_id,
        ))
    }
    pub fn keys(&mut self, keys: &[TargetVersionKey]) -> Result<(), ReviewWorkspaceError> {
        self.limit_targets(keys.len())?;
        self.field(&keys.len())?;
        for key in keys {
            self.key(key)?;
        }
        Ok(())
    }
    pub fn snapshot(&mut self, r: &SnapshotRef) -> Result<(), ReviewWorkspaceError> {
        self.field(&(r.snapshot_id, r.blake3))
    }
    pub fn legacy(&mut self, r: &LegacyTargetRef) -> Result<(), ReviewWorkspaceError> {
        self.field(&(r.round_id, r.feedback_id, r.target_index))
    }
    pub fn history(&mut self, r: &HistoryRef) -> Result<(), ReviewWorkspaceError> {
        self.field(&(r.project_id, r.stream_id))?;
        match &r.source {
            HistorySource::Snapshot { snapshot, keys } => {
                self.field(&"snapshot")?;
                self.snapshot(snapshot)?;
                self.keys(keys)
            }
            HistorySource::Legacy {
                round_id,
                record_blake3,
                targets,
            } => {
                self.field(&("legacy", round_id, record_blake3, targets.len()))?;
                self.limit_targets(targets.len())?;
                for target in targets {
                    self.legacy(target)?;
                }
                Ok(())
            }
        }
    }
    pub fn binding(&mut self, b: &SourceBindingDecision) -> Result<(), ReviewWorkspaceError> {
        self.key(&b.target_key)?;
        self.field(&b.new_asset_version_id)?;
        self.anchor(&b.anchor)?;
        match b.confirmation {
            SourceBindingConfirmation::UserConfirmed => self.field(&"userConfirmed"),
            SourceBindingConfirmation::ProducerVerifiedAndPositionConfirmed { usage_id } => {
                self.field(&("producerVerifiedAndPositionConfirmed", usage_id))
            }
        }
    }
    pub fn anchor(&mut self, a: &FeedbackAnchor) -> Result<(), ReviewWorkspaceError> {
        fn canonical(v: f64) -> f64 {
            if v == 0.0 { 0.0 } else { v }
        }
        self.field(&a.kind_name())?;
        match a {
            FeedbackAnchor::Asset => Ok(()),
            FeedbackAnchor::ImageRect(r) => self.field(&(
                canonical(r.x()),
                canonical(r.y()),
                canonical(r.width()),
                canonical(r.height()),
            )),
            FeedbackAnchor::ImageStroke(s) => {
                self.field(&s.points().len())?;
                for p in s.points() {
                    self.field(&(canonical(p.x()), canonical(p.y())))?;
                }
                Ok(())
            }
            FeedbackAnchor::VideoPoint { position_us } => self.field(position_us),
            FeedbackAnchor::VideoRange { start_us, end_us } => self.field(&(start_us, end_us)),
        }
    }
}
impl Write for Encoder {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > 64 * 1024 * 1024 - self.bytes {
            self.limited = true;
            return Err(io::Error::other("command size exceeded"));
        }
        self.hash.update(bytes);
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
