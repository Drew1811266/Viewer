use std::sync::Arc;
use viewer_application::{
    SearchPort, SearchSnippetPort,
    scheduler::TaskCoordinator,
    search::{CoordinatedSnippet, SearchError},
};
use viewer_domain::{
    EntityId, RelativePath, SessionId,
    file::{FileKind, FileNode, ImageMetadata, ReviewState},
    search::{
        Generation, ImageOrientation, MatchedField, NumericRange, SearchFilters, SearchLayout,
        SearchQuery, SearchScope, SearchSort, SearchSortKey, SortDirection,
    },
};
use viewer_infrastructure::search::{index::SessionIndex, query::SessionSearch, text::TextStatus};

struct Fixture {
    _directory: tempfile::TempDir,
    index: Arc<SessionIndex>,
    session: SessionId,
    id10: EntityId,
    front: EntityId,
    portrait: EntityId,
    square: EntityId,
    note: EntityId,
    other: EntityId,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let index = Arc::new(SessionIndex::open(directory.path().join("session.sqlite")).unwrap());
        let root = EntityId::new();
        let id2 = EntityId::new();
        let id10 = EntityId::new();
        let front = EntityId::new();
        let portrait = EntityId::new();
        let image2 = EntityId::new();
        let image10 = EntityId::new();
        let note = EntityId::new();
        let other = EntityId::new();
        index
            .upsert_batch(
                &[
                    node(root, "catalog", FileKind::Directory, 0, 1),
                    node(id2, "catalog/id2", FileKind::Directory, 0, 2),
                    node(id10, "catalog/id10", FileKind::Directory, 0, 3),
                    node(front, "catalog/id2/正面-front.png", FileKind::Png, 400, 40),
                    node(
                        portrait,
                        "catalog/id2/portrait.jpg",
                        FileKind::Jpeg,
                        300,
                        30,
                    ),
                    node(image2, "catalog/id10/image2.jpg", FileKind::Jpeg, 200, 20),
                    node(image10, "catalog/id10/image10.jpg", FileKind::Jpeg, 100, 10),
                    node(note, "catalog/id2/prompt.md", FileKind::Markdown, 500, 50),
                    node(other, "catalog/id2/guide.pdf", FileKind::Other, 600, 60),
                ],
                Generation::new(1),
            )
            .unwrap();
        index
            .replace_image_metadata(
                front,
                &RelativePath::parse("catalog/id2/正面-front.png").unwrap(),
                Ok(ImageMetadata {
                    width: 2_000,
                    height: 1_000,
                }),
            )
            .unwrap();
        index
            .replace_image_metadata(
                portrait,
                &RelativePath::parse("catalog/id2/portrait.jpg").unwrap(),
                Ok(ImageMetadata {
                    width: 800,
                    height: 1_200,
                }),
            )
            .unwrap();
        index
            .replace_image_metadata(
                image2,
                &RelativePath::parse("catalog/id10/image2.jpg").unwrap(),
                Ok(ImageMetadata {
                    width: 500,
                    height: 500,
                }),
            )
            .unwrap();
        index
            .replace_text(
                note,
                &RelativePath::parse("catalog/id2/prompt.md").unwrap(),
                &TextStatus::Indexed(format!("白色陶瓷杯 正面产品说明 {}", "备注".repeat(100))),
            )
            .unwrap();
        index
            .replace_text(
                other,
                &RelativePath::parse("catalog/id2/guide.pdf").unwrap(),
                &TextStatus::Indexed("other file body must not be searchable".into()),
            )
            .unwrap();
        index
            .set_review_metadata(front, Some(ReviewState::Keep), true)
            .unwrap();
        index
            .set_review_metadata(portrait, Some(ReviewState::Reject), false)
            .unwrap();
        Self {
            _directory: directory,
            index,
            session: SessionId::new(),
            id10,
            front,
            portrait,
            square: image2,
            note,
            other,
        }
    }

    fn search(&self) -> SessionSearch {
        SessionSearch::new(self.session, Arc::clone(&self.index))
    }
}

