use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension};
use std::{fs, sync::Arc};
use viewer_application::{
    SearchPort,
    scheduler::TaskCoordinator,
    search::{CoordinatedSearch, SearchError},
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode, ReviewState},
    search::{
        Generation, MatchedField, SearchFilters, SearchLayout, SearchQuery, SearchScope,
        SearchSort, SearchSortKey, SortDirection,
    },
};
use viewer_infrastructure::search::{
    index::SessionIndex,
    query::SessionSearch,
    text::{TextExtractor, TextStatus},
};

#[test]
fn session_index_keeps_its_public_path() {
    fn index(_: Option<viewer_infrastructure::search::index::SessionIndex>) {}
    index(None);
}

#[test]
fn text_index_extracts_utf8_bom_empty_and_normalizes_newlines() {
    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path().join("plain.txt");
    let bom = directory.path().join("bom.md");
    let empty = directory.path().join("empty.txt");
    fs::write(&plain, "第一行\r\nsecond\rline").unwrap();
    fs::write(&bom, b"\xef\xbb\xbfproduct prompt").unwrap();
    fs::write(&empty, b"").unwrap();

    assert_eq!(
        TextExtractor::extract(&plain).unwrap(),
        TextStatus::Indexed("第一行\nsecond\nline".into())
    );
    assert_eq!(
        TextExtractor::extract(&bom).unwrap(),
        TextStatus::Indexed("product prompt".into())
    );
    assert_eq!(
        TextExtractor::extract(&empty).unwrap(),
        TextStatus::Indexed(String::new())
    );
}

#[test]
fn text_index_isolates_an_unsupported_encoding() {
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("invalid.txt");
    let valid = directory.path().join("valid.txt");
    fs::write(&invalid, [0xff, 0xfe, 0xfd]).unwrap();
    fs::write(&valid, "仍可索引").unwrap();

    assert_eq!(
        TextExtractor::extract(&invalid).unwrap(),
        TextStatus::UnsupportedEncoding
    );
    assert_eq!(
        TextExtractor::extract(&valid).unwrap(),
        TextStatus::Indexed("仍可索引".into())
    );
}

#[test]
fn text_index_skips_files_larger_than_ten_mib() {
    let directory = tempfile::tempdir().unwrap();
    let oversized = directory.path().join("oversized.md");
    fs::write(&oversized, vec![b'a'; 11 * 1024 * 1024]).unwrap();

    assert_eq!(
        TextExtractor::extract(&oversized).unwrap(),
        TextStatus::TooLarge
    );
}

