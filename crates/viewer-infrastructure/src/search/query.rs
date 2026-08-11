use async_trait::async_trait;
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};
use std::{cmp::Ordering, collections::HashSet, str::FromStr, sync::Arc};
use viewer_application::{
    SearchPort, SearchSnippetPort,
    metadata::IndexedNode,
    search::{SearchError, natural_cmp},
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, ReviewState},
    search::{
        Generation, ImageOrientation, MatchRange, MatchedField, SearchHit, SearchLayout,
        SearchPage, SearchQuery, SearchScope, SearchSortKey, SortDirection,
    },
};

use super::index::{
    SessionIndex, SessionIndexError, encode_kind, encode_review_state, read_indexed_node,
};

const EXACT_FILENAME_BAND: i64 = 4_000_000;
const FILENAME_BAND: i64 = 3_000_000;
const PATH_BAND: i64 = 2_000_000;
const BODY_BAND: i64 = 1_000_000;
const SHORT_TEXT_CANDIDATE_LIMIT: usize = 2_000;
const MAX_SNIPPET_SCALARS: usize = 160;
const SNIPPET_LEADING_CONTEXT: usize = 60;

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
        if !query.is_valid() {
            return Err(SearchError::InvalidQuery);
        }
        let index = Arc::clone(&self.index);
        tokio::task::spawn_blocking(move || index.search_page(&query))
            .await
            .map_err(|error| SearchError::Backend(error.to_string()))?
            .map_err(|error| SearchError::Backend(error.to_string()))
    }
}

#[async_trait]
impl SearchSnippetPort for SessionSearch {
    async fn text_snippet(
        &self,
        session_id: SessionId,
        _generation: Generation,
        entity_id: EntityId,
        query: String,
    ) -> Result<Option<String>, SearchError> {
        if session_id != self.session_id {
            return Err(SearchError::InvalidSession);
        }
        if query.trim().is_empty() {
            return Err(SearchError::InvalidQuery);
        }
        let index = Arc::clone(&self.index);
        tokio::task::spawn_blocking(move || index.text_snippet(entity_id, &query))
            .await
            .map_err(|error| SearchError::Backend(error.to_string()))?
            .map_err(|error| SearchError::Backend(error.to_string()))
    }
}

impl SessionIndex {
    pub fn search_page(&self, query: &SearchQuery) -> Result<SearchPage, SessionIndexError> {
        if !query.is_valid() {
            return Err(SessionIndexError::InvalidSearchQuery);
        }
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
            let mut filename_indices = Vec::new();
            let mut path_indices = Vec::new();
            let filename_score = pattern.indices(
                Utf32Str::new(&candidate.name, &mut utf32_buffer),
                &mut filename_matcher,
                &mut filename_indices,
            );
            let path_score = pattern.indices(
                Utf32Str::new(
                    candidate.indexed.node.relative_path.as_str(),
                    &mut utf32_buffer,
                ),
                &mut path_matcher,
                &mut path_indices,
            );
            let exact_filename = !text.is_empty() && candidate.name.to_lowercase() == folded_text;
            let (matched_field, score, match_ranges) = if text.is_empty() {
                (MatchedField::Path, 0, Vec::new())
            } else if exact_filename {
                (
                    MatchedField::ExactFilename,
                    EXACT_FILENAME_BAND,
                    vec![MatchRange {
                        start: 0,
                        end: u32::try_from(candidate.name.chars().count()).unwrap_or(u32::MAX),
                    }],
                )
            } else if let Some(score) = filename_score {
                (
                    MatchedField::Filename,
                    FILENAME_BAND + i64::from(score),
                    indices_to_ranges(filename_indices),
                )
            } else if let Some(score) = path_score {
                (
                    MatchedField::Path,
                    PATH_BAND + i64::from(score),
                    indices_to_ranges(path_indices),
                )
            } else if body_matches.contains(&candidate.indexed.node.entity_id) {
                (MatchedField::Body, BODY_BAND, Vec::new())
            } else {
                continue;
            };
            hits.push(SearchHit {
                node: candidate.indexed.node,
                marker: candidate.indexed.marker,
                image_metadata: candidate.indexed.image_metadata,
                matched_field,
                score,
                group_relative_path: candidate.group_relative_path,
                match_ranges,
            });
        }

