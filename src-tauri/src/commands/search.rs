use std::{str::FromStr, sync::Arc};

use tauri::State;
use viewer_domain::{
    SessionId,
    search::{Generation, NumericRange, SearchFilters, SearchQuery, SearchScope, SearchSort},
};

use crate::{
    commands::browse::parse_entity_id,
    dto::{SearchPageDto, SearchProjectRequestDto, SearchTextSnippetRequestDto, TextSnippetDto},
    error::{CommandError, ErrorCategory},
    state::DesktopRuntime,
};

#[tauri::command]
pub async fn search_project(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: SearchProjectRequestDto,
) -> Result<SearchPageDto, CommandError> {
    let session = parse_session_id(&request.session_id)?;
    let generation = Generation::new(request.generation);
    let revision = request.revision;
    let query = search_query(request)?;
    runtime
        .search_project(session, generation, revision, query)
        .await
}

#[tauri::command]
pub async fn search_text_snippet(
    runtime: State<'_, Arc<DesktopRuntime>>,
    request: SearchTextSnippetRequestDto,
) -> Result<TextSnippetDto, CommandError> {
    runtime
        .search_text_snippet(
            parse_session_id(&request.session_id)?,
            Generation::new(request.generation),
            request.revision,
            parse_entity_id(&request.entity_id)?,
            request.query,
        )
        .await
}

pub(crate) fn parse_session_id(value: &str) -> Result<SessionId, CommandError> {
    SessionId::from_str(value).map_err(|_| invalid_search_request())
}

fn search_query(request: SearchProjectRequestDto) -> Result<SearchQuery, CommandError> {
    let scope = request
        .scope_folder_id
        .as_deref()
        .map(parse_entity_id)
        .transpose()?
        .map_or(SearchScope::Project, SearchScope::Subtree);
    let modified_ns = NumericRange {
        min: parse_optional_i128(request.filters.modified_ns_min.as_deref())?,
        max: parse_optional_i128(request.filters.modified_ns_max.as_deref())?,
    };
    Ok(SearchQuery {
        text: request.text,
        scope,
        filters: SearchFilters {
            kinds: request.filters.kinds,
            review_states: request.filters.review_states,
            favorite_only: request.filters.favorite_only,
            unmarked_only: request.filters.unmarked_only,
            orientations: request.filters.orientations,
            width: NumericRange {
                min: request.filters.width_min,
                max: request.filters.width_max,
            },
            height: NumericRange {
                min: request.filters.height_min,
                max: request.filters.height_max,
            },
            size: NumericRange {
                min: request.filters.size_min,
                max: request.filters.size_max,
            },
            modified_ns,
        },
        sort: SearchSort {
            key: request.sort.key,
            direction: request.sort.direction,
        },
        layout: request.layout,
        offset: request.offset,
        limit: request.limit,
    })
}

fn parse_optional_i128(value: Option<&str>) -> Result<Option<i128>, CommandError> {
    value
        .map(|value| value.parse::<i128>().map_err(|_| invalid_search_request()))
        .transpose()
}

fn invalid_search_request() -> CommandError {
    CommandError::new(
        "invalid_search_query",
        ErrorCategory::Validation,
        "搜索条件无效，请调整后重试。",
        false,
    )
}
