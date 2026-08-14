use async_trait::async_trait;
use std::{
    collections::VecDeque,
    sync::{Mutex, MutexGuard},
};
use viewer_application::{
    EngineEvent, EngineOpenRequest, FrameDirection, PlaybackRate, SeekRequest, SurfaceRect,
    VideoEngine, VideoEngineError, VideoEvent,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeVideoEngineCall {
    OpenPaused(EngineOpenRequest),
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
struct FakeState {
    calls: Vec<FakeVideoEngineCall>,
    failures: VecDeque<VideoEngineError>,
}

#[derive(Default)]
pub struct FakeVideoEngine {
    state: Mutex<FakeState>,
}

impl FakeVideoEngine {
    pub fn calls(&self) -> Vec<FakeVideoEngineCall> {
        self.lock_state().calls.clone()
    }

    pub fn fail_next(&self, error: VideoEngineError) {
        self.lock_state().failures.push_back(error);
    }

    pub async fn emit(&self, generation: u64, event: EngineEvent) -> VideoEvent {
        VideoEvent { generation, event }
    }

    fn record(&self, call: FakeVideoEngineCall) -> Result<(), VideoEngineError> {
        let mut state = self.lock_state();
        state.calls.push(call);
        state.failures.pop_front().map_or(Ok(()), Err)
    }

    fn lock_state(&self) -> MutexGuard<'_, FakeState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl VideoEngine for FakeVideoEngine {
    async fn open_paused(&self, request: EngineOpenRequest) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::OpenPaused(request))
    }

    async fn reveal_surface(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::RevealSurface(generation))
    }

    async fn close(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::Close(generation))
    }

    async fn play(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::Play(generation))
    }

    async fn pause(&self, generation: u64) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::Pause(generation))
    }

    fn publish_seek(&self, generation: u64, request: SeekRequest) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::PublishSeek(generation, request))
    }

    async fn step(
        &self,
        generation: u64,
        direction: FrameDirection,
    ) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::Step(generation, direction))
    }

    async fn set_volume(&self, generation: u64, percent: u8) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::SetVolume(generation, percent))
    }

    async fn set_muted(&self, generation: u64, muted: bool) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::SetMuted(generation, muted))
    }

    async fn set_rate(&self, generation: u64, rate: PlaybackRate) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::SetRate(generation, rate))
    }

    fn publish_surface_rect(
        &self,
        generation: u64,
        sequence: u64,
        rect: SurfaceRect,
    ) -> Result<(), VideoEngineError> {
        self.record(FakeVideoEngineCall::PublishSurfaceRect(
            generation, sequence, rect,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeVideoEngine, FakeVideoEngineCall};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use viewer_application::{
        EngineEvent, EngineOpenRequest, FrameDirection, PlaybackRate, SeekIntent, SeekRequest,
        SurfaceRect, VideoEngine, VideoEngineError, VideoEvent, VideoSource,
    };
    use viewer_domain::{EntityId, VideoSessionId};

    #[test]
    fn fake_records_the_complete_platform_neutral_engine_contract() {
        let fake = FakeVideoEngine::default();
        let request = EngineOpenRequest {
            generation: 4,
            session_id: VideoSessionId::from_u128(3),
            source: VideoSource {
                entity_id: EntityId::from_u128(2),
                canonical_path: "/project/clip.mp4".into(),
            },
        };
        let rect = SurfaceRect {
            x: 1,
            y: 2,
            width: 640,
            height: 360,
        };

        block_on(async {
            fake.open_paused(request.clone()).await.unwrap();
            fake.reveal_surface(4).await.unwrap();
            fake.play(4).await.unwrap();
            fake.pause(4).await.unwrap();
            fake.publish_seek(
                4,
                SeekRequest {
                    request_id: 8,
                    time_us: 123,
                    intent: SeekIntent::Commit,
                },
            )
            .unwrap();
            fake.step(4, FrameDirection::Backward).await.unwrap();
            fake.set_volume(4, 42).await.unwrap();
            fake.set_muted(4, true).await.unwrap();
            fake.set_rate(4, PlaybackRate::OneAndHalf).await.unwrap();
            fake.publish_surface_rect(4, 9, rect).unwrap();
            fake.close(4).await.unwrap();
        });

        assert_eq!(
            fake.calls(),
            vec![
                FakeVideoEngineCall::OpenPaused(request),
                FakeVideoEngineCall::RevealSurface(4),
                FakeVideoEngineCall::Play(4),
                FakeVideoEngineCall::Pause(4),
                FakeVideoEngineCall::PublishSeek(
                    4,
                    SeekRequest {
                        request_id: 8,
                        time_us: 123,
                        intent: SeekIntent::Commit,
                    },
                ),
                FakeVideoEngineCall::Step(4, FrameDirection::Backward),
                FakeVideoEngineCall::SetVolume(4, 42),
                FakeVideoEngineCall::SetMuted(4, true),
                FakeVideoEngineCall::SetRate(4, PlaybackRate::OneAndHalf),
                FakeVideoEngineCall::PublishSurfaceRect(4, 9, rect),
                FakeVideoEngineCall::Close(4),
            ]
        );
    }

    #[test]
    fn fake_emits_generation_scoped_events_and_injects_one_failure() {
        let fake = FakeVideoEngine::default();
        fake.fail_next(VideoEngineError::RenderSurface);

        assert_eq!(
            block_on(fake.reveal_surface(8)),
            Err(VideoEngineError::RenderSurface)
        );
        assert_eq!(block_on(fake.reveal_surface(8)), Ok(()));
        assert_eq!(
            block_on(fake.emit(8, EngineEvent::FirstFrameReady)),
            VideoEvent {
                generation: 8,
                event: EngineEvent::FirstFrameReady,
            }
        );
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("fake video engine futures must complete immediately"),
        }
    }
}
