use super::{
    Failure, ReadErrorCode,
    project::CurrentProject,
    request::{Operation, Request},
};
use crate::review::{
    ContinuousReviewProtocol,
    continuous::{history, references, repository::View},
    v3, v4,
};
use serde_json::{Value, json};
use viewer_application::{
    ReviewStreamLocator,
    review_workspace::{ReviewHeads, ReviewPublicationProtocol},
};
use viewer_domain::review::continuous::SnapshotRef;

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
        gate_list(project)?;
        let streams: Vec<_> = project.view.as_ref().map(|v| v.index.streams.iter().map(|s| json!({
            "reviewStreamId": s.review_stream_id, "taskId": s.task_id, "batchId": s.batch_id,
            "currentRef": s.current_ref.map(|r| json!({"snapshotId": r.snapshot_id, "blake3": blake3::Hash::from(r.blake3).to_hex().as_str()})),
        })).collect()).unwrap_or_default();
        let protocol: ContinuousReviewProtocol = project
            .view
            .as_ref()
            .map_or(ReviewPublicationProtocol::V3, |view| view.index.protocol)
            .into();
        return Ok(
            json!({"protocolVersion": protocol.as_str(), "status": "ok", "projectId": project.project_id, "streams": streams}),
        );
    }
    let Some(view) = &project.view else {
        gate_without_public_view(project, request)?;
        if request.locator()?.is_some() {
            return Err(Failure::new(
                ReadErrorCode::UnknownStream,
                "project has no review stream",
            ));
        }
        return result(
            ReviewPublicationProtocol::V3,
            v3::ReviewReadResult::NoReviewState(v3::NoReviewStateResult::new(
                project.project_id,
                None,
            )),
        );
    };
    let explicit = controlled_for_locator(project, request)?;
    if explicit.is_some_and(|stream| stream.heads.authoring != stream.heads.published) {
        return Err(Failure::publication_pending());
    }
    let stream = selected(view, request)?;
    if let Some(controlled) = explicit
        && stream.is_some_and(|stream| stream.review_stream_id != controlled.stream_id)
    {
        return Err(Failure::integrity(
            "review metadata and public stream selection disagree",
        ));
    }
    if let Some(stream) = stream {
        gate_stream(
            project.heads.as_ref().and_then(|heads| {
                heads
                    .streams
                    .iter()
                    .find(|value| value.stream_id == stream.review_stream_id)
            }),
            stream.current_ref,
        )?;
    } else if project.heads.as_ref().is_some_and(|heads| {
        heads
            .streams
            .iter()
            .any(|stream| stream.heads.authoring != stream.heads.published)
    }) {
        return Err(Failure::publication_pending());
    }
    let Some((stream, reference)) = stream.and_then(|s| s.current_ref.map(|r| (s, r))) else {
        return result(
            view.index.protocol,
            v3::ReviewReadResult::NoReviewState(v3::NoReviewStateResult::new(
                project.project_id,
                stream.map(|s| s.review_stream_id),
            )),
        );
    };
    let record = history::read_state(view, stream.review_stream_id, &reference)?;
    references::feedback_origins(view, &record)?;
    let source_checks = super::source::check(&project.root, &record.state.assets);
    let delta = super::delta::read(view, &record.record, request.since_snapshot_id.as_deref())?;
    project.root.verify()?;
    result(
        record.protocol,
        v3::ReviewReadResult::Current(v3::CurrentReadResult::from_verified(
            reference,
            record.record,
            source_checks,
            delta,
        )?),
    )
}

fn controlled_for_locator<'a>(
    project: &'a CurrentProject,
    request: &Request,
) -> Result<Option<&'a super::heads::PinnedReviewStream>, Failure> {
    let Some(heads) = &project.heads else {
        return Ok(None);
    };
    match request.locator()? {
        None => Ok(None),
        Some(ReviewStreamLocator::Id(id)) => {
            Ok(heads.streams.iter().find(|stream| stream.stream_id == id))
        }
        Some(ReviewStreamLocator::Production(production)) => {
            let mut matches = heads
                .streams
                .iter()
                .filter(|stream| stream.production.as_ref() == Some(&production));
            let first = matches.next();
            if matches.next().is_some() {
                return Err(Failure::new(
                    ReadErrorCode::AmbiguousStream,
                    "multiple review streams match task and batch",
                ));
            }
            Ok(first)
        }
    }
}

fn gate_without_public_view(project: &CurrentProject, request: &Request) -> Result<(), Failure> {
    if let Some(stream) = controlled_for_locator(project, request)? {
        return gate_stream(Some(stream), None);
    }
    if request.locator()?.is_some() {
        return Ok(());
    }
    let Some(heads) = &project.heads else {
        return Ok(());
    };
    if heads
        .streams
        .iter()
        .any(|stream| stream.heads.authoring != stream.heads.published)
    {
        return Err(Failure::publication_pending());
    }
    if heads
        .streams
        .iter()
        .any(|stream| stream.heads.authoring.is_some())
    {
        return Err(Failure::integrity(
            "published review head has no public index",
        ));
    }
    Ok(())
}

fn gate_list(project: &CurrentProject) -> Result<(), Failure> {
    let Some(heads) = &project.heads else {
        return Ok(());
    };
    if heads
        .streams
        .iter()
        .any(|stream| stream.heads.authoring != stream.heads.published)
    {
        return Err(Failure::publication_pending());
    }
    for controlled in &heads.streams {
        let public = project.view.as_ref().and_then(|view| {
            view.index
                .streams
                .iter()
                .find(|stream| stream.review_stream_id == controlled.stream_id)
                .and_then(|stream| stream.current_ref)
        });
        gate_stream(Some(controlled), public)?;
    }
    Ok(())
}

fn gate_stream(
    controlled: Option<&super::heads::PinnedReviewStream>,
    public: Option<SnapshotRef>,
) -> Result<(), Failure> {
    let Some(controlled) = controlled else {
        return Ok(());
    };
    let ReviewHeads {
        authoring,
        published,
    } = controlled.heads;
    if authoring != published {
        return Err(Failure::publication_pending());
    }
    match (authoring, public) {
        (None, None) => Ok(()),
        (Some(head), Some(reference)) if head.snapshot_id == reference.snapshot_id => Ok(()),
        _ => Err(Failure::integrity(
            "published review head does not match the public index",
        )),
    }
}

pub(super) fn result(
    protocol: ReviewPublicationProtocol,
    value: v3::ReviewReadResult,
) -> Result<Value, Failure> {
    let bytes = match protocol {
        ReviewPublicationProtocol::V3 => v3::encode_read_result_v3(&value),
        ReviewPublicationProtocol::V4 => v4::encode_read_result_v4(&value),
    }?;
    serde_json::from_slice(&bytes)
        .map_err(|_| Failure::integrity("cannot encode verified read result"))
}
