use viewer_application::{
    TimelineThumbnailRequest, TimelineThumbnailResult, VideoThumbnailService,
};
use viewer_domain::{VideoSessionId, VideoThumbnailRequestId};

#[test]
fn a_new_hover_replaces_the_only_pending_request() {
    let service = VideoThumbnailService::default();
    let session_id = VideoSessionId::from_u128(1);
    service.activate(session_id, 7);

    let old = service.request(500_000).unwrap();
    let new = service.request(1_000_000).unwrap();

    assert_ne!(old.request_id, new.request_id);
    assert_eq!(service.pending(), Some(new));
}

#[test]
fn only_the_exact_newest_result_can_publish() {
    let service = VideoThumbnailService::default();
    let session_id = VideoSessionId::from_u128(1);
    service.activate(session_id, 7);
    let old = service.request(500_000).unwrap();
    let current = service.request(1_000_000).unwrap();

    assert_eq!(service.publish(result(old, "old")), None);
    assert_eq!(
        service.publish(result(
            TimelineThumbnailRequest {
                request_id: current.request_id,
                bucket_us: 500_000,
                ..current
            },
            "wrong-bucket"
        )),
        None
    );
    assert_eq!(service.publish(result(current, "current")), Some("current"));
    assert_eq!(service.pending(), None);
}

#[test]
fn navigation_and_close_invalidate_thumbnail_results() {
    let service = VideoThumbnailService::default();
    let old_session = VideoSessionId::from_u128(1);
    let new_session = VideoSessionId::from_u128(2);
    service.activate(old_session, 7);
    let old = service.request(500_000).unwrap();

    service.activate(new_session, 8);
    assert_eq!(service.publish(result(old, "old-generation")), None);
    let current = service.request(1_000_000).unwrap();
    service.close();

    assert_eq!(service.publish(result(current, "closed")), None);
    assert_eq!(service.pending(), None);
    assert_eq!(service.request(1_500_000), None);
}

#[test]
fn every_matching_dimension_is_required_for_publication() {
    let service = VideoThumbnailService::default();
    let session_id = VideoSessionId::from_u128(1);

    service.activate(session_id, 7);
    let pending = service.request(500_000).unwrap();
    assert_eq!(
        service.publish(result(
            TimelineThumbnailRequest {
                session_id: VideoSessionId::from_u128(2),
                ..pending
            },
            "wrong-session"
        )),
        None
    );

    service.activate(session_id, 7);
    let pending = service.request(500_000).unwrap();
    assert_eq!(
        service.publish(result(
            TimelineThumbnailRequest {
                generation: 8,
                ..pending
            },
            "wrong-generation"
        )),
        None
    );

    service.activate(session_id, 7);
    let pending = service.request(500_000).unwrap();
    assert_eq!(
        service.publish(result(
            TimelineThumbnailRequest {
                request_id: VideoThumbnailRequestId::from_u128(12),
                ..pending
            },
            "wrong-request"
        )),
        None
    );

    service.activate(session_id, 7);
    let pending = service.request(500_000).unwrap();
    assert_eq!(
        service.publish(result(
            TimelineThumbnailRequest {
                bucket_us: 1_000_000,
                ..pending
            },
            "wrong-bucket"
        )),
        None
    );
}

fn result<T>(request: TimelineThumbnailRequest, artifact: T) -> TimelineThumbnailResult<T> {
    TimelineThumbnailResult {
        session_id: request.session_id,
        generation: request.generation,
        request_id: request.request_id,
        bucket_us: request.bucket_us,
        artifact,
    }
}
