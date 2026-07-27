use rusqlite::{TransactionBehavior, params};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use viewer_application::metadata::{
    FileCopyProjection, FileMoveProjection, MarkerChange, MarkerProjectionError,
    MarkerProjectionPort, OperationProjectionError, OperationProjectionPort,
};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, FileNode},
};

use super::{SessionIndex, encode_kind, encode_review_state};

impl MarkerProjectionPort for SessionIndex {
    fn sync_markers(&self, changes: &[MarkerChange]) -> Result<(), MarkerProjectionError> {
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction()
            .map_err(|_| MarkerProjectionError::Unavailable)?;
        for change in changes {
            let changed = transaction
                .execute(
                    "UPDATE nodes SET review_state = ?4, favorite = ?5
                     WHERE entity_id = ?1 AND relative_path = ?2 AND kind = ?3",
                    params![
                        change.target.entity_id.to_string(),
                        change.target.relative_path.as_str(),
                        encode_kind(change.target.kind),
                        change.marker.review_state.map(encode_review_state),
                        change.marker.favorite,
                    ],
                )
                .map_err(|_| MarkerProjectionError::Unavailable)?;
            if changed != 1 {
                return Err(MarkerProjectionError::Unavailable);
            }
        }
        transaction
            .commit()
            .map_err(|_| MarkerProjectionError::Unavailable)
    }
}

impl OperationProjectionPort for SessionIndex {
    fn apply_copy(
        &self,
        copies: &[FileCopyProjection],
        case_sensitive: bool,
    ) -> Result<(), OperationProjectionError> {
        validate_copy_projections(copies, case_sensitive)?;
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| OperationProjectionError::Unavailable)?;

        for copy in copies {
            if !node_matches(&transaction, &copy.source)? {
                return Err(OperationProjectionError::Stale);
            }
            let entity_exists = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM nodes WHERE entity_id = ?1)",
                    [copy.destination.entity_id.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
            if entity_exists {
                return Err(OperationProjectionError::Conflict);
            }
        }

        let existing = projection_nodes(&transaction)?;
        let mut occupied = HashSet::with_capacity(existing.len() + copies.len());
        for row in &existing {
            if !occupied.insert(projection_path_key(&row.path, case_sensitive)) {
                return Err(OperationProjectionError::Stale);
            }
        }
        for copy in copies {
            if !occupied.insert(projection_path_key(
                copy.destination.relative_path.as_str(),
                case_sensitive,
            )) {
                return Err(OperationProjectionError::Conflict);
            }
        }

        let mut future_nodes = existing
            .into_iter()
            .map(|row| (row.path, (row.entity_id, row.kind)))
            .collect::<HashMap<_, _>>();
        for copy in copies {
            future_nodes.insert(
                copy.destination.relative_path.as_str().to_owned(),
                (
                    copy.destination.entity_id.to_string(),
                    encode_kind(copy.destination.kind),
                ),
            );
        }

