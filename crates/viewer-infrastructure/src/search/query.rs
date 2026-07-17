use async_trait::async_trait;
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};
use std::{collections::HashSet, str::FromStr, sync::Arc};
use viewer_application::{SearchPort, search::SearchError};
use viewer_domain::{
    EntityId, SessionId,
    file::{FileKind, FileNode},
    search::{Generation, MatchedField, SearchHit, SearchPage, SearchQuery, SearchScope},
};

use super::index::{SessionIndex, SessionIndexError, encode_kind, encode_review_state, read_node};

const EXACT_FILENAME_BAND: i64 = 4_000_000;
const FILENAME_BAND: i64 = 3_000_000;
const PATH_BAND: i64 = 2_000_000;
const BODY_BAND: i64 = 1_000_000;
const SHORT_TEXT_CANDIDATE_LIMIT: usize = 2_000;

#[derive(Clone)]
pub struct SessionSearch {
    session_id: SessionId,
    index: Arc<SessionIndex>,
}

impl SessionSearch {
    pub fn new(session_id: SessionId, index: Arc<SessionIndex>) -> Self {
        Self { session_id, index }
    }
}

#[async_trait]
impl SearchPort for SessionSearch {
    async fn search(
        &self,
        session_id: SessionId,
        _generation: Generation,
        query: SearchQuery,
    ) -> Result<SearchPage, SearchError> {
        if session_id != self.session_id {
            return Err(SearchError::InvalidSession);
        }
        let index = Arc::clone(&self.index);
        tokio::task::spawn_blocking(move || index.search_page(&query))
            .await
            .map_err(|error| SearchError::Backend(error.to_string()))?
            .map_err(|error| SearchError::Backend(error.to_string()))
    }
}

impl SessionIndex {
    pub fn search_page(&self, query: &SearchQuery) -> Result<SearchPage, SessionIndexError> {
        let connection = self.lock_connection();
        let candidates = load_candidates(&connection, query)?;
        let text = query.text.trim();
        let body_matches = body_matches(&connection, text, &candidates)?;
        let pattern = Pattern::new(
            text,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut filename_matcher = Matcher::new(Config::DEFAULT);
        let mut path_matcher = Matcher::new(Config::DEFAULT.match_paths());
        let mut utf32_buffer = Vec::new();
        let folded_text = text.to_lowercase();
        let mut hits = Vec::new();

        for candidate in candidates {
            if text.is_empty() {
                hits.push(SearchHit {
                    node: candidate.node,
                    matched_field: MatchedField::Path,
                    score: 0,
                });
                continue;
            }
            let exact_filename = candidate.name.to_lowercase() == folded_text;
            let filename_score = pattern.score(
                Utf32Str::new(&candidate.name, &mut utf32_buffer),
                &mut filename_matcher,
            );
            let path_score = pattern.score(
                Utf32Str::new(candidate.node.relative_path.as_str(), &mut utf32_buffer),
                &mut path_matcher,
            );
            let (matched_field, score) = if exact_filename {
                (MatchedField::ExactFilename, EXACT_FILENAME_BAND)
            } else if let Some(score) = filename_score {
                (MatchedField::Filename, FILENAME_BAND + i64::from(score))
            } else if let Some(score) = path_score {
                (MatchedField::Path, PATH_BAND + i64::from(score))
            } else if body_matches.contains(&candidate.node.entity_id) {
                (MatchedField::Body, BODY_BAND)
            } else {
                continue;
            };
            hits.push(SearchHit {
                node: candidate.node,
                matched_field,
                score,
            });
        }

        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| {
                    left.node
                        .relative_path
                        .as_str()
                        .to_lowercase()
                        .cmp(&right.node.relative_path.as_str().to_lowercase())
                })
                .then_with(|| {
                    left.node
                        .entity_id
                        .to_string()
                        .cmp(&right.node.entity_id.to_string())
                })
        });
        let total = u32::try_from(hits.len()).unwrap_or(u32::MAX);
        let offset = usize::try_from(query.offset).unwrap_or(usize::MAX);
        let limit = usize::try_from(query.limit).unwrap_or(usize::MAX);
        let hits = hits.into_iter().skip(offset).take(limit).collect();
        Ok(SearchPage { total, hits })
    }
}

struct Candidate {
    node: FileNode,
    name: String,
}

