use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use viewer_application::browse::BrowserFile;
use viewer_application::{
    EngineEvent, EngineOpenRequest, FrameDirection, PlaybackRate, SeekIntent, SeekRequest,
    SurfaceRect, VideoCommand, VideoCommandKind, VideoEngine, VideoEngineError, VideoMedia,
    VideoPlaybackState, VideoPreviewService, VideoServiceError, VideoSource, video_neighbors,
};
use viewer_domain::{
    EntityId, RelativePath,
    file::{FileKind, Marker},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Call {
    OpenPaused(u64),
    RevealSurface(u64),
    Close(u64),
    Play(u64),
    Pause(u64),
    PublishSeek(u64, SeekRequest),
    Step(u64, FrameDirection),
    SetVolume(u64, u8),
    SetMuted(u64, bool),
    SetRate(u64, PlaybackRate),
    PublishSurfaceRect(u64, u64, SurfaceRect),
}

#[derive(Default)]
struct RecordingEngine {
    calls: Mutex<Vec<Call>>,
}

impl RecordingEngine {
    fn record(&self, call: Call) {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(call);
    }

    fn calls(&self) -> Vec<Call> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[async_trait]
impl VideoEngine for RecordingEngine {
    async fn open_paused(&self, request: EngineOpenRequest) -> Result<(), VideoEngineError> {
        self.record(Call::OpenPaused(request.generation));
        Ok(())
    }

    async fn reveal_surface(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(Call::RevealSurface(generation));
        Ok(())
    }

    async fn close(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(Call::Close(generation));
        Ok(())
    }

    async fn play(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(Call::Play(generation));
        Ok(())
    }

    async fn pause(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(Call::Pause(generation));
        Ok(())
    }

    fn publish_seek(&self, generation: u64, request: SeekRequest) -> Result<(), VideoEngineError> {
        self.record(Call::PublishSeek(generation, request));
        Ok(())
    }

    async fn step(
        &self,
        generation: u64,
        direction: FrameDirection,
    ) -> Result<(), VideoEngineError> {
        self.record(Call::Step(generation, direction));
        Ok(())
    }

    async fn set_volume(&self, generation: u64, percent: u8) -> Result<(), VideoEngineError> {
        self.record(Call::SetVolume(generation, percent));
        Ok(())
    }

    async fn set_muted(&self, generation: u64, muted: bool) -> Result<(), VideoEngineError> {
        self.record(Call::SetMuted(generation, muted));
        Ok(())
    }

    async fn set_rate(&self, generation: u64, rate: PlaybackRate) -> Result<(), VideoEngineError> {
        self.record(Call::SetRate(generation, rate));
        Ok(())
    }

    fn publish_surface_rect(
        &self,
        generation: u64,
        sequence: u64,
        rect: SurfaceRect,
    ) -> Result<(), VideoEngineError> {
        self.record(Call::PublishSurfaceRect(generation, sequence, rect));
        Ok(())
    }
}

fn source(index: u128) -> VideoSource {
    VideoSource {
        entity_id: EntityId::from_u128(index),
        canonical_path: format!("/project/video-{index}.mp4").into(),
    }
}

fn media() -> VideoMedia {
    VideoMedia {
        duration_us: Some(2_000_000),
        display_width: Some(1_920),
        display_height: Some(1_080),
        rotation_degrees: 0,
    }
}

fn harness() -> (VideoPreviewService<RecordingEngine>, Arc<RecordingEngine>) {
    let engine = Arc::new(RecordingEngine::default());
    (VideoPreviewService::new(Arc::clone(&engine)), engine)
}

#[tokio::test]
async fn first_frame_reveals_then_starts_playback_exactly_once() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();

    service
        .handle_engine_event(generation, EngineEvent::Prepared(media()))
        .await
        .unwrap();
    service
        .handle_engine_event(generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();
    service
        .handle_engine_event(generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();

    assert_eq!(
        engine.calls(),
        vec![
            Call::OpenPaused(generation),
            Call::RevealSurface(generation),
            Call::Play(generation),
        ]
    );
    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::Playing
    );
}

#[tokio::test]
async fn stale_first_frame_cannot_reveal_after_navigation() {
    let (service, engine) = harness();
    let old_generation = service.open(source(1)).await.unwrap();
    let new_generation = service.open(source(2)).await.unwrap();

    service
        .handle_engine_event(old_generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();

    assert_eq!(service.snapshot().generation, new_generation);
    assert_eq!(
        engine.calls(),
        vec![
            Call::OpenPaused(old_generation),
            Call::Close(old_generation),
            Call::OpenPaused(new_generation),
        ]
    );
}

async fn start_playing(service: &VideoPreviewService<RecordingEngine>, generation: u64) {
    service
        .handle_engine_event(generation, EngineEvent::Prepared(media()))
        .await
        .unwrap();
    service
        .handle_engine_event(generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();
}

#[tokio::test]
async fn preview_does_not_enter_seeking_and_only_matching_commit_completes() {
    const PREVIEW: SeekRequest = SeekRequest {
        request_id: 10,
        time_us: 2_000_000,
        intent: SeekIntent::Preview,
    };
    const COMMIT: SeekRequest = SeekRequest {
        request_id: 11,
        time_us: 2_250_000,
        intent: SeekIntent::Commit,
    };

    let (service, _) = harness();
    let generation = service.open(source(1)).await.unwrap();
    start_playing(&service, generation).await;
    service
        .execute(VideoCommand {
            generation,
            kind: VideoCommandKind::Pause,
        })
        .await
        .unwrap();

    service.preview_seek(generation, PREVIEW).unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Paused);

    service
        .execute(VideoCommand::seek(generation, COMMIT))
        .await
        .unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Seeking);

    service
        .handle_engine_event(
            generation,
            EngineEvent::TimeChanged {
                time_us: PREVIEW.time_us,
                duration_us: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(service.snapshot().time_us, 0);

    service
        .handle_engine_event(
            generation,
            EngineEvent::SeekCompleted {
                request_id: PREVIEW.request_id,
                time_us: PREVIEW.time_us,
            },
        )
        .await
        .unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Seeking);

    service
        .handle_engine_event(
            generation,
            EngineEvent::SeekCompleted {
                request_id: COMMIT.request_id,
                time_us: COMMIT.time_us,
            },
        )
        .await
        .unwrap();
    assert_eq!(service.snapshot().state, VideoPlaybackState::Paused);
}

#[tokio::test]
async fn close_is_idempotent_and_late_events_cannot_resurrect_the_session() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();

    service.close().await.unwrap();
    service.close().await.unwrap();
    service
        .handle_engine_event(generation, EngineEvent::Prepared(media()))
        .await
        .unwrap();
    service
        .handle_engine_event(generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();

    let snapshot = service.snapshot();
    assert_eq!(snapshot.state, viewer_application::VideoPlaybackState::Idle);
    assert_eq!(snapshot.session_id, None);
    assert_eq!(
        engine.calls(),
        vec![Call::OpenPaused(generation), Call::Close(generation)]
    );
}

#[tokio::test]
async fn ended_pauses_on_the_reported_final_frame() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();
    start_playing(&service, generation).await;

    service
        .handle_engine_event(
            generation,
            EngineEvent::TimeChanged {
                time_us: 1_999_500,
                duration_us: Some(2_000_000),
            },
        )
        .await
        .unwrap();
    service
        .handle_engine_event(generation, EngineEvent::Ended)
        .await
        .unwrap();

    let snapshot = service.snapshot();
    assert_eq!(
        snapshot.state,
        viewer_application::VideoPlaybackState::Ended
    );
    assert_eq!(snapshot.time_us, 2_000_000);
    assert_eq!(engine.calls().last(), Some(&Call::Pause(generation)));
}

#[tokio::test]
async fn navigation_closes_then_resets_ephemeral_playback_preferences() {
    let (service, engine) = harness();
    let old_generation = service.open(source(1)).await.unwrap();
    start_playing(&service, old_generation).await;
    service.set_volume(37).await.unwrap();
    service.set_muted(true).await.unwrap();
    service.set_rate(PlaybackRate::Double).await.unwrap();
    service
        .handle_engine_event(
            old_generation,
            EngineEvent::TimeChanged {
                time_us: 900_000,
                duration_us: Some(2_000_000),
            },
        )
        .await
        .unwrap();

    let new_generation = service.open(source(2)).await.unwrap();

    let snapshot = service.snapshot();
    assert_eq!(new_generation, old_generation + 1);
    assert_eq!(snapshot.time_us, 0);
    assert_eq!(snapshot.duration_us, None);
    assert_eq!(snapshot.volume_percent, 100);
    assert!(!snapshot.muted);
    assert_eq!(snapshot.rate, PlaybackRate::Normal);
    assert_eq!(
        engine.calls().as_slice(),
        &[
            Call::OpenPaused(old_generation),
            Call::RevealSurface(old_generation),
            Call::Play(old_generation),
            Call::SetVolume(old_generation, 37),
            Call::SetMuted(old_generation, true),
            Call::SetRate(old_generation, PlaybackRate::Double),
            Call::Close(old_generation),
            Call::OpenPaused(new_generation),
        ]
    );
}

#[tokio::test]
async fn frame_step_while_playing_pauses_before_step() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();
    start_playing(&service, generation).await;

    service.step(FrameDirection::Forward).await.unwrap();

    assert!(engine.calls().ends_with(&[
        Call::Pause(generation),
        Call::Step(generation, FrameDirection::Forward),
    ]));
    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::FrameStepping
    );

    service
        .handle_engine_event(generation, EngineEvent::FrameStepped { time_us: 1_033_366 })
        .await
        .unwrap();
    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::Paused
    );
    assert_eq!(service.snapshot().time_us, 1_033_366);
}