        let mut ordered = copies.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|copy| copy.destination.relative_path.as_str().matches('/').count());
        for copy in ordered {
            let parent_id =
                projection_parent(copy.destination.relative_path.as_str(), &future_nodes)?;
            let size = i64::try_from(copy.destination.size)
                .map_err(|_| OperationProjectionError::InvalidInput)?;
            transaction
                .execute(
                    "INSERT INTO nodes(
                        entity_id, parent_entity_id, relative_path, name, kind, size, modified_ns
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        copy.destination.entity_id.to_string(),
                        parent_id,
                        copy.destination.relative_path.as_str(),
                        projection_name(copy.destination.relative_path.as_str()),
                        encode_kind(copy.destination.kind),
                        size,
                        copy.destination.modified_ns.to_string(),
                    ],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| OperationProjectionError::Unavailable)
    }

    fn apply_move(
        &self,
        moves: &[FileMoveProjection],
        case_sensitive: bool,
    ) -> Result<(), OperationProjectionError> {
        validate_move_projections(moves, case_sensitive)?;
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| OperationProjectionError::Unavailable)?;
        for mapping in moves {
            if !node_matches(&transaction, &mapping.source)? {
                return Err(OperationProjectionError::Stale);
            }
            if mapping.destination.entity_id != mapping.source.entity_id {
                let destination_exists = transaction
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM nodes WHERE entity_id = ?1)",
                        [mapping.destination.entity_id.to_string()],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(|_| OperationProjectionError::Unavailable)?;
                if destination_exists {
                    return Err(OperationProjectionError::Conflict);
                }
            }
        }

        let existing = projection_nodes(&transaction)?;
        let mut affected = Vec::new();
        let mut unaffected = Vec::new();
        for row in existing {
            if let Some((mapping_index, destination)) = projected_session_path(&row.path, moves) {
                if RelativePath::parse(&destination).is_err() {
                    return Err(OperationProjectionError::InvalidInput);
                }
                affected.push(ProjectedSessionNode {
                    entity_id: row.entity_id,
                    old_path: row.path,
                    new_path: destination,
                    kind: row.kind,
                    mapping_index,
                });
            } else {
                unaffected.push(row);
            }
        }

        let mut occupied = HashSet::with_capacity(unaffected.len() + affected.len());
        for row in &unaffected {
            if !occupied.insert(projection_path_key(&row.path, case_sensitive)) {
                return Err(OperationProjectionError::Conflict);
            }
        }
        for row in &affected {
            if !occupied.insert(projection_path_key(&row.new_path, case_sensitive)) {
                return Err(OperationProjectionError::Conflict);
            }
        }

        let mut future_nodes = unaffected
            .into_iter()
            .map(|row| (row.path, (row.entity_id, row.kind)))
            .collect::<HashMap<_, _>>();
        for row in &affected {
            let mapping = &moves[row.mapping_index];
            let entity_id = if row.old_path == mapping.source.relative_path.as_str() {
                mapping.destination.entity_id.to_string()
            } else {
                row.entity_id.clone()
            };
            future_nodes.insert(row.new_path.clone(), (entity_id, row.kind));
        }
        let mut root_parents = HashMap::with_capacity(moves.len());
        for (index, mapping) in moves.iter().enumerate() {
            root_parents.insert(
                index,
                projection_parent(mapping.destination.relative_path.as_str(), &future_nodes)?,
            );
        }

        for row in &affected {
            let temporary = format!(".viewer-projection/{}", row.entity_id);
            transaction
                .execute(
                    "UPDATE nodes SET relative_path = ?2, name = ?3 WHERE entity_id = ?1",
                    params![
                        &row.entity_id,
                        temporary,
                        format!("projection-{}", row.entity_id),
                    ],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        for row in &affected {
            let parent_id = (row.old_path
                == moves[row.mapping_index].source.relative_path.as_str())
            .then(|| root_parents[&row.mapping_index].clone())
            .flatten();
            if row.old_path == moves[row.mapping_index].source.relative_path.as_str() {
                let destination = &moves[row.mapping_index].destination;
                let size = i64::try_from(destination.size)
                    .map_err(|_| OperationProjectionError::InvalidInput)?;
                transaction
                    .execute(
                        "UPDATE nodes
                         SET entity_id = ?2, parent_entity_id = ?3, relative_path = ?4,
                             name = ?5, size = ?6, modified_ns = ?7
                         WHERE entity_id = ?1",
                        params![
                            &row.entity_id,
                            destination.entity_id.to_string(),
                            parent_id,
                            &row.new_path,
                            projection_name(&row.new_path),
                            size,
                            destination.modified_ns.to_string(),
                        ],
                    )
                    .map_err(|_| OperationProjectionError::Unavailable)?;
            } else {
                transaction
                    .execute(
                        "UPDATE nodes SET relative_path = ?2, name = ?3 WHERE entity_id = ?1",
                        params![
                            &row.entity_id,
                            &row.new_path,
                            projection_name(&row.new_path),
                        ],
                    )
                    .map_err(|_| OperationProjectionError::Unavailable)?;
            }
            transaction
                .execute(
                    "UPDATE text_fts SET entity_id = ?2, relative_path = ?3 WHERE entity_id = ?1",
                    params![
                        &row.entity_id,
                        if row.old_path == moves[row.mapping_index].source.relative_path.as_str() {
                            moves[row.mapping_index].destination.entity_id.to_string()
                        } else {
                            row.entity_id.clone()
                        },
                        &row.new_path,
                    ],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| OperationProjectionError::Unavailable)
    }

    fn apply_trash(&self, sources: &[FileNode]) -> Result<(), OperationProjectionError> {
        validate_trash_projections(sources)?;
        let mut connection = self.lock_connection();
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| OperationProjectionError::Unavailable)?;
        for source in sources {
            if !node_matches(&transaction, source)? {
                return Err(OperationProjectionError::Stale);
            }
        }
        for source in sources {
            transaction
                .execute(
                    "DELETE FROM text_fts WHERE entity_id IN (
                        SELECT entity_id FROM nodes
                        WHERE relative_path = ?1
                           OR substr(relative_path, 1, length(?1) + 1) = ?1 || '/'
                     )",
                    [source.relative_path.as_str()],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
            transaction
                .execute(
                    "DELETE FROM nodes
                     WHERE relative_path = ?1
                        OR substr(relative_path, 1, length(?1) + 1) = ?1 || '/'",
                    [source.relative_path.as_str()],
                )
                .map_err(|_| OperationProjectionError::Unavailable)?;
        }
        transaction
            .commit()
            .map_err(|_| OperationProjectionError::Unavailable)
    }
}