fn load_candidates(
    connection: &Connection,
    query: &SearchQuery,
) -> Result<Vec<Candidate>, SessionIndexError> {
    let mut sql = String::from(
        "SELECT entity_id, relative_path, kind, size, modified_ns, name
         FROM nodes WHERE 1 = 1",
    );
    let mut values = Vec::<Value>::new();
    if let SearchScope::Subtree(root) = query.scope {
        let Some(path) = connection
            .query_row(
                "SELECT relative_path FROM nodes WHERE entity_id = ?1",
                [root.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        else {
            return Ok(Vec::new());
        };
        sql.push_str(
            " AND (relative_path = ?
                    OR substr(relative_path, 1, length(?) + 1) = ? || '/')",
        );
        values.extend([
            Value::Text(path.clone()),
            Value::Text(path.clone()),
            Value::Text(path),
        ]);
    }
    if !query.kinds.is_empty() {
        sql.push_str(" AND kind IN (");
        push_placeholders(&mut sql, query.kinds.len());
        sql.push(')');
        values.extend(
            query
                .kinds
                .iter()
                .map(|kind| Value::Integer(encode_kind(*kind))),
        );
    }
    if !query.review_states.is_empty() {
        sql.push_str(" AND review_state IN (");
        push_placeholders(&mut sql, query.review_states.len());
        sql.push(')');
        values.extend(
            query
                .review_states
                .iter()
                .map(|state| Value::Integer(encode_review_state(*state))),
        );
    }
    if query.favorite_only {
        sql.push_str(" AND favorite = 1");
    }
    sql.push_str(" ORDER BY relative_path, entity_id");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values.iter()), |row| {
        Ok(Candidate {
            node: read_node(row)?,
            name: row.get(5)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn body_matches(
    connection: &Connection,
    text: &str,
    candidates: &[Candidate],
) -> Result<HashSet<EntityId>, SessionIndexError> {
    if text.is_empty() {
        return Ok(HashSet::new());
    }
    let eligible = candidates
        .iter()
        .filter(|candidate| matches!(candidate.node.kind, FileKind::Markdown | FileKind::Text));
    if eligible.clone().next().is_none() {
        return Ok(HashSet::new());
    }
    if text.chars().count() <= 2 {
        let bounded: Vec<_> = eligible
            .take(SHORT_TEXT_CANDIDATE_LIMIT)
            .map(|candidate| candidate.node.entity_id)
            .collect();
        return short_body_matches(connection, text, &bounded);
    }

    let allowed: HashSet<_> = eligible.map(|candidate| candidate.node.entity_id).collect();
    let phrase = format!("\"{}\"", text.replace('"', "\"\""));
    let mut statement = connection.prepare("SELECT entity_id FROM text_fts WHERE body MATCH ?1")?;
    let rows = statement.query_map([phrase], |row| row.get::<_, String>(0))?;
    let mut matches = HashSet::new();
    for value in rows {
        let entity_id = parse_entity_id(value?)?;
        if allowed.contains(&entity_id) {
            matches.insert(entity_id);
        }
    }
    Ok(matches)
}

fn short_body_matches(
    connection: &Connection,
    text: &str,
    eligible: &[EntityId],
) -> Result<HashSet<EntityId>, SessionIndexError> {
    let mut sql = String::from("SELECT entity_id, body FROM text_fts WHERE entity_id IN (");
    push_placeholders(&mut sql, eligible.len());
    sql.push(')');
    let identifiers: Vec<_> = eligible
        .iter()
        .map(|entity_id| Value::Text(entity_id.to_string()))
        .collect();
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(identifiers.iter()), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let folded_text = text.to_lowercase();
    let mut matches = HashSet::new();
    for row in rows {
        let (entity_id, body) = row?;
        if body.to_lowercase().contains(&folded_text) {
            matches.insert(parse_entity_id(entity_id)?);
        }
    }
    Ok(matches)
}

fn parse_entity_id(value: String) -> Result<EntityId, SessionIndexError> {
    EntityId::from_str(&value).map_err(|_| SessionIndexError::InvalidPersistedValue {
        field: "entity_id",
        value,
    })
}

fn push_placeholders(sql: &mut String, count: usize) {
    for index in 0..count {
        if index > 0 {
            sql.push(',');
        }
        sql.push('?');
    }
}
