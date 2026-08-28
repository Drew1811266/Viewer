use super::{
    Failure, ReadErrorCode,
    project::CurrentProject,
    request::{Operation, Request},
};
use crate::review::{
    continuous::{history, references, repository::View},
    v3,
};
use serde_json::{Value, json};
use viewer_application::ReviewStreamLocator;

pub(super) fn selected<'a>(
    view: &'a View,
    request: &Request,
) -> Result<Option<&'a v3::ReviewStreamV3>, Failure> {
    let locator = request.locator()?;
    let mut matches = view.index.streams.iter().filter(|s| match &locator {
        None => true,
        Some(ReviewStreamLocator::Id(id)) => s.review_stream_id == *id,
        Some(ReviewStreamLocator::Production(scope)) => {
            s.task_id.as_deref() == Some(scope.task_id.as_str())
                && s.batch_id.as_deref() == Some(scope.batch_id.as_str())
        }
    });
    let first = matches.next();
    if matches.next().is_some() {
        return Err(Failure::new(
            ReadErrorCode::AmbiguousStream,
            "multiple review streams; select one exact --stream or --task and --batch",
        ));
    }
    if first.is_none() && locator.is_some() {
        return Err(Failure::new(
            ReadErrorCode::UnknownStream,
            "review stream was not found",
        ));
    }
    Ok(first)
}

pub(super) fn read(project: &CurrentProject, request: &Request) -> Result<Value, Failure> {
    if request.operation == Operation::List {
        let streams: Vec<_> = project.view.as_ref().map(|v| v.index.streams.iter().map(|s| json!({
            "reviewStreamId": s.review_stream_id, "taskId": s.task_id, "batchId": s.batch_id,
            "currentRef": s.current_ref.map(|r| json!({"snapshotId": r.snapshot_id, "blake3": blake3::Hash::from(r.blake3).to_hex().as_str()})),
        })).collect()).unwrap_or_default();
        return Ok(
            json!({"protocolVersion": "viewer.review/3", "status": "ok", "projectId": project.project_id, "streams": streams}),
        );
    }
    let Some(view) = &project.view else {
        if request.locator()?.is_some() {
            return Err(Failure::new(
                ReadErrorCode::UnknownStream,
                "project has no review stream",
            ));
        }
        return result(v3::ReviewReadResult::NoReviewState(
            v3::NoReviewStateResult::new(project.project_id, None),
        ));
    };
    let stream = selected(view, request)?;
    let Some((stream, reference)) = stream.and_then(|s| s.current_ref.map(|r| (s, r))) else {
        return result(v3::ReviewReadResult::NoReviewState(
            v3::NoReviewStateResult::new(project.project_id, stream.map(|s| s.review_stream_id)),
        ));
    };
    let record = history::read_state(view, stream.review_stream_id, &reference)?;
    references::feedback_origins(view, &record)?;
    let source_checks = super::source::check(&project.root, &record.state.assets);
    let delta = super::delta::read(view, &record, request.since_snapshot_id.as_deref())?;
    project.root.verify()?;
    result(v3::ReviewReadResult::Current(
        v3::CurrentReadResult::from_verified(reference, record, source_checks, delta)?,
    ))
}

pub(super) fn result(value: v3::ReviewReadResult) -> Result<Value, Failure> {
    value.validate()?;
    serde_json::to_value(value)
        .map_err(|_| Failure::integrity("cannot encode verified read result"))
}