#[derive(Debug)]
struct ProjectionNodeRow {
    entity_id: String,
    path: String,
    kind: i64,
}

#[derive(Debug)]
struct ProjectedSessionNode {
    entity_id: String,
    old_path: String,
    new_path: String,
    kind: i64,
    mapping_index: usize,
}

fn projection_nodes(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<Vec<ProjectionNodeRow>, OperationProjectionError> {
    let mut statement = transaction
        .prepare("SELECT entity_id, relative_path, kind FROM nodes ORDER BY relative_path")
        .map_err(|_| OperationProjectionError::Unavailable)?;
    let rows = statement
        .query_map([], |row| {
            Ok(ProjectionNodeRow {
                entity_id: row.get(0)?,
                path: row.get(1)?,
                kind: row.get(2)?,
            })
        })
        .map_err(|_| OperationProjectionError::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| OperationProjectionError::Unavailable)?;
    if rows.iter().any(|row| {
        EntityId::from_str(&row.entity_id).is_err()
            || RelativePath::parse(&row.path).is_err()
            || !(0..=4).contains(&row.kind)
    }) {
        return Err(OperationProjectionError::Stale);
    }
    Ok(rows)
}

fn node_matches(
    transaction: &rusqlite::Transaction<'_>,
    node: &FileNode,
) -> Result<bool, OperationProjectionError> {
    let size = i64::try_from(node.size).map_err(|_| OperationProjectionError::InvalidInput)?;
    transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM nodes
                WHERE entity_id = ?1 AND relative_path = ?2 AND kind = ?3
                  AND size = ?4 AND modified_ns = ?5
             )",
            params![
                node.entity_id.to_string(),
                node.relative_path.as_str(),
                encode_kind(node.kind),
                size,
                node.modified_ns.to_string(),
            ],
            |row| row.get(0),
        )
        .map_err(|_| OperationProjectionError::Unavailable)
}

fn validate_copy_projections(
    copies: &[FileCopyProjection],
    case_sensitive: bool,
) -> Result<(), OperationProjectionError> {
    if copies.is_empty() {
        return Err(OperationProjectionError::InvalidInput);
    }
    let mut entities = HashSet::with_capacity(copies.len());
    let mut destinations = HashSet::with_capacity(copies.len());
    for copy in copies {
        if copy.source.kind != copy.destination.kind
            || copy.source.size != copy.destination.size
            || copy.source.entity_id == copy.destination.entity_id
            || !entities.insert(copy.destination.entity_id)
        {
            return Err(OperationProjectionError::InvalidInput);
        }
        if !destinations.insert(projection_path_key(
            copy.destination.relative_path.as_str(),
            case_sensitive,
        )) {
            return Err(OperationProjectionError::Conflict);
        }
    }
    Ok(())
}