#[tokio::test]
async fn generic_other_files_match_filename_and_path_but_not_body() {
    let fixture = Fixture::new();
    let search = fixture.search();

    let filename = search
        .search(fixture.session, Generation::new(1), query("guide"))
        .await
        .unwrap();
    assert_eq!(filename.hits[0].node.entity_id, fixture.other);
    assert_eq!(filename.hits[0].matched_field, MatchedField::Filename);

    let path = search
        .search(fixture.session, Generation::new(1), query("id2guide"))
        .await
        .unwrap();
    assert_eq!(path.hits[0].node.entity_id, fixture.other);
    assert_eq!(path.hits[0].matched_field, MatchedField::Path);

    let body = search
        .search(
            fixture.session,
            Generation::new(1),
            query("mustnotbesearchable"),
        )
        .await
        .unwrap();
    assert!(body.hits.is_empty());
}

fn node(entity_id: EntityId, path: &str, kind: FileKind, size: u64, modified_ns: i128) -> FileNode {
    FileNode {
        entity_id,
        relative_path: RelativePath::parse(path).unwrap(),
        kind,
        size,
        modified_ns,
    }
}

fn query(text: &str) -> SearchQuery {
    SearchQuery {
        text: text.into(),
        scope: SearchScope::Project,
        filters: SearchFilters::default(),
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
        limit: 200,
    }
}

#[tokio::test]
async fn unicode_name_path_and_body_search_return_highlights_without_body_content() {
    let fixture = Fixture::new();
    let search = fixture.search();
    let filename = search
        .search(fixture.session, Generation::new(1), query("正面front"))
        .await
        .unwrap();
    assert_eq!(filename.hits[0].node.entity_id, fixture.front);
    assert_eq!(filename.hits[0].matched_field, MatchedField::Filename);
    assert!(!filename.hits[0].match_ranges.is_empty());

    let path = search
        .search(fixture.session, Generation::new(1), query("id10image2"))
        .await
        .unwrap();
    assert_eq!(path.hits[0].matched_field, MatchedField::Path);
    let body = search
        .search(fixture.session, Generation::new(1), query("陶瓷杯"))
        .await
        .unwrap();
    assert_eq!(body.hits[0].node.entity_id, fixture.note);
    assert_eq!(body.hits[0].matched_field, MatchedField::Body);
    assert!(body.hits[0].match_ranges.is_empty());
    let serialized = serde_json::to_string(&body).unwrap();
    assert!(!serialized.contains("白色陶瓷杯 正面产品说明"));
}

#[tokio::test]
async fn filter_categories_or_within_and_and_between_categories() {
    let fixture = Fixture::new();
    let search = fixture.search();
    let mut request = query("");
    request.filters = SearchFilters {
        kinds: vec![FileKind::Jpeg, FileKind::Png],
        review_states: vec![ReviewState::Keep, ReviewState::Pending],
        favorite_only: true,
        orientations: vec![ImageOrientation::Landscape, ImageOrientation::Square],
        width: NumericRange {
            min: Some(1_000),
            max: Some(3_000),
        },
        height: NumericRange {
            min: None,
            max: Some(1_000),
        },
        size: NumericRange {
            min: Some(350),
            max: Some(450),
        },
        modified_ns: NumericRange {
            min: Some(35),
            max: Some(45),
        },
        ..SearchFilters::default()
    };
    let filtered = search
        .search(fixture.session, Generation::new(1), request)
        .await
        .unwrap();
    assert_eq!(filtered.hits.len(), 1);
    assert_eq!(filtered.hits[0].node.entity_id, fixture.front);
    assert!(filtered.hits[0].marker.favorite);
    assert_eq!(filtered.hits[0].image_metadata.unwrap().width, 2_000);

    let mut unmarked = query("");
    unmarked.filters.kinds = vec![FileKind::Jpeg, FileKind::Png];
    unmarked.filters.unmarked_only = true;
    let unmarked = search
        .search(fixture.session, Generation::new(1), unmarked)
        .await
        .unwrap();
    assert!(
        unmarked
            .hits
            .iter()
            .all(|hit| hit.marker.review_state.is_none())
    );
    assert!(
        !unmarked
            .hits
            .iter()
            .any(|hit| hit.node.entity_id == fixture.front)
    );
    assert!(
        !unmarked
            .hits
            .iter()
            .any(|hit| hit.node.entity_id == fixture.portrait)
    );
}

