//! Conservative retained-payload accounting for the session's prepared command cache.
use super::*;
use std::mem::size_of_val;
use viewer_domain::review::{FeedbackAnchor, continuous::*};

pub(super) const MAX_PREPARED_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn command_bytes(command: &ReviewWorkspaceCommand) -> usize {
    let mut total = 4096_usize; // Envelope/context/IDs/map-entry overhead and fixed fields.
    let mut add = |bytes: usize| total = total.saturating_add(bytes);
    match command {
        ReviewWorkspaceCommand::SaveFeedback { text, targets, .. } => {
            add(text.len());
            add(size_of_val(targets.as_slice()));
            add(targets.len().saturating_mul(32)); // Generated target IDs.
            for target in targets {
                let (TargetEdit::Add { anchor, .. } | TargetEdit::Redraw { anchor, .. }) = target;
                add(anchor_bytes(anchor));
            }
        }
        ReviewWorkspaceCommand::Withdraw { targets } => add(size_of_val(targets.as_slice())),
        ReviewWorkspaceCommand::Archive(selection) => {
            add(size_of_val(selection.groups.as_slice()));
            for group in &selection.groups {
                add(size_of_val(group.targets.as_slice()));
            }
        }
        ReviewWorkspaceCommand::Restore { decisions, .. } => {
            add(size_of_val(decisions.as_slice()));
            for decision in decisions {
                if let RestoreChoice::ContinueAsNew {
                    confirmed_anchor: Some(anchor),
                    ..
                } = &decision.choice
                {
                    add(anchor_bytes(anchor));
                }
            }
        }
        ReviewWorkspaceCommand::ContinueHistorical {
            history_ref,
            bindings,
        } => {
            add(size_of_val(bindings.as_slice()));
            add(bindings.len().saturating_mul(32));
            for binding in bindings {
                add(anchor_bytes(&binding.anchor));
            }
            match &history_ref.source {
                HistorySource::Snapshot { keys, .. } => add(size_of_val(keys.as_slice())),
                HistorySource::Legacy { targets, .. } => add(size_of_val(targets.as_slice())),
            }
        }
        ReviewWorkspaceCommand::ConfirmSource(binding) => add(anchor_bytes(&binding.anchor)),
        ReviewWorkspaceCommand::ConfirmApplicability { anchor, .. } => add(anchor_bytes(anchor)),
        ReviewWorkspaceCommand::AdoptUsage { .. } => {}
        ReviewWorkspaceCommand::ContinueLegacy {
            history_ref,
            bindings,
        } => {
            add(size_of_val(bindings.as_slice()));
            add(bindings.len().saturating_mul(32));
            if let HistorySource::Legacy { targets, .. } = &history_ref.source {
                add(size_of_val(targets.as_slice()));
            }
            for binding in bindings {
                add(anchor_bytes(&binding.anchor));
            }
        }
        ReviewWorkspaceCommand::Migrate(plan) => {
            if let MigrationChoice::ContinueSelected {
                legacy_targets,
                bindings,
            } = &plan.choice
            {
                add(size_of_val(legacy_targets.as_slice()));
                add(size_of_val(bindings.as_slice()));
                for binding in bindings {
                    add(anchor_bytes(&binding.anchor));
                }
            }
        }
    }
    total
}
fn anchor_bytes(anchor: &FeedbackAnchor) -> usize {
    match anchor {
        FeedbackAnchor::ImageStroke(stroke) => size_of_val(stroke.points()),
        _ => 0,
    }
}

pub(super) fn snapshot_bytes(snapshot: &StoredContinuousSnapshot) -> usize {
    let mut total = 4096_usize;
    for asset in &snapshot.state.assets {
        total = total
            .saturating_add(1024)
            .saturating_add(asset.relative_path.as_str().len());
    }
    for feedback in &snapshot.state.feedback {
        total = total
            .saturating_add(512)
            .saturating_add(feedback.text.len());
        for target in &feedback.targets {
            total = total
                .saturating_add(512)
                .saturating_add(anchor_bytes(&target.anchor));
        }
        if let Some(history) = &feedback.history_ref {
            let count = match &history.source {
                HistorySource::Snapshot { keys, .. } => keys.len(),
                HistorySource::Legacy { targets, .. } => targets.len(),
            };
            total = total.saturating_add(count.saturating_mul(128));
        }
    }
    for evidence in &snapshot.evidence {
        total = total.saturating_add(512);
        if let EvidenceCapability::Image { annotations, .. } = &evidence.capability {
            total = total.saturating_add(annotations.len().saturating_mul(128));
        }
    }
    total.saturating_add(snapshot.changes.len().saturating_mul(256))
}