#[tokio::test]
async fn stale_generation_command_cannot_control_the_new_video() {
    let (service, engine) = harness();
    let old_generation = service.open(source(1)).await.unwrap();
    let new_generation = service.open(source(2)).await.unwrap();
    let calls_before_stale_command = engine.calls();

    let result = service.execute(VideoCommand::play(old_generation)).await;

    assert_eq!(result, Err(VideoServiceError::StaleGeneration));
    assert_eq!(service.snapshot().generation, new_generation);
    assert_eq!(engine.calls(), calls_before_stale_command);
}

fn browser_file(index: u128, kind: FileKind) -> BrowserFile {
    BrowserFile {
        entity_id: EntityId::from_u128(index),
        relative_path: RelativePath::parse(&format!("item-{index}")).unwrap(),
        name: format!("item-{index}"),
        kind,
        size: 10,
        modified_ns: 20,
        marker: Marker::default(),
        image_metadata: None,
        video_metadata: None,
    }
}

#[test]
fn navigation_is_video_only() {
    let files = vec![
        browser_file(10, FileKind::Jpeg),
        browser_file(1, FileKind::Video),
        browser_file(11, FileKind::Text),
        browser_file(2, FileKind::Video),
        browser_file(12, FileKind::Png),
        browser_file(3, FileKind::Video),
    ];

    assert_eq!(
        video_neighbors(&files, EntityId::from_u128(2)),
        (Some(EntityId::from_u128(1)), Some(EntityId::from_u128(3)))
    );
    assert_eq!(
        video_neighbors(&files, EntityId::from_u128(1)),
        (None, Some(EntityId::from_u128(2)))
    );
    assert_eq!(
        video_neighbors(&files, EntityId::from_u128(3)),
        (Some(EntityId::from_u128(2)), None)
    );
    assert_eq!(
        video_neighbors(&files, EntityId::from_u128(11)),
        (None, None)
    );
}