#[tokio::test]
async fn every_filter_dimension_is_enforced_independently() {
    let fixture = Fixture::new();
    let search = fixture.search();
    let cases = [
        (
            SearchFilters {
                kinds: vec![FileKind::Png],
                ..SearchFilters::default()
            },
            fixture.front,
        ),
        (
            SearchFilters {
                review_states: vec![ReviewState::Reject],
                ..SearchFilters::default()
            },
            fixture.portrait,
        ),
        (
            SearchFilters {
                kinds: vec![FileKind::Jpeg, FileKind::Png],
                favorite_only: true,
                ..SearchFilters::default()
            },
            fixture.front,
        ),
        (
            SearchFilters {
                kinds: vec![FileKind::Jpeg, FileKind::Png],
                orientations: vec![ImageOrientation::Portrait],
                ..SearchFilters::default()
            },
            fixture.portrait,
        ),
        (
            SearchFilters {
                kinds: vec![FileKind::Jpeg, FileKind::Png],
                orientations: vec![ImageOrientation::Square],
                ..SearchFilters::default()
            },
            fixture.square,
        ),
        (
            SearchFilters {
                kinds: vec![FileKind::Jpeg, FileKind::Png],
                width: NumericRange {
                    min: Some(1_500),
                    max: None,
                },
                ..SearchFilters::default()
            },
            fixture.front,
        ),
        (
            SearchFilters {
                kinds: vec![FileKind::Jpeg, FileKind::Png],
                height: NumericRange {
                    min: Some(1_100),
                    max: None,
                },
                ..SearchFilters::default()
            },
            fixture.portrait,
        ),
        (
            SearchFilters {
                kinds: vec![FileKind::Jpeg, FileKind::Png],
                size: NumericRange {
                    min: Some(250),
                    max: Some(350),
                },
                ..SearchFilters::default()
            },
            fixture.portrait,
        ),
        (
            SearchFilters {
                kinds: vec![FileKind::Jpeg, FileKind::Png],
                modified_ns: NumericRange {
                    min: Some(29),
                    max: Some(31),
                },
                ..SearchFilters::default()
            },
            fixture.portrait,
        ),
    ];
    for (filters, expected) in cases {
        let mut request = query("");
        request.filters = filters;
        let page = search
            .search(fixture.session, Generation::new(1), request)
            .await
            .unwrap();
        assert_eq!(page.hits.len(), 1);
        assert_eq!(page.hits[0].node.entity_id, expected);
    }
}

