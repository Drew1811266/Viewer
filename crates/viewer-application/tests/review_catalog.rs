use viewer_application::{
    ReviewCatalog, ReviewCatalogError, ReviewProtocolVersion, ReviewRecordLocation,
    ReviewRoundRecord, ReviewStreamHead, ReviewStreamLocator,
};
use viewer_domain::review::{ProductionId, ProductionScope};
use viewer_domain::{ProjectId, ReviewRoundId, ReviewStreamId};

fn production(task_id: &str, batch_id: &str) -> ProductionScope {
    ProductionScope {
        task_id: ProductionId::parse(task_id).unwrap(),
        batch_id: ProductionId::parse(batch_id).unwrap(),
    }
}

fn stream_head(id: u128, task_id: &str, batch_id: &str) -> ReviewStreamHead {
    let review_round_id = ReviewRoundId::from_u128(id + 100);
    ReviewStreamHead {
        review_stream_id: ReviewStreamId::from_u128(id),
        production: Some(production(task_id, batch_id)),
        completed_rounds: vec![ReviewRoundRecord {
            review_round_id,
            protocol_version: ReviewProtocolVersion::V1,
            location: ReviewRecordLocation::new(format!("rounds/{review_round_id}.json")).unwrap(),
            blake3: [0; 32],
        }],
        latest_completed_round_id: Some(review_round_id),
    }
}

fn two_stream_catalog() -> ReviewCatalog {
    ReviewCatalog {
        project_id: ProjectId::from_u128(9),
        streams: vec![
            stream_head(1, "task-a", "batch-a"),
            stream_head(2, "task-b", "batch-b"),
        ],
    }
}

#[test]
fn catalog_never_uses_a_project_global_latest_round() {
    let catalog = two_stream_catalog();
    assert!(matches!(
        catalog.resolve_stream(None),
        Err(ReviewCatalogError::Ambiguous)
    ));
    assert_eq!(
        catalog
            .resolve_stream(Some(
                &ReviewStreamLocator::Id(ReviewStreamId::from_u128(2),)
            ))
            .unwrap()
            .review_stream_id,
        ReviewStreamId::from_u128(2),
    );
}

#[test]
fn production_locator_requires_one_exact_task_and_batch_match() {
    let catalog = two_stream_catalog();
    let locator = ReviewStreamLocator::Production(production("task-b", "batch-b"));
    assert_eq!(
        catalog
            .resolve_stream(Some(&locator))
            .unwrap()
            .review_stream_id,
        ReviewStreamId::from_u128(2)
    );
    assert_eq!(
        catalog.resolve_stream(Some(&ReviewStreamLocator::Production(production(
            "task-b", "batch-a"
        )))),
        Err(ReviewCatalogError::NotFound)
    );
}

#[test]
fn empty_and_duplicate_matching_catalogs_fail_without_guessing() {
    let empty = ReviewCatalog {
        project_id: ProjectId::from_u128(9),
        streams: vec![],
    };
    assert_eq!(
        empty.resolve_stream(None),
        Err(ReviewCatalogError::NotFound)
    );

    let duplicate = ReviewCatalog {
        project_id: ProjectId::from_u128(9),
        streams: vec![
            stream_head(1, "task-a", "batch-a"),
            stream_head(2, "task-a", "batch-a"),
        ],
    };
    assert_eq!(
        duplicate.resolve_stream(Some(&ReviewStreamLocator::Production(production(
            "task-a", "batch-a"
        )))),
        Err(ReviewCatalogError::Ambiguous)
    );
}

#[test]
fn catalog_resolves_round_records_without_exposing_absolute_paths() {
    let record = ReviewRoundRecord {
        review_round_id: ReviewRoundId::from_u128(7),
        protocol_version: ReviewProtocolVersion::V2,
        location: ReviewRecordLocation::new(
            "rounds/00000000-0000-0000-0000-000000000007/round.json",
        )
        .unwrap(),
        blake3: [0x2a; 32],
    };

    assert!(!record.location.as_str().starts_with('/'));
    assert!(ReviewRecordLocation::new("../outside.json").is_err());
    assert!(ReviewRecordLocation::new("rounds\\outside.json").is_err());
}