#[tokio::test]
async fn seek_returns_to_the_pre_seek_playback_state() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();
    start_playing(&service, generation).await;

    service
        .execute(VideoCommand::seek(
            generation,
            SeekRequest {
                request_id: 20,
                time_us: 3_000_000,
                intent: SeekIntent::Commit,
            },
        ))
        .await
        .unwrap();
    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::Seeking
    );
    assert_eq!(
        engine.calls().last(),
        Some(&Call::PublishSeek(
            generation,
            SeekRequest {
                request_id: 20,
                time_us: 2_000_000,
                intent: SeekIntent::Commit,
            },
        ))
    );

    service
        .handle_engine_event(
            generation,
            EngineEvent::SeekCompleted {
                request_id: 20,
                time_us: 2_000_000,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::Playing
    );

    service
        .execute(VideoCommand {
            generation,
            kind: VideoCommandKind::Pause,
        })
        .await
        .unwrap();
    service
        .execute(VideoCommand::seek(
            generation,
            SeekRequest {
                request_id: 21,
                time_us: 250_000,
                intent: SeekIntent::Commit,
            },
        ))
        .await
        .unwrap();
    service
        .handle_engine_event(
            generation,
            EngineEvent::SeekCompleted {
                request_id: 21,
                time_us: 250_000,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::Paused
    );
    assert_eq!(service.snapshot().time_us, 250_000);
}

#[tokio::test]
async fn normalized_engine_failure_blocks_late_first_frame_reveal() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();

    service
        .handle_engine_event(
            generation,
            EngineEvent::Failed(viewer_domain::video::VideoFailureKind::Damaged),
        )
        .await
        .unwrap();
    service
        .handle_engine_event(generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();

    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::Failed(
            viewer_domain::video::VideoFailureKind::Damaged
        )
    );
    assert_eq!(engine.calls(), vec![Call::OpenPaused(generation)]);
}