        hits.sort_by(|left, right| compare_hits(left, right, query));
        let total = u32::try_from(hits.len()).unwrap_or(u32::MAX);
        let offset = usize::try_from(query.offset).unwrap_or(usize::MAX);
        let limit = usize::try_from(query.limit).unwrap_or(usize::MAX);
        let hits = hits.into_iter().skip(offset).take(limit).collect();
        drop(connection);
        let progress = self.index_progress()?;
        Ok(SearchPage {
            total,
            hits,
            progress,
        })
    }

    pub fn text_snippet(
        &self,
        entity_id: EntityId,
        query: &str,
    ) -> Result<Option<String>, SessionIndexError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(None);
        }
        let connection = self.lock_connection();
        let body = connection
            .query_row(
                "SELECT text_fts.body
                 FROM text_fts
                 JOIN nodes ON nodes.entity_id = text_fts.entity_id
                 WHERE text_fts.entity_id = ?1 AND nodes.kind IN (3, 4)",
                [entity_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(body) = body else {
            return Ok(None);
        };
        let folded_body = body.to_lowercase();
        let folded_query = query.to_lowercase();
        let Some(byte_offset) = folded_body.find(&folded_query) else {
            return Ok(None);
        };
        let safe_byte_offset = if body.is_char_boundary(byte_offset) {
            byte_offset
        } else {
            0
        };
        let matched_scalar = body[..safe_byte_offset].chars().count();
        let start = matched_scalar.saturating_sub(SNIPPET_LEADING_CONTEXT);
        Ok(Some(
            body.chars().skip(start).take(MAX_SNIPPET_SCALARS).collect(),
        ))
    }
}

struct Candidate {
    indexed: IndexedNode,
    name: String,
    group_relative_path: Option<RelativePath>,
}

fn load_candidates(
    connection: &Connection,
    query: &SearchQuery,
) -> Result<Vec<Candidate>, SessionIndexError> {
    let mut sql = String::from(
        "SELECT nodes.entity_id, nodes.relative_path, nodes.kind, nodes.size,
                nodes.modified_ns, nodes.review_state, nodes.favorite,
                nodes.image_width, nodes.image_height, nodes.image_status,
                nodes.text_status, video_metadata.duration_us,
                video_metadata.display_width, video_metadata.display_height,
                video_metadata.rotation_degrees,
                video_metadata.frame_rate_millihertz, video_metadata.video_codec,
                video_metadata.audio_codec, video_metadata.probe_status,
                video_metadata.failure_kind, video_metadata.updated_generation,
                nodes.name
         FROM nodes
         LEFT JOIN video_metadata ON video_metadata.node_id = nodes.entity_id
         WHERE 1 = 1",
    );
    let mut values = Vec::<Value>::new();
    if let SearchScope::Subtree(root) = query.scope {
        let Some(path) = connection
            .query_row(
                "SELECT relative_path FROM nodes WHERE entity_id = ?1 AND kind = 0",
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
    if !query.filters.kinds.is_empty() {
        sql.push_str(" AND kind IN (");
        push_placeholders(&mut sql, query.filters.kinds.len());
        sql.push(')');
        values.extend(
            query
                .filters
                .kinds
                .iter()
                .map(|kind| Value::Integer(encode_kind(*kind))),
        );
    }
    if !query.filters.review_states.is_empty() {
        sql.push_str(" AND review_state IN (");
        push_placeholders(&mut sql, query.filters.review_states.len());
        sql.push(')');
        values.extend(
            query
                .filters
                .review_states
                .iter()
                .map(|state| Value::Integer(encode_review_state(*state))),
        );
    }
    if query.filters.favorite_only {
        sql.push_str(" AND favorite = 1");
    }
    if query.filters.unmarked_only {
        sql.push_str(" AND review_state IS NULL");
    }
    push_range(&mut sql, &mut values, "size", &query.filters.size)?;
    push_range(&mut sql, &mut values, "image_width", &query.filters.width)?;
    push_range(&mut sql, &mut values, "image_height", &query.filters.height)?;
    if !query.filters.orientations.is_empty() {
        sql.push_str(" AND image_width IS NOT NULL AND image_height IS NOT NULL AND (");
        for (index, orientation) in query.filters.orientations.iter().enumerate() {
            if index > 0 {
                sql.push_str(" OR ");
            }
            sql.push_str(match orientation {
                ImageOrientation::Landscape => "image_width > image_height",
                ImageOrientation::Portrait => "image_width < image_height",
                ImageOrientation::Square => "image_width = image_height",
            });
        }
        sql.push(')');
    }
    sql.push_str(" ORDER BY relative_path, entity_id");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values.iter()), |row| {
        let indexed = read_indexed_node(row)?;
        let name = row.get(21)?;
        let group_relative_path = parent_path(&indexed.node.relative_path);
        Ok(Candidate {
            indexed,
            name,
            group_relative_path,
        })
    })?;
    let candidates = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(candidates
        .into_iter()
        .filter(|candidate| {
            query
                .filters
                .modified_ns
                .contains(&candidate.indexed.node.modified_ns)
        })
        .collect())
}