#[tokio::test]
async fn subtree_scope_and_every_sort_key_are_stable_in_both_directions() {
    let fixture = Fixture::new();
    let search = fixture.search();
    let mut scoped = query("");
    scoped.scope = SearchScope::Subtree(fixture.id10);
    scoped.filters.kinds = vec![FileKind::Jpeg];
    let natural = search
        .search(fixture.session, Generation::new(1), scoped.clone())
        .await
        .unwrap();
    assert_eq!(
        natural
            .hits
            .iter()
            .map(|hit| hit.node.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["catalog/id10/image2.jpg", "catalog/id10/image10.jpg"]
    );

    for key in [
        SearchSortKey::ModifiedTime,
        SearchSortKey::Size,
        SearchSortKey::PixelDimensions,
        SearchSortKey::ReviewState,
    ] {
        let mut ascending = query("");
        ascending.filters.kinds = vec![FileKind::Jpeg, FileKind::Png];
        ascending.sort = SearchSort {
            key,
            direction: SortDirection::Ascending,
        };
        let forward = search
            .search(fixture.session, Generation::new(1), ascending.clone())
            .await
            .unwrap();
        ascending.sort.direction = SortDirection::Descending;
        let reverse = search
            .search(fixture.session, Generation::new(1), ascending)
            .await
            .unwrap();
        let forward_ids = forward
            .hits
            .iter()
            .map(|hit| hit.node.entity_id)
            .collect::<Vec<_>>();
        let mut reverse_ids = reverse
            .hits
            .iter()
            .map(|hit| hit.node.entity_id)
            .collect::<Vec<_>>();
        reverse_ids.reverse();
        assert_eq!(forward_ids, reverse_ids, "sort {key:?}");
        let expected = match key {
            SearchSortKey::ModifiedTime | SearchSortKey::Size => vec![
                "catalog/id10/image10.jpg",
                "catalog/id10/image2.jpg",
                "catalog/id2/portrait.jpg",
                "catalog/id2/正面-front.png",
            ],
            SearchSortKey::PixelDimensions => vec![
                "catalog/id10/image10.jpg",
                "catalog/id10/image2.jpg",
                "catalog/id2/portrait.jpg",
                "catalog/id2/正面-front.png",
            ],
            SearchSortKey::ReviewState => vec![
                "catalog/id10/image2.jpg",
                "catalog/id10/image10.jpg",
                "catalog/id2/正面-front.png",
                "catalog/id2/portrait.jpg",
            ],
            _ => unreachable!(),
        };
        assert_eq!(
            forward
                .hits
                .iter()
                .map(|hit| hit.node.relative_path.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[tokio::test]
async fn grouped_layout_orders_groups_naturally_while_flat_layout_sorts_globally() {
    let fixture = Fixture::new();
    let search = fixture.search();
    let mut grouped = query("");
    grouped.filters.kinds = vec![FileKind::Jpeg, FileKind::Png];
    grouped.layout = SearchLayout::Grouped;
    grouped.sort = SearchSort {
        key: SearchSortKey::Size,
        direction: SortDirection::Descending,
    };
    let grouped_page = search
        .search(fixture.session, Generation::new(1), grouped.clone())
        .await
        .unwrap();
    assert_eq!(
        grouped_page.hits[0]
            .group_relative_path
            .as_ref()
            .unwrap()
            .as_str(),
        "catalog/id2"
    );
    assert_eq!(
        grouped_page
            .hits
            .last()
            .unwrap()
            .group_relative_path
            .as_ref()
            .unwrap()
            .as_str(),
        "catalog/id10"
    );

    grouped.layout = SearchLayout::Flat;
    let flat = search
        .search(fixture.session, Generation::new(1), grouped)
        .await
        .unwrap();
    assert_eq!(flat.hits[0].node.entity_id, fixture.front);
}

#[tokio::test]
async fn pages_are_capped_report_partial_indexing_and_snippets_are_separate_and_bounded() {
    let fixture = Fixture::new();
    let search = fixture.search();
    let mut invalid = query("");
    invalid.limit = 201;
    assert!(
        search
            .search(fixture.session, Generation::new(1), invalid)
            .await
            .is_err()
    );

    let page = search
        .search(fixture.session, Generation::new(1), query("陶瓷杯"))
        .await
        .unwrap();
    assert!(!page.progress.is_complete());
    let snippet = search
        .text_snippet(
            fixture.session,
            Generation::new(1),
            fixture.note,
            "陶瓷杯".into(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(snippet.chars().count() <= 160);
    assert!(snippet.contains("陶瓷杯"));
    assert_eq!(
        search
            .text_snippet(
                fixture.session,
                Generation::new(1),
                fixture.note,
                "不存在".into(),
            )
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn coordinated_snippets_reject_stale_project_generations() {
    let fixture = Fixture::new();
    let coordinator = Arc::new(TaskCoordinator::default());
    let current = coordinator.begin_session(fixture.session);
    let snippet = CoordinatedSnippet::new(Arc::new(fixture.search()), Arc::clone(&coordinator));
    assert!(
        snippet
            .text_snippet(fixture.session, current, fixture.note, "陶瓷杯".into(),)
            .await
            .unwrap()
            .is_some()
    );
    coordinator.bump_generation(fixture.session).unwrap();
    assert!(matches!(
        snippet
            .text_snippet(fixture.session, current, fixture.note, "陶瓷杯".into(),)
            .await,
        Err(SearchError::Stale)
    ));
}
