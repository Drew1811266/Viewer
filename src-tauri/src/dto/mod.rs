mod browse;
mod markers;
mod operations;
mod preview;
mod project;
mod search;
mod settings;

pub use browse::*;
pub use markers::*;
pub use operations::*;
pub use preview::*;
pub use project::*;
pub use search::*;
pub use settings::*;

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_application::browse::{SelectionAgreement, SelectionInfo, SelectionTypeCounts};
    use viewer_domain::RelativePath;

    #[test]
    fn failed_scan_item_display_never_exposes_an_absolute_or_reserved_path() {
        assert_eq!(
            browse::safe_relative_display("catalog/id-1"),
            "catalog/id-1"
        );
        for unsafe_value in ["/Users/example/secret", "../outside", ".viewer/index"] {
            assert_eq!(browse::safe_relative_display(unsafe_value), "unavailable");
        }
    }

    #[test]
    fn selection_dto_keeps_unmarked_distinct_from_mixed_and_exposes_relative_paths_only() {
        let dto = SelectionInfoDto::from(SelectionInfo {
            relative_paths: vec![RelativePath::parse("catalog/id-2/image.jpg").unwrap()],
            total_size: 42,
            types: SelectionTypeCounts {
                folders: 0,
                images: 1,
                videos: 0,
                other_files: 0,
            },
            common_review: SelectionAgreement::Common(None),
            common_favorite: SelectionAgreement::Mixed,
        });

        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["relativePaths"][0], "catalog/id-2/image.jpg");
        assert_eq!(json["commonReview"]["state"], "common");
        assert!(json["commonReview"]["value"].is_null());
        assert_eq!(json["commonFavorite"]["state"], "mixed");
    }

    #[test]
    fn m2_command_requests_accept_only_the_frozen_camel_case_shape() {
        let request: SearchProjectRequestDto = serde_json::from_value(serde_json::json!({
            "sessionId": "00000000-0000-0000-0000-000000000001",
            "generation": 3,
            "revision": 7,
            "text": "鞋",
            "scopeFolderId": null,
            "filters": {
                "kinds": ["jpeg"],
                "reviewStates": ["keep"],
                "favoriteOnly": true,
                "modifiedNsMin": "123"
            },
            "sort": { "key": "natural_name", "direction": "ascending" },
            "layout": "flat",
            "offset": 0,
            "limit": 200
        }))
        .unwrap();
        assert_eq!(request.generation, 3);
        assert_eq!(request.revision, 7);
        assert_eq!(request.filters.modified_ns_min.as_deref(), Some("123"));

        let invalid = serde_json::from_value::<SetReviewStateRequestDto>(serde_json::json!({
            "session_id": "not-camel-case",
            "generation": 1,
            "entityIds": [],
            "reviewState": null
        }));
        assert!(invalid.is_err());
    }
}
