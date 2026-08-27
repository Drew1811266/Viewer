use super::super::common::ReviewProtocolError;
use super::{
    EvidenceCapability, EvidenceRef, ReviewArchiveRecord, ReviewIndexV3, ReviewStateRecord,
    ReviewUsageRecord,
};
use std::collections::{HashMap, HashSet};
use viewer_domain::review::continuous::{
    ContinuousReviewError, HistorySource, ReviewAvailability, ReviewPendingReason,
};
use viewer_domain::review::{FeedbackAnchor, ReviewMedia};

pub(super) const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn portable_path(path: &viewer_domain::RelativePath) -> Result<(), ReviewProtocolError> {
    let path = path.as_str();
    if path.len() > 4096 {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    if path.contains('\\') || path.as_bytes().get(1) == Some(&b':') {
        return Err(ReviewProtocolError::InvalidData);
    }
    Ok(())
}

pub(super) fn portable_assets(
    assets: &[viewer_domain::review::AssetVersion],
) -> Result<(), ReviewProtocolError> {
    for asset in assets {
        portable_path(&asset.relative_path)?;
        if asset.evidence.size_bytes > MAX_SAFE_INTEGER
            || matches!(asset.media,ReviewMedia::Video { duration_us:Some(duration), .. } if duration > MAX_SAFE_INTEGER)
        {
            return Err(ReviewProtocolError::LimitExceeded);
        }
    }
    Ok(())
}

pub(super) fn domain_error(error: ContinuousReviewError) -> ReviewProtocolError {
    match error {
        ContinuousReviewError::LimitExceeded => ReviewProtocolError::LimitExceeded,
        _ => ReviewProtocolError::InvalidData,
    }
}

pub(super) fn state(record: &ReviewStateRecord) -> Result<(), ReviewProtocolError> {
    use ReviewProtocolError::*;
    record.state.validate().map_err(domain_error)?;
    portable_assets(&record.state.assets)?;
    for item in &record.state.feedback {
        if item.created_at_ms as u64 > MAX_SAFE_INTEGER {
            return Err(LimitExceeded);
        }
        for target in &item.targets {
            if matches!(target.anchor,FeedbackAnchor::VideoPoint {position_us} if position_us > MAX_SAFE_INTEGER)
                || matches!(target.anchor,FeedbackAnchor::VideoRange {end_us,..} if end_us > MAX_SAFE_INTEGER)
            {
                return Err(LimitExceeded);
            }
        }
    }
    if record.changes.len() > 100_000_000 || record.evidence.len() > 50_000 {
        return Err(LimitExceeded);
    }
    for change in &record.changes {
        change.validate().map_err(domain_error)?;
    }
    let mut bindings = HashMap::new();
    let mut files = HashMap::new();
    let assets: HashMap<_, _> = record.state.assets.iter().map(|a| (a.id, a)).collect();
    let mut target_keys = HashMap::new();
    let mut local_keys = HashMap::<_, Vec<_>>::new();
    for feedback in &record.state.feedback {
        for target in &feedback.targets {
            let key = viewer_domain::review::continuous::TargetVersionKey {
                feedback_id: feedback.id,
                text_revision_id: feedback.text_revision_id,
                target_id: target.id,
                target_revision_id: target.revision_id,
            };
            target_keys.insert(target.id, key);
            if matches!(
                target.anchor,
                FeedbackAnchor::ImageRect(_) | FeedbackAnchor::ImageStroke(_)
            ) {
                local_keys
                    .entry(target.asset_version_id)
                    .or_default()
                    .push(key);
            }
        }
    }
    for binding in &record.evidence {
        let asset = assets.get(&binding.asset_version_id).ok_or(InvalidData)?;
        if bindings
            .insert(binding.asset_version_id, &binding.capability)
            .is_some()
        {
            return Err(InvalidData);
        }
        match &binding.capability {
            EvidenceCapability::Image {
                base,
                annotated,
                annotations,
            } => {
                if !matches!(
                    asset.media,
                    ReviewMedia::Image {
                        width: Some(_),
                        height: Some(_)
                    }
                ) || asset.evidence.blake3.is_none()
                {
                    return Err(InvalidData);
                }
                for reference in std::iter::once(base).chain(annotated) {
                    evidence_ref(reference)?;
                    if let Some(existing) = files.insert(reference.blake3, reference)
                        && existing != reference
                    {
                        return Err(InvalidData);
                    }
                }
                if annotated
                    .as_ref()
                    .is_some_and(|a| (a.width, a.height) != (base.width, base.height))
                {
                    return Err(InvalidData);
                }
                let expected = local_keys.get(&asset.id).map(Vec::as_slice).unwrap_or(&[]);
                if annotations.len() != expected.len()
                    || annotated.is_some() != !expected.is_empty()
                {
                    return Err(InvalidData);
                }
                for (index, (actual, key)) in annotations.iter().zip(expected).enumerate() {
                    if actual.ordinal as usize != index + 1 || actual.key != *key {
                        return Err(InvalidData);
                    }
                }
            }
            EvidenceCapability::NotImage {}
                if !matches!(asset.media, ReviewMedia::Video { .. }) =>
            {
                return Err(InvalidData);
            }
            EvidenceCapability::LegacyAbsent {}
                if !matches!(asset.media, ReviewMedia::Image { .. }) =>
            {
                return Err(InvalidData);
            }
            _ => {}
        }
    }
    if files.values().map(|r| r.size_bytes).sum::<u64>() > 4 * 1024 * 1024 * 1024 {
        return Err(LimitExceeded);
    }
    for feedback in &record.state.feedback {
        for target in &feedback.targets {
            match bindings.get(&target.asset_version_id) {
                Some(EvidenceCapability::LegacyAbsent {})
                    if !matches!(
                        feedback.history_ref.as_ref().map(|h| &h.source),
                        Some(HistorySource::Legacy { .. })
                    ) || !matches!(&target.availability, ReviewAvailability::NeedsConfirmation(reasons) if reasons.contains(&ReviewPendingReason::LegacyEvidenceAbsent)) =>
                {
                    return Err(InvalidData);
                }
                None => return Err(InvalidData),
                _ => {}
            }
        }
    }
    // Each transition sequence must finish at the exact key in this full snapshot.
    let mut final_keys = HashMap::new();
    let mut transitions = HashSet::new();
    for change in &record.changes {
        if let Some(previous) = final_keys.insert(change.target_id, change.after)
            && previous != change.before
        {
            return Err(InvalidData);
        }
        if !transitions.insert((
            change.target_id,
            change.before,
            change.after,
            change.archive_id,
            change.historical_key,
        )) {
            return Err(InvalidData);
        }
    }
    for (id, key) in final_keys {
        if target_keys.get(&id).copied() != key {
            return Err(InvalidData);
        }
    }
    Ok(())
}

pub(super) fn evidence_ref(value: &EvidenceRef) -> Result<(), ReviewProtocolError> {
    if value.width == 0 || value.height == 0 || value.size_bytes < 33 {
        return Err(ReviewProtocolError::InvalidData);
    }
    if value.size_bytes > 64 * 1024 * 1024
        || u64::from(value.width) * u64::from(value.height) > 16_777_216
    {
        return Err(ReviewProtocolError::LimitExceeded);
    }
    Ok(())
}

pub(super) fn index(value: &ReviewIndexV3) -> Result<(), ReviewProtocolError> {
    use ReviewProtocolError::*;
    if value.streams.len() > 10_000 {
        return Err(LimitExceeded);
    }
    let mut streams = HashSet::new();
    let mut scopes = HashSet::new();
    let mut snapshots = HashSet::new();
    let mut archives = HashSet::new();
    let mut legacy = HashSet::new();
    let mut usages = HashSet::new();
    for stream in &value.streams {
        if !streams.insert(stream.review_stream_id)
            || !scopes.insert((&stream.task_id, &stream.batch_id))
        {
            return Err(InvalidData);
        }
        match (&stream.task_id, &stream.batch_id) {
            (None, None) => {}
            (Some(task), Some(batch)) => {
                viewer_domain::review::ProductionId::parse(task).map_err(|_| InvalidData)?;
                viewer_domain::review::ProductionId::parse(batch).map_err(|_| InvalidData)?;
            }
            _ => return Err(InvalidData),
        }
        if stream.archive_refs.len() > 10_000
            || stream.legacy_refs.len() > 10_000
            || stream.usage_refs.len() > 10_000
        {
            return Err(LimitExceeded);
        }
        if let Some(reference) = &stream.current_ref
            && !snapshots.insert(reference.snapshot_id)
        {
            return Err(InvalidData);
        }
        for reference in &stream.archive_refs {
            if !archives.insert(reference.archive_id)
                || reference.location != format!("archives/{}.json", reference.archive_id)
            {
                return Err(InvalidData);
            }
        }
        for reference in &stream.usage_refs {
            if !usages.insert(reference.declaration_id)
                || reference.location != format!("usage/{}.json", reference.declaration_id)
            {
                return Err(InvalidData);
            }
        }
        for reference in &stream.legacy_refs {
            if !legacy.insert(reference.round_id) {
                return Err(InvalidData);
            }
            let expected = match reference.protocol_version.as_str() {
                "viewer.review/1" => format!("rounds/{}.json", reference.round_id),
                "viewer.review/2" => format!("rounds/{}/round.json", reference.round_id),
                _ => return Err(UnsupportedVersion),
            };
            if reference.location != expected {
                return Err(InvalidData);
            }
        }
    }
    Ok(())
}

pub(super) fn archive(record: &ReviewArchiveRecord) -> Result<(), ReviewProtocolError> {
    use ReviewProtocolError::*;
    use viewer_domain::review::continuous::{ArchiveBasis, ArchiveDisposition, classify_archive};
    let value = &record.checkpoint;
    if value.created_at_ms as u64 > MAX_SAFE_INTEGER {
        return Err(LimitExceeded);
    }
    if value.created_at_ms < 0
        || value.groups.is_empty()
        || value.before.snapshot_id == record.result_snapshot_id
    {
        return Err(InvalidData);
    }
    if value.groups.len() > 10_000
        || value.removed.len() > 100_000_000
        || value.retained.len() > 100_000_000
    {
        return Err(LimitExceeded);
    }
    let mut targets = HashMap::new();
    let mut references = HashMap::from([(value.before.snapshot_id, value.before.blake3)]);
    for group in &value.groups {
        if group.targets.is_empty() {
            return Err(InvalidData);
        }
        if group.targets.len() > 100_000_000 {
            return Err(LimitExceeded);
        }
        if let ArchiveBasis::Known { snapshot, .. } = &group.basis {
            if snapshot.snapshot_id == record.result_snapshot_id {
                return Err(InvalidData);
            }
            if references
                .insert(snapshot.snapshot_id, snapshot.blake3)
                .is_some_and(|old| old != snapshot.blake3)
            {
                return Err(InvalidData);
            }
        }
        for key in &group.targets {
            if targets.insert(key.target_id, *key).is_some() {
                return Err(InvalidData);
            }
            if targets.len() > 100_000_000 {
                return Err(LimitExceeded);
            }
        }
    }
    for key in &value.removed {
        if targets.remove(&key.target_id) != Some(*key) {
            return Err(InvalidData);
        }
    }
    for retained in &value.retained {
        if targets.remove(&retained.basis.target_id) != Some(retained.basis)
            || retained.disposition == ArchiveDisposition::RemoveCurrent
            || classify_archive(retained.current, retained.basis) != retained.disposition
        {
            return Err(InvalidData);
        }
        if retained.current.is_some_and(|k| {
            k.target_id != retained.basis.target_id || k.feedback_id != retained.basis.feedback_id
        }) {
            return Err(InvalidData);
        }
    }
    if !targets.is_empty() {
        return Err(InvalidData);
    }
    Ok(())
}

pub(super) fn usage(value: &ReviewUsageRecord) -> Result<(), ReviewProtocolError> {
    use ReviewProtocolError::*;
    if value.targets.is_empty() {
        return Err(InvalidData);
    }
    if value.targets.len() > 100_000_000 || value.outputs.len() > 50_000 {
        return Err(LimitExceeded);
    }
    for output in &value.outputs {
        portable_path(&output.relative_path)?;
    }
    let mut targets = HashSet::new();
    if value.targets.iter().any(|k| !targets.insert(k.target_id)) {
        return Err(InvalidData);
    }
    let mut paths = HashSet::new();
    if value
        .outputs
        .iter()
        .any(|o| !paths.insert(&o.relative_path))
    {
        return Err(InvalidData);
    }
    Ok(())
}
