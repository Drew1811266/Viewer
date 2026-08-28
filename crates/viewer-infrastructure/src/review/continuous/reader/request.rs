use super::{Failure, ReadErrorCode};
use serde::Deserialize;
use viewer_application::ReviewStreamLocator;
use viewer_domain::review::{ProductionId, ProductionScope};

#[derive(Deserialize)]
enum Protocol {
    #[serde(rename = "viewer.review.reader/1")]
    V1,
}
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) enum Operation {
    LegacyLatest,
    LegacyList,
    Current,
    History,
    List,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Request {
    #[serde(rename = "protocolVersion")]
    _protocol: Protocol,
    pub operation: Operation,
    pub project_root: String,
    pub review_stream_id: Option<String>,
    pub task_id: Option<String>,
    pub batch_id: Option<String>,
    pub since_snapshot_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub archive_id: Option<String>,
    pub legacy_round_id: Option<String>,
}
impl Request {
    pub fn validate(&self) -> Result<(), Failure> {
        if self.project_root.is_empty() || self.project_root.contains('\0') {
            return Err(Failure::new(
                ReadErrorCode::UnsafePath,
                "project root must be a nonempty path",
            ));
        }
        let selector = self.locator()?;
        if matches!(self.operation, Operation::LegacyList | Operation::List) && selector.is_some() {
            return Err(Failure::integrity(
                "--list cannot be combined with a Stream selector",
            ));
        }
        if self.since_snapshot_id.is_some() && self.operation != Operation::Current {
            return Err(Failure::integrity(
                "--since is only valid for current reads",
            ));
        }
        let history_count = [&self.snapshot_id, &self.archive_id, &self.legacy_round_id]
            .iter()
            .filter(|id| id.is_some())
            .count();
        if self.operation == Operation::History {
            if history_count != 1 || self.review_stream_id.is_none() {
                return Err(Failure::integrity(
                    "history requires --stream and exactly one explicit history selector",
                ));
            }
        } else if history_count != 0 {
            return Err(Failure::integrity(
                "history selector is not valid for this operation",
            ));
        }
        for id in [
            &self.since_snapshot_id,
            &self.snapshot_id,
            &self.archive_id,
            &self.legacy_round_id,
        ]
        .into_iter()
        .flatten()
        {
            let _: viewer_domain::ReviewSnapshotId = parse_id(id)?;
        }
        Ok(())
    }
    pub fn locator(&self) -> Result<Option<ReviewStreamLocator>, Failure> {
        if self.task_id.is_some() != self.batch_id.is_some() {
            return Err(Failure::integrity(
                "task and batch must be provided together",
            ));
        }
        if self.review_stream_id.is_some() && self.task_id.is_some() {
            return Err(Failure::integrity(
                "choose either review stream id or task and batch",
            ));
        }
        if let Some(id) = &self.review_stream_id {
            return Ok(Some(ReviewStreamLocator::Id(parse_id(id)?)));
        }
        if let (Some(task), Some(batch)) = (&self.task_id, &self.batch_id) {
            return Ok(Some(ReviewStreamLocator::Production(ProductionScope {
                task_id: ProductionId::parse(task)
                    .map_err(|_| Failure::integrity("task id is invalid"))?,
                batch_id: ProductionId::parse(batch)
                    .map_err(|_| Failure::integrity("batch id is invalid"))?,
            })));
        }
        Ok(None)
    }
}

pub(super) fn parse_id<T: std::str::FromStr>(value: &str) -> Result<T, Failure> {
    if value.len() != 36
        || !value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err(Failure::integrity("review selector id is invalid"));
    }
    value
        .parse()
        .map_err(|_| Failure::integrity("review selector id is invalid"))
}