fn validate_move_projections(
    moves: &[FileMoveProjection],
    case_sensitive: bool,
) -> Result<(), OperationProjectionError> {
    if moves.is_empty() {
        return Err(OperationProjectionError::InvalidInput);
    }
    let mut entities = HashSet::with_capacity(moves.len());
    let mut destination_entities = HashSet::with_capacity(moves.len());
    let mut sources = HashSet::with_capacity(moves.len());
    let mut destinations = HashSet::with_capacity(moves.len());
    for mapping in moves {
        if mapping.source.relative_path == mapping.destination.relative_path
            || mapping.source.kind != mapping.destination.kind
            || mapping.source.size != mapping.destination.size
            || (mapping.source.kind == FileKind::Directory
                && mapping.source.entity_id != mapping.destination.entity_id)
            || !entities.insert(mapping.source.entity_id)
            || !destination_entities.insert(mapping.destination.entity_id)
            || !sources.insert(projection_path_key(
                mapping.source.relative_path.as_str(),
                case_sensitive,
            ))
            || projection_is_descendant(
                mapping.destination.relative_path.as_str(),
                mapping.source.relative_path.as_str(),
                case_sensitive,
            )
        {
            return Err(OperationProjectionError::InvalidInput);
        }
        if !destinations.insert(projection_path_key(
            mapping.destination.relative_path.as_str(),
            case_sensitive,
        )) {
            return Err(OperationProjectionError::Conflict);
        }
    }
    for (index, mapping) in moves.iter().enumerate() {
        if moves[index + 1..].iter().any(|other| {
            projection_is_descendant(
                mapping.source.relative_path.as_str(),
                other.source.relative_path.as_str(),
                case_sensitive,
            ) || projection_is_descendant(
                other.source.relative_path.as_str(),
                mapping.source.relative_path.as_str(),
                case_sensitive,
            )
        }) {
            return Err(OperationProjectionError::InvalidInput);
        }
    }
    Ok(())
}

fn validate_trash_projections(sources: &[FileNode]) -> Result<(), OperationProjectionError> {
    if sources.is_empty() {
        return Err(OperationProjectionError::InvalidInput);
    }
    let mut entities = HashSet::with_capacity(sources.len());
    let mut paths = HashSet::with_capacity(sources.len());
    for source in sources {
        if !entities.insert(source.entity_id) || !paths.insert(source.relative_path.clone()) {
            return Err(OperationProjectionError::InvalidInput);
        }
    }
    for (index, source) in sources.iter().enumerate() {
        if sources[index + 1..].iter().any(|other| {
            projection_is_descendant(
                source.relative_path.as_str(),
                other.relative_path.as_str(),
                true,
            ) || projection_is_descendant(
                other.relative_path.as_str(),
                source.relative_path.as_str(),
                true,
            )
        }) {
            return Err(OperationProjectionError::InvalidInput);
        }
    }
    Ok(())
}

fn projected_session_path(path: &str, moves: &[FileMoveProjection]) -> Option<(usize, String)> {
    moves.iter().enumerate().find_map(|(index, mapping)| {
        if path == mapping.source.relative_path.as_str() {
            return Some((index, mapping.destination.relative_path.as_str().to_owned()));
        }
        path.strip_prefix(mapping.source.relative_path.as_str())
            .and_then(|suffix| suffix.strip_prefix('/'))
            .map(|suffix| {
                (
                    index,
                    format!("{}/{suffix}", mapping.destination.relative_path.as_str()),
                )
            })
    })
}

fn projection_parent(
    path: &str,
    future_nodes: &HashMap<String, (String, i64)>,
) -> Result<Option<String>, OperationProjectionError> {
    let Some((parent, _)) = path.rsplit_once('/') else {
        return Ok(None);
    };
    let Some((entity_id, kind)) = future_nodes.get(parent) else {
        return Err(OperationProjectionError::Stale);
    };
    if *kind != encode_kind(FileKind::Directory) {
        return Err(OperationProjectionError::Stale);
    }
    Ok(Some(entity_id.clone()))
}

pub(super) fn projection_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn projection_path_key(path: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        path.to_owned()
    } else {
        path.to_lowercase()
    }
}

fn projection_is_descendant(candidate: &str, ancestor: &str, case_sensitive: bool) -> bool {
    let candidate = projection_path_key(candidate, case_sensitive);
    let ancestor = projection_path_key(ancestor, case_sensitive);
    candidate
        .strip_prefix(&ancestor)
        .is_some_and(|suffix| suffix.starts_with('/'))
}