#[test]
fn text_index_replacement_is_atomic_and_removes_stale_rows() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("session.sqlite");
    let entity_id = EntityId::new();
    let relative_path = RelativePath::parse("notes.txt").unwrap();
    let index = SessionIndex::open(&database).unwrap();
    index
        .upsert_batch(&[FileNode {
            entity_id,
            relative_path: relative_path.clone(),
            kind: FileKind::Text,
            size: 5,
            modified_ns: 10,
        }])
        .unwrap();
    index
        .replace_text(
            entity_id,
            &relative_path,
            &TextStatus::Indexed("产品说明".into()),
        )
        .unwrap();
    index.close().unwrap();

    let connection = Connection::open(&database).unwrap();
    let body = connection
        .query_row(
            "SELECT body FROM text_fts WHERE entity_id = ?1",
            [entity_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(body, "产品说明");
    drop(connection);

    let index = SessionIndex::open(&database).unwrap();
    index
        .replace_text(entity_id, &relative_path, &TextStatus::TooLarge)
        .unwrap();
    index.close().unwrap();
    let connection = Connection::open(&database).unwrap();
    let stale_body = connection
        .query_row(
            "SELECT body FROM text_fts WHERE entity_id = ?1",
            [entity_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap();
    assert_eq!(stale_body, None);
}

struct SearchFixture {
    _directory: tempfile::TempDir,
    index: Arc<SessionIndex>,
    session_id: SessionId,
    product_a: EntityId,
    front: EntityId,
}

impl SearchFixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("session.sqlite");
        let index = Arc::new(SessionIndex::open(&database).unwrap());
        let product_a = EntityId::new();
        let front = EntityId::new();
        let side = EntityId::new();
        let path_only = EntityId::new();
        let explanation = EntityId::new();
        let notes = EntityId::new();
        index
            .upsert_batch(&[
                search_node(product_a, "产品-A", FileKind::Directory),
                search_node(EntityId::new(), "front-set", FileKind::Directory),
                search_node(front, "产品-A/front.png", FileKind::Png),
                search_node(side, "产品-A/side.png", FileKind::Png),
                search_node(path_only, "front-set/angle.png", FileKind::Png),
                search_node(explanation, "产品说明.md", FileKind::Markdown),
                search_node(notes, "notes.txt", FileKind::Text),
            ])
            .unwrap();
        index
            .replace_text(
                explanation,
                &RelativePath::parse("产品说明.md").unwrap(),
                &TextStatus::Indexed("白色陶瓷杯，正面产品图".into()),
            )
            .unwrap();
        index
            .replace_text(
                notes,
                &RelativePath::parse("notes.txt").unwrap(),
                &TextStatus::Indexed("普通备注".into()),
            )
            .unwrap();
        index
            .set_review_metadata(front, Some(ReviewState::Keep), true)
            .unwrap();
        index
            .set_review_metadata(side, Some(ReviewState::Reject), false)
            .unwrap();
        Self {
            _directory: directory,
            index,
            session_id: SessionId::new(),
            product_a,
            front,
        }
    }

    fn search(&self) -> SessionSearch {
        SessionSearch::new(self.session_id, Arc::clone(&self.index))
    }
}

#[tokio::test]
async fn ranking_prefers_an_exact_or_filename_match_over_a_path_match() {
    let fixture = SearchFixture::new();
    let search = fixture.search();
    let exact = search
        .search(
            fixture.session_id,
            Generation::new(1),
            query("front.png", SearchScope::Project, vec![FileKind::Png]),
        )
        .await
        .unwrap();
    assert_eq!(exact.hits[0].node.entity_id, fixture.front);
    assert_eq!(exact.hits[0].matched_field, MatchedField::ExactFilename);

    let fuzzy = search
        .search(
            fixture.session_id,
            Generation::new(1),
            query("front", SearchScope::Project, vec![FileKind::Png]),
        )
        .await
        .unwrap();
    assert_eq!(fuzzy.hits[0].node.entity_id, fixture.front);
    assert_eq!(fuzzy.hits[0].matched_field, MatchedField::Filename);
    assert!(fuzzy.hits.iter().any(|hit| {
        hit.node.relative_path.as_str() == "front-set/angle.png"
            && hit.matched_field == MatchedField::Path
    }));
}

#[tokio::test]
async fn cjk_search_combines_trigram_and_bounded_short_query_fallback() {
    let fixture = SearchFixture::new();
    let search = fixture.search();
    for text in ["产品图", "陶瓷杯", "白色"] {
        let page = search
            .search(
                fixture.session_id,
                Generation::new(2),
                query(text, SearchScope::Project, vec![FileKind::Markdown]),
            )
            .await
            .unwrap();
        assert_eq!(page.hits.len(), 1, "query {text}");
        assert_eq!(page.hits[0].node.relative_path.as_str(), "产品说明.md");
        assert_eq!(page.hits[0].matched_field, MatchedField::Body);
    }
}

#[tokio::test]
async fn scope_and_filters_are_applied_before_search_scoring() {
    let fixture = SearchFixture::new();
    let search = fixture.search();
    let mut filtered = query(
        "",
        SearchScope::Subtree(fixture.product_a),
        vec![FileKind::Png],
    );
    filtered.filters.review_states = vec![ReviewState::Keep];
    filtered.filters.favorite_only = true;
    let page = search
        .search(fixture.session_id, Generation::new(3), filtered)
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.hits[0].node.entity_id, fixture.front);

    let subtree = search
        .search(
            fixture.session_id,
            Generation::new(3),
            query(
                "",
                SearchScope::Subtree(fixture.product_a),
                vec![FileKind::Png],
            ),
        )
        .await
        .unwrap();
    assert_eq!(subtree.total, 2);
    assert!(
        subtree
            .hits
            .iter()
            .all(|hit| hit.node.relative_path.as_str().starts_with("产品-A/"))
    );
    assert!(
        subtree.hits[0].node.relative_path.as_str() < subtree.hits[1].node.relative_path.as_str()
    );
}