fn push_range<T>(
    sql: &mut String,
    values: &mut Vec<Value>,
    column: &str,
    range: &viewer_domain::search::NumericRange<T>,
) -> Result<(), SessionIndexError>
where
    T: Copy + TryInto<i64> + std::fmt::Display,
{
    if let Some(min) = range.min {
        let value = min
            .try_into()
            .map_err(|_| SessionIndexError::InvalidSearchQuery)?;
        sql.push_str(&format!(" AND {column} >= ?"));
        values.push(Value::Integer(value));
    }
    if let Some(max) = range.max {
        let value = max
            .try_into()
            .map_err(|_| SessionIndexError::InvalidSearchQuery)?;
        sql.push_str(&format!(" AND {column} <= ?"));
        values.push(Value::Integer(value));
    }
    Ok(())
}

fn body_matches(
    connection: &Connection,
    text: &str,
    candidates: &[Candidate],
) -> Result<HashSet<EntityId>, SessionIndexError> {
    if text.is_empty() {
        return Ok(HashSet::new());
    }
    let eligible = candidates.iter().filter(|candidate| {
        matches!(
            candidate.indexed.node.kind,
            FileKind::Markdown | FileKind::Text
        )
    });
    if eligible.clone().next().is_none() {
        return Ok(HashSet::new());
    }
    if text.chars().count() <= 2 {
        let bounded: Vec<_> = eligible
            .take(SHORT_TEXT_CANDIDATE_LIMIT)
            .map(|candidate| candidate.indexed.node.entity_id)
            .collect();
        return short_body_matches(connection, text, &bounded);
    }

    let allowed: HashSet<_> = eligible
        .map(|candidate| candidate.indexed.node.entity_id)
        .collect();
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
    if eligible.is_empty() {
        return Ok(HashSet::new());
    }
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

fn compare_hits(left: &SearchHit, right: &SearchHit, query: &SearchQuery) -> Ordering {
    if query.layout == SearchLayout::Grouped {
        let group = compare_optional_paths(
            left.group_relative_path.as_ref(),
            right.group_relative_path.as_ref(),
        );
        if group != Ordering::Equal {
            return group;
        }
    }
    let ascending = match query.sort.key {
        SearchSortKey::Relevance => right.score.cmp(&left.score),
        SearchSortKey::NaturalName => natural_cmp(file_name(left), file_name(right)),
        SearchSortKey::ModifiedTime => left.node.modified_ns.cmp(&right.node.modified_ns),
        SearchSortKey::Size => left.node.size.cmp(&right.node.size),
        SearchSortKey::PixelDimensions => pixel_count(left).cmp(&pixel_count(right)),
        SearchSortKey::ReviewState => review_rank(left).cmp(&review_rank(right)),
    }
    .then_with(|| {
        natural_cmp(
            left.node.relative_path.as_str(),
            right.node.relative_path.as_str(),
        )
    })
    .then_with(|| {
        left.node
            .entity_id
            .to_string()
            .cmp(&right.node.entity_id.to_string())
    });
    match query.sort.direction {
        SortDirection::Ascending => ascending,
        SortDirection::Descending => ascending.reverse(),
    }
}

fn compare_optional_paths(left: Option<&RelativePath>, right: Option<&RelativePath>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => natural_cmp(left.as_str(), right.as_str()),
    }
}

fn pixel_count(hit: &SearchHit) -> Option<u64> {
    hit.image_metadata
        .map(|metadata| u64::from(metadata.width) * u64::from(metadata.height))
}

fn review_rank(hit: &SearchHit) -> u8 {
    match hit.marker.review_state {
        None => 0,
        Some(ReviewState::Keep) => 1,
        Some(ReviewState::Pending) => 2,
        Some(ReviewState::Reject) => 3,
    }
}

fn file_name(hit: &SearchHit) -> &str {
    hit.node
        .relative_path
        .as_str()
        .rsplit_once('/')
        .map_or(hit.node.relative_path.as_str(), |(_, name)| name)
}

fn parent_path(path: &RelativePath) -> Option<RelativePath> {
    path.as_str()
        .rsplit_once('/')
        .and_then(|(parent, _)| RelativePath::parse(parent).ok())
}

fn indices_to_ranges(mut indices: Vec<u32>) -> Vec<MatchRange> {
    indices.sort_unstable();
    indices.dedup();
    let mut ranges = Vec::new();
    for index in indices {
        match ranges.last_mut() {
            Some(MatchRange { end, .. }) if *end == index => *end = index.saturating_add(1),
            _ => ranges.push(MatchRange {
                start: index,
                end: index.saturating_add(1),
            }),
        }
    }
    ranges
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
