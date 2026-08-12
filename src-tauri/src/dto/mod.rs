mod browse;
mod markers;
mod operations;
mod preview;
mod project;
mod search;
mod settings;
mod video;

pub use browse::*;
pub use markers::*;
pub use operations::*;
pub use preview::*;
pub use project::*;
pub use search::*;
pub use settings::*;
pub use video::*;

#[cfg(test)]
mod tests {
    use super::*;
    use viewer_application::browse::{
        BrowserFile, ContentFolderCard, FolderReviewProgress, FolderWorkspace, SelectionAgreement,
        SelectionInfo, SelectionTypeCounts,
    };
    use viewer_domain::{
        EntityId, RelativePath,
        file::{FileKind, Marker},
        video::{VideoMetadata, VideoProbeStatus},
    };

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
    fn task_9_video_dto_deferral_is_explicit_lossless_in_rust_and_absent_from_json() {
        let metadata = VideoMetadata {
            duration_us: Some(2_000_000),
            display_width: Some(1_920),
            display_height: Some(1_080),
            rotation_degrees: 0,
            frame_rate_millihertz: Some(30_000),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            probe_status: VideoProbeStatus::Ready,
        };
        let video = BrowserFile {
            entity_id: EntityId::from_u128(9),
            relative_path: RelativePath::parse("clip.mp4").unwrap(),
            name: "clip.mp4".to_owned(),
            kind: FileKind::Video,
            size: 42,
            modified_ns: 7,
            marker: Marker::default(),
            image_metadata: None,
            video_metadata: Some(metadata.clone()),
        };
        let workspace = FolderWorkspaceDto::from(FolderWorkspace::Content {
            images: Vec::new(),
            videos: vec![video],
            other_files: Vec::new(),
        });
        let FolderWorkspaceDto::Content {
            videos_deferred_until_task_9,
            ..
        } = &workspace
        else {
            panic!("workspace should remain content")
        };
        assert_eq!(videos_deferred_until_task_9.len(), 1);
        assert_eq!(
            videos_deferred_until_task_9[0].video_metadata,
            Some(metadata)
        );
        let workspace_json = serde_json::to_value(&workspace).unwrap();
        assert!(workspace_json.get("videos").is_none());
        assert!(workspace_json.get("videosDeferredUntilTask9").is_none());

        let folder = ContentFolderCardDto::from(ContentFolderCard {
            entity_id: EntityId::from_u128(10),
            relative_path: RelativePath::parse("catalog").unwrap(),
            name: "catalog".to_owned(),
            marker: Marker::default(),
            image_count: 0,
            video_count: 3,
            other_file_count: 0,
            review_progress: FolderReviewProgress::default(),
            representative_images: Vec::new(),
        });
        assert_eq!(folder.video_count_deferred_until_task_9, 3);
        assert!(
            serde_json::to_value(folder)
                .unwrap()
                .get("videoCount")
                .is_none()
        );

        let counts = SelectionTypeCountsDto::from(SelectionTypeCounts {
            folders: 0,
            images: 0,
            videos: 2,
            other_files: 0,
        });
        assert_eq!(counts.videos_deferred_until_task_9, 2);
        assert!(
            serde_json::to_value(counts)
                .unwrap()
                .get("videos")
                .is_none()
        );
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
