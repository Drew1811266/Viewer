use super::{Failure, ReadErrorCode};
use crate::{
    portable::schema::PortableSchemaError,
    review::{authoring::decode_production_scope, continuous::owned_io::Directory},
};
use rusqlite::Connection;
use std::str::FromStr;
use viewer_application::review_workspace::{ReviewAuthoringHead, ReviewHeads};
use viewer_domain::{ProjectId, ReviewStreamId, review::ProductionScope};

const MAX_CONTROLLED_STREAMS: usize = 10_000;

pub(super) struct PinnedReviewHeads {
    _connection: Connection,
    pub project_id: ProjectId,
    pub streams: Vec<PinnedReviewStream>,
}

pub(super) struct PinnedReviewStream {
    pub stream_id: ReviewStreamId,
    pub production: Option<ProductionScope>,
    pub heads: ReviewHeads,
}

impl PinnedReviewHeads {
    pub fn open(root: &Directory) -> Result<Option<Self>, Failure> {
        let Some(viewer) = root.child(".viewer", false)? else {
            return Ok(None);
        };
        let Some(database_file) = viewer.regular("metadata.sqlite", false)? else {
            return Ok(None);
        };
        let database_path = viewer.path().join("metadata.sqlite");
        let connection = crate::portable::schema::open_readonly_database(&database_path)
            .map_err(metadata_open_failure)?;
        connection
            .execute_batch("BEGIN DEFERRED TRANSACTION")
            .map_err(|_| Failure::integrity("review metadata transaction is invalid"))?;
        let has_control_plane = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type = 'table' AND name = 'review_authoring_streams'
                 )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| Failure::integrity("review metadata schema is invalid"))?;
        if !has_control_plane {
            viewer.verify_regular_name(&database_file, "metadata.sqlite")?;
            return Ok(None);
        }
        let project_text = connection
            .query_row(
                "SELECT project_id FROM project_metadata WHERE singleton = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| Failure::integrity("review metadata identity is invalid"))?;
        let project_id = canonical_id(&project_text)?;
        let streams = {
            let mut statement = connection
                .prepare(
                    "SELECT stream_id, project_id, production_scope,
                            authoring_seq, authoring_snapshot_id,
                            published_seq, published_snapshot_id
                     FROM review_authoring_streams
                     ORDER BY stream_id
                     LIMIT 10001",
                )
                .map_err(|_| Failure::integrity("review metadata heads are invalid"))?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                })
                .map_err(|_| Failure::integrity("review metadata heads are invalid"))?;
            let mut streams = Vec::new();
            for row in rows {
                let (
                    stream,
                    stored_project,
                    production,
                    authoring_seq,
                    authoring_id,
                    published_seq,
                    published_id,
                ) = row.map_err(|_| Failure::integrity("review metadata heads are invalid"))?;
                if stored_project != project_text {
                    return Err(Failure::integrity("review metadata identity is invalid"));
                }
                streams.push(PinnedReviewStream {
                    stream_id: canonical_id(&stream)?,
                    production: decode_production_scope(&production).map_err(Failure::from)?,
                    heads: ReviewHeads {
                        authoring: decode_head(authoring_seq, authoring_id.as_deref())?,
                        published: decode_head(published_seq, published_id.as_deref())?,
                    },
                });
            }
            streams
        };
        if streams.len() > MAX_CONTROLLED_STREAMS {
            return Err(Failure::new(
                ReadErrorCode::LimitExceeded,
                "review metadata stream limit exceeded",
            ));
        }
        if streams.iter().any(|stream| {
            stream
                .heads
                .published
                .zip(stream.heads.authoring)
                .is_some_and(|(published, authoring)| published.sequence > authoring.sequence)
        }) {
            return Err(Failure::integrity("review metadata heads are invalid"));
        }
        viewer.verify_regular_name(&database_file, "metadata.sqlite")?;
        root.verify()?;
        Ok(Some(Self {
            _connection: connection,
            project_id,
            streams,
        }))
    }

    pub fn require_project(&self, project_id: ProjectId) -> Result<(), Failure> {
        if self.streams.is_empty() || self.project_id == project_id {
            Ok(())
        } else {
            Err(Failure::integrity("review metadata identity is invalid"))
        }
    }
}

fn metadata_open_failure(error: PortableSchemaError) -> Failure {
    match error {
        PortableSchemaError::UnsafePath => {
            Failure::new(ReadErrorCode::UnsafePath, "review metadata path is unsafe")
        }
        PortableSchemaError::Io(_) | PortableSchemaError::Missing => {
            Failure::new(ReadErrorCode::Io, "review metadata could not be read")
        }
        PortableSchemaError::Database(_)
        | PortableSchemaError::RequiresMigration
        | PortableSchemaError::UnsupportedSchema(_)
        | PortableSchemaError::InvalidSchemaHistory => {
            Failure::integrity("review metadata database is invalid")
        }
    }
}