#[tokio::test]
async fn search_rejects_a_different_project_session() {
    let fixture = SearchFixture::new();
    let result = fixture
        .search()
        .search(
            SessionId::new(),
            Generation::new(1),
            query("front", SearchScope::Project, vec![]),
        )
        .await;
    assert!(matches!(result, Err(SearchError::InvalidSession)));
}

fn query(text: &str, scope: SearchScope, kinds: Vec<FileKind>) -> SearchQuery {
    SearchQuery {
        text: text.into(),
        scope,
        filters: SearchFilters {
            kinds,
            ..SearchFilters::default()
        },
        sort: SearchSort {
            key: if text.is_empty() {
                SearchSortKey::NaturalName
            } else {
                SearchSortKey::Relevance
            },
            direction: SortDirection::Ascending,
        },
        layout: SearchLayout::Flat,
        offset: 0,
        limit: 100,
    }
}

fn search_node(entity_id: EntityId, path: &str, kind: FileKind) -> FileNode {
    FileNode {
        entity_id,
        relative_path: RelativePath::parse(path).unwrap(),
        kind,
        size: if kind == FileKind::Directory { 0 } else { 10 },
        modified_ns: 20,
    }
}

struct BarrierSearch {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl SearchPort for BarrierSearch {
    async fn search(
        &self,
        _session_id: SessionId,
        _generation: Generation,
        query: SearchQuery,
    ) -> Result<viewer_domain::search::SearchPage, SearchError> {
        if query.text == "old" {
            self.started.notify_one();
            self.release.notified().await;
        }
        Ok(viewer_domain::search::SearchPage {
            total: 0,
            hits: Vec::new(),
            progress: viewer_domain::search::IndexProgress::default(),
        })
    }
}

#[tokio::test]
async fn stale_search_generation_never_reaches_the_result_sink() {
    let coordinator = Arc::new(TaskCoordinator::default());
    let session_id = SessionId::new();
    let old_generation = coordinator.begin_session(session_id);
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let search = Arc::new(CoordinatedSearch::new(
        Arc::new(BarrierSearch {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        }),
        Arc::clone(&coordinator),
    ));
    let (results, mut published) = tokio::sync::mpsc::channel(2);
    let old_search = Arc::clone(&search);
    let old_results = results.clone();
    let old_task = tokio::spawn(async move {
        let result = old_search
            .search(
                session_id,
                old_generation,
                query("old", SearchScope::Project, vec![]),
            )
            .await;
        if result.is_ok() {
            old_results.send(old_generation).await.unwrap();
        }
        result
    });
    started.notified().await;

    let current_generation = coordinator.bump_generation(session_id).unwrap();
    search
        .search(
            session_id,
            current_generation,
            query("new", SearchScope::Project, vec![]),
        )
        .await
        .unwrap();
    results.send(current_generation).await.unwrap();
    release.notify_one();
    assert!(matches!(old_task.await.unwrap(), Err(SearchError::Stale)));
    drop(search);
    drop(results);

    let mut generations = Vec::new();
    while let Some(generation) = published.recv().await {
        generations.push(generation);
    }
    assert_eq!(generations, [current_generation]);
}