#[tokio::test]
async fn a_late_progress_event_cannot_move_ended_playback_off_the_final_frame() {
    let (service, _engine) = harness();
    let generation = service.open(source(1)).await.unwrap();
    start_playing(&service, generation).await;
    service
        .handle_engine_event(generation, EngineEvent::Ended)
        .await
        .unwrap();

    service
        .handle_engine_event(
            generation,
            EngineEvent::TimeChanged {
                time_us: 1_250_000,
                duration_us: Some(2_000_000),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        service.snapshot().state,
        viewer_application::VideoPlaybackState::Ended
    );
    assert_eq!(service.snapshot().time_us, 2_000_000);
}

#[tokio::test]
async fn seek_before_first_frame_is_rejected_without_bypassing_reveal() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();
    service
        .handle_engine_event(generation, EngineEvent::Prepared(media()))
        .await
        .unwrap();

    assert_eq!(
        service
            .execute(VideoCommand::seek(
                generation,
                SeekRequest {
                    request_id: 22,
                    time_us: 500_000,
                    intent: SeekIntent::Commit,
                },
            ))
            .await,
        Err(VideoServiceError::InvalidState)
    );
    service
        .handle_engine_event(generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();

    assert_eq!(
        engine.calls(),
        vec![
            Call::OpenPaused(generation),
            Call::RevealSurface(generation),
            Call::Play(generation),
        ]
    );
}

#[tokio::test]
async fn frame_step_before_first_frame_is_rejected_without_bypassing_reveal() {
    let (service, engine) = harness();
    let generation = service.open(source(1)).await.unwrap();
    service
        .handle_engine_event(generation, EngineEvent::Prepared(media()))
        .await
        .unwrap();

    assert_eq!(
        service.step(FrameDirection::Forward).await,
        Err(VideoServiceError::InvalidState)
    );
    service
        .handle_engine_event(generation, EngineEvent::FirstFrameReady)
        .await
        .unwrap();

    assert_eq!(
        engine.calls(),
        vec![
            Call::OpenPaused(generation),
            Call::RevealSurface(generation),
            Call::Play(generation),
        ]
    );
}

#[tokio::test]
async fn generation_scoped_close_is_idempotent_but_stale_close_is_rejected() {
    let (service, engine) = harness();
    let old_generation = service.open(source(1)).await.unwrap();
    let close_old = VideoCommand {
        generation: old_generation,
        kind: VideoCommandKind::Close,
    };

    service.execute(close_old).await.unwrap();
    service.execute(close_old).await.unwrap();
    let new_generation = service.open(source(2)).await.unwrap();
    assert_eq!(
        service.execute(close_old).await,
        Err(VideoServiceError::StaleGeneration)
    );

    assert_eq!(service.snapshot().generation, new_generation);
    assert_eq!(
        engine.calls(),
        vec![
            Call::OpenPaused(old_generation),
            Call::Close(old_generation),
            Call::OpenPaused(new_generation),
        ]
    );
}