fn decode_head(
    sequence: Option<i64>,
    snapshot_id: Option<&str>,
) -> Result<Option<ReviewAuthoringHead>, Failure> {
    match (sequence, snapshot_id) {
        (None, None) => Ok(None),
        (Some(sequence), Some(snapshot_id)) => Ok(Some(ReviewAuthoringHead {
            sequence: u64::try_from(sequence)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| Failure::integrity("review metadata head is invalid"))?,
            snapshot_id: canonical_id(snapshot_id)?,
        })),
        _ => Err(Failure::integrity("review metadata head is incomplete")),
    }
}

fn canonical_id<T>(value: &str) -> Result<T, Failure>
where
    T: FromStr + ToString,
{
    let parsed = value
        .parse::<T>()
        .map_err(|_| Failure::integrity("review metadata identifier is invalid"))?;
    if value.len() != 36 || parsed.to_string() != value {
        return Err(Failure::integrity(
            "review metadata identifier is not canonical",
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{portable::PortableProjectMetadata, review::ProjectReviewRepositoryProvider};
    use viewer_application::{ProjectAccess, review_workspace::*};
    use viewer_domain::{
        FeedbackId, ReviewArchiveId, ReviewCommandId, ReviewSnapshotId, ReviewTextRevisionId,
        review::continuous::{ContinuousReviewState, SnapshotRef},
    };

    #[test]
    fn pinned_heads_remain_one_consistent_invocation_when_authoring_advances() {
        let root = tempfile::tempdir().unwrap();
        let metadata =
            PortableProjectMetadata::open(root.path(), ProjectAccess::ReadWrite, 1).unwrap();
        let project_id = metadata.project_id();
        drop(metadata);
        let stream = ReviewStreamId::from_u128(2);
        let snapshot_id = ReviewSnapshotId::from_u128(3);
        let provider = ProjectReviewRepositoryProvider::new(root.path(), project_id);
        provider
            .continuous_writer()
            .unwrap()
            .commit(ReviewCommitRequest {
                expected: None,
                production: None,
                next: PreparedContinuousSnapshot {
                    state: ContinuousReviewState::empty(project_id, stream, snapshot_id),
                    command_id: ReviewCommandId::from_u128(4),
                    payload_digest: [5; 32],
                    changes: vec![],
                    evidence: vec![],
                },
                archives: vec![],
                adopted_usage: vec![],
                staged_evidence: vec![],
            })
            .unwrap();
        provider.bootstrap_authoring(stream).unwrap();

        let root_directory = Directory::open_anchored(root.path()).unwrap();
        let pinned = PinnedReviewHeads::open(&root_directory).unwrap().unwrap();
        let pinned_head = pinned.streams[0].heads;
        assert_eq!(pinned_head.authoring, pinned_head.published);

        let authoring = provider.authoring_writer().unwrap();
        let current = authoring.load_current(stream).unwrap().unwrap();
        let mut next = current.clone();
        next.head = ReviewAuthoringHead {
            sequence: 2,
            snapshot_id: ReviewSnapshotId::from_u128(6),
        };
        next.state.snapshot_id = next.head.snapshot_id;
        next.state.parent = Some(SnapshotRef {
            snapshot_id: current.head.snapshot_id,
            blake3: current.payload_digest,
        });
        next.command_id = ReviewCommandId::from_u128(7);
        next.payload_digest = [8; 32];
        next.generated = GeneratedReviewIds {
            snapshot_id: next.head.snapshot_id,
            feedback_id: FeedbackId::from_u128(9),
            text_revision_id: ReviewTextRevisionId::from_u128(10),
            archive_id: ReviewArchiveId::from_u128(11),
            targets: vec![],
            migration: vec![],
            created_at_ms: 2,
        };
        next.barrier = ReviewBarrierKind::None;
        authoring
            .commit(stream, next.command_id, next.payload_digest, &mut |_| {
                Ok(ReviewAuthoringCommitRequest {
                    expected_snapshot_id: Some(current.head.snapshot_id),
                    next: next.clone(),
                })
            })
            .unwrap();

        assert_eq!(pinned.streams[0].heads, pinned_head);
        let fresh = PinnedReviewHeads::open(&root_directory).unwrap().unwrap();
        assert_ne!(
            fresh.streams[0].heads.authoring,
            fresh.streams[0].heads.published
        );
    }
}
