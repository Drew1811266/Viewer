//! Real-window development measurement. Uses the production host, decode
//! worker, input accumulator and display-link actor; no frontend test renderer.
#[path = "image_render_acceptance/main_thread_cpu.rs"]
mod main_thread_cpu;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use viewer_desktop::{
    dto::ImageRenderEventDto,
    image_render_events::ImageRenderEventPort,
    image_render_runtime::{
        AuthorizedImageRenderCommand as Command, ImageRenderDriver,
        ImageRenderMagnifierPreferences, NativeImageRenderDriver,
    },
};
use viewer_platform_macos::image_render::{AuthorizedImageSource, SurfaceLayout};
use viewer_render_core::{
    AnnotationGeometry, AnnotationId, AnnotationNode, AssetGeneration, LogicalPoint, MagnifySample,
    Modifiers, NativeInput, NormalizedPoint, NormalizedRect, PointerButton, PointerPhase,
    PointerSample, RenderSessionId, SceneRevision, SceneSnapshot,
};
use viewer_render_wgpu::{
    FrameReceipt, GpuFrameTiming, GpuTimingSupport, ImageRendererDiagnostics,
    ImageRendererPerformanceWorkload, MagnifierShape,
};

enum Observation {
    Frame(FrameReceipt, u64, u64, u64, u64),
    Ready(u32, u32),
    Failed(String),
    DetailUnavailable(u64),
    Recovering,
    GpuSupport(u64, GpuTimingSupport),
    GpuFrame(GpuFrameTiming),
}

type FrameKey = (u64, u64, u64);

#[derive(Default)]
struct GpuCapture {
    support: BTreeMap<u64, GpuTimingSupport>,
    samples: BTreeMap<FrameKey, GpuFrameTiming>,
}

impl GpuCapture {
    fn complete(&self, presented: &BTreeSet<FrameKey>) -> bool {
        presented.iter().all(|key| match self.support.get(&key.0) {
            Some(GpuTimingSupport::Unavailable) => true,
            Some(GpuTimingSupport::Available) => self.samples.contains_key(key),
            None => false,
        })
    }

    fn record(&mut self, timing: GpuFrameTiming) {
        self.samples.insert(
            (timing.renderer_id, timing.generation.0, timing.frame_index),
            timing,
        );
    }
}

struct Observer(mpsc::Sender<Observation>);

#[derive(Clone, Copy, Debug, PartialEq)]
enum MeasurementScenario {
    Main,
    Magnifier,
}

impl MeasurementScenario {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "main" => Ok(Self::Main),
            "magnifier" => Ok(Self::Magnifier),
            _ => Err("expected measurement scenario: main or magnifier"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Magnifier => "magnifier",
        }
    }

    fn preferences(self) -> Option<ImageRenderMagnifierPreferences> {
        (self == Self::Magnifier).then_some(ImageRenderMagnifierPreferences {
            width_px: 320.0,
            height_px: 320.0,
            magnification: 2.0,
            shape: MagnifierShape::Circle,
        })
    }
}

impl ImageRenderEventPort for Observer {
    fn publish(&self, event: ImageRenderEventDto) {
        let observation = match event {
            ImageRenderEventDto::Ready { width, height, .. } => Observation::Ready(width, height),
            ImageRenderEventDto::Failed { code, .. } => Observation::Failed(code),
            ImageRenderEventDto::DetailAvailabilityChanged {
                asset_generation,
                available: false,
                ..
            } => Observation::DetailUnavailable(asset_generation),
            ImageRenderEventDto::Recovering { .. } => Observation::Recovering,
            _ => return,
        };
        let _ = self.0.send(observation);
    }

    fn surface_frame_submitted(
        &self,
        receipt: FrameReceipt,
        tick: u64,
        generation: u64,
        dropped: u64,
        renderer_id: u64,
    ) {
        let _ = self.0.send(Observation::Frame(
            receipt,
            tick,
            generation,
            dropped,
            renderer_id,
        ));
    }

    fn gpu_timing_support(&self, renderer_id: u64, support: GpuTimingSupport) {
        let _ = self.0.send(Observation::GpuSupport(renderer_id, support));
    }

    fn gpu_frame_completed(&self, timing: GpuFrameTiming) {
        let _ = self.0.send(Observation::GpuFrame(timing));
    }
}

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let source = PathBuf::from(arguments.next().expect("expected fixture JPEG path"));
    let output = PathBuf::from(arguments.next().expect("expected receipt output path"));
    let scenario = MeasurementScenario::parse(
        &arguments
            .next()
            .expect("expected main or magnifier scenario")
            .to_string_lossy(),
    )
    .expect("invalid measurement scenario");
    assert!(arguments.next().is_none(), "unexpected arguments");
    let mut context = viewer_desktop::application_context();
    context.config_mut().app.windows.clear();
    context.config_mut().build.dev_url = None;
    tauri::Builder::default()
        .setup(move |app| {
            let window = tauri::WebviewWindowBuilder::new(
                app,
                "renderer-acceptance",
                tauri::WebviewUrl::External("about:blank".parse()?),
            )
            .title("Viewer — 原生图片性能验收")
            .inner_size(1200.0, 760.0)
            .transparent(true)
            .initialization_script(
                "document.addEventListener('DOMContentLoaded', () => { document.documentElement.style.background='transparent'; document.body.style.background='transparent'; });",
            )
            .build()?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let result = measure(window, source, output, scenario);
                if let Err(error) = &result {
                    eprintln!("Native window measurement failed: {error}");
                }
                handle.exit(if result.is_ok() { 0 } else { 1 });
            });
            Ok(())
        })
        .run(context)
        .expect("native acceptance window must run");
}

fn measure(
    window: tauri::WebviewWindow,
    source_path: PathBuf,
    output: PathBuf,
    scenario: MeasurementScenario,
) -> Result<(), Box<dyn std::error::Error>> {
    // Every invocation owns a fresh derived cache; the warm open reuses only
    // this run's cache. User projects and their metadata are never edited.
    let cache = tempfile::tempdir()?;
    let (sender, receiver) = mpsc::channel();
    let driver = NativeImageRenderDriver::new(cache.path(), Arc::new(Observer(sender)));
    driver.bind_window(window.clone())?;
    let size = window
        .inner_size()?
        .to_logical::<f64>(window.scale_factor()?);
    let started = Instant::now();
    let mut diagnostics = ImageRendererDiagnostics::new(0);
    let source = AuthorizedImageSource::authorize_for_process(source_path)?;
    eprintln!(
        "Source authorization: {:.2} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    driver.apply(Command::Open {
        session_id: RenderSessionId(1),
        generation: AssetGeneration(1),
        source: source.clone(),
    })?;
    driver.apply(Command::SetScene { scene: scene() })?;
    driver.apply(Command::SetMagnifier {
        magnifier: scenario.preferences(),
    })?;
    driver.apply(Command::SetSurface {
        layout: SurfaceLayout {
            left: 0.0,
            top: 0.0,
            width: size.width,
            height: size.height,
            scale_factor: window.scale_factor()?,
        },
    })?;
    let mut source_size = None;
    let mut gpu = GpuCapture::default();
    eprintln!(
        "Surface initialized: {:.2} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    next_frame(
        &receiver,
        1,
        Some(scenario == MeasurementScenario::Magnifier),
        &mut diagnostics,
        &mut source_size,
        &mut gpu,
    )?;
    diagnostics.mark_first_interactive(elapsed_ns(started));
    eprintln!(
        "Cold content submitted: {:.2} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );

    diagnostics.begin_warm_open(elapsed_ns(started));
    driver.apply(Command::Open {
        session_id: RenderSessionId(1),
        generation: AssetGeneration(2),
        source,
    })?;
    driver.apply(Command::SetScene { scene: scene() })?;
    driver.apply(Command::SetMagnifier {
        magnifier: scenario.preferences(),
    })?;
    next_frame(
        &receiver,
        2,
        Some(scenario == MeasurementScenario::Magnifier),
        &mut diagnostics,
        &mut source_size,
        &mut gpu,
    )?;
    diagnostics.mark_warm_first_interactive(elapsed_ns(started));
    diagnostics.reset_frame_window();

    let input = driver
        .native_input_sink()
        .ok_or("native input monitor is missing")?;
    let mut presented = BTreeSet::new();
    let mut magnifier_frame_samples = 0;
    let main_cpu_start = main_thread_cpu::capture();
    for frame in 0..240 {
        let position = LogicalPoint::new(size.width / 2.0, size.height / 2.0)?;
        input(NativeInput::Magnify(MagnifySample {
            location: position,
            factor: if frame < 120 { 1.008 } else { 1.0 / 1.008 },
            timestamp_ns: elapsed_ns(started),
        }));
        for (phase, dx) in [
            (PointerPhase::Down, 0.0),
            (PointerPhase::Move, 1.0),
            (PointerPhase::Up, 1.0),
        ] {
            input(NativeInput::Pointer(PointerSample {
                phase,
                location: LogicalPoint::new(position.x + dx, position.y)?,
                button: PointerButton::Primary,
                pressure: 1.0,
                modifiers: Modifiers {
                    space: true,
                    ..Modifiers::default()
                },
                timestamp_ns: elapsed_ns(started),
            }));
        }
        let (receipt, tick, key) = next_frame(
            &receiver,
            2,
            None,
            &mut diagnostics,
            &mut source_size,
            &mut gpu,
        )?;
        magnifier_frame_samples += usize::from(receipt.magnifier_rendered);
        presented.insert(key);
        diagnostics.record_frame(receipt, tick);
    }
    // This brackets the warm workload on AppKit's main thread; it is not
    // per-frame or scanout CPU, and excludes the query drain and close below.
    let main_thread_cpu = main_thread_cpu::observe(main_cpu_start, main_thread_cpu::capture());
    // Query completion is asynchronous. Wait in this measurement worker, never
    // the render actor, and join only measurements for the exact sampled frames.
    let deadline = Instant::now() + Duration::from_secs(3);
    while !gpu.complete(&presented) {
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            Observation::GpuSupport(id, support) => {
                gpu.support.insert(id, support);
            }
            Observation::GpuFrame(timing) => gpu.record(timing),
            Observation::Failed(code) => return Err(code.into()),
            Observation::DetailUnavailable(2) => {
                return Err("required image detail is unavailable".into());
            }
            Observation::Recovering => diagnostics.record_recovery(),
            _ => {}
        }
    }
    let support = if presented
        .iter()
        .any(|key| gpu.support.get(&key.0) == Some(&GpuTimingSupport::Available))
    {
        GpuTimingSupport::Available
    } else {
        GpuTimingSupport::Unavailable
    };
    diagnostics.set_gpu_timing_support(support);
    for key in &presented {
        if let Some(timing) = gpu.samples.get(key) {
            diagnostics.record_gpu_timing(*timing);
        }
    }
    driver.close()?;
    let (width, height) = source_size.ok_or("decoded source dimensions were not reported")?;
    let receipt = diagnostics.finish(ImageRendererPerformanceWorkload {
        source_width: width,
        source_height: height,
        annotation_count: 500,
        display_hz: 60,
    });
    let mut receipt = serde_json::to_value(receipt)?;
    receipt["scenario"] = serde_json::json!(scenario.name());
    receipt["magnifier_frame_samples"] = serde_json::json!(magnifier_frame_samples);
    receipt["main_thread_cpu"] = serde_json::to_value(main_thread_cpu)?;
    std::fs::write(&output, serde_json::to_vec_pretty(&receipt)?)?;
    println!("Native surface receipt: {}", output.display());
    Ok(())
}

fn next_frame(
    receiver: &mpsc::Receiver<Observation>,
    expected_generation: u64,
    expected_magnifier: Option<bool>,
    diagnostics: &mut ImageRendererDiagnostics,
    source_size: &mut Option<(u32, u32)>,
    gpu: &mut GpuCapture,
) -> Result<(FrameReceipt, u64, FrameKey), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            Observation::Frame(receipt, tick, generation, dropped, renderer_id)
                if generation == expected_generation
                    && expected_magnifier
                        .is_none_or(|enabled| receipt.magnifier_rendered == enabled) =>
            {
                if dropped > 0 {
                    return Err(format!("native input dropped {dropped} samples").into());
                }
                return Ok((
                    receipt,
                    tick,
                    (renderer_id, generation, receipt.frame_index),
                ));
            }
            Observation::Frame(..) => {}
            Observation::DetailUnavailable(generation) => {
                if generation == expected_generation {
                    return Err("required image detail is unavailable".into());
                }
            }
            Observation::Ready(width, height) => *source_size = Some((width, height)),
            Observation::Failed(code) => return Err(code.into()),
            Observation::Recovering => diagnostics.record_recovery(),
            Observation::GpuSupport(id, support) => {
                gpu.support.insert(id, support);
            }
            Observation::GpuFrame(timing) => gpu.record(timing),
        }
    }
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn scene() -> SceneSnapshot {
    let annotations = (0..500_u32)
        .map(|index| {
            let x = f64::from(index % 25) / 25.0;
            let y = f64::from(index / 25) / 20.0;
            let rect = NormalizedRect::new(x, y, 0.025, 0.025).unwrap();
            let point = |dx, dy| NormalizedPoint::new(x + dx, y + dy).unwrap();
            let geometry = match index % 5 {
                0 => AnnotationGeometry::Point {
                    position: point(0.01, 0.01),
                },
                1 => AnnotationGeometry::Arrow {
                    tail: point(0.0, 0.0),
                    head: point(0.025, 0.025),
                },
                2 => AnnotationGeometry::Rectangle { rect },
                3 => AnnotationGeometry::Ellipse { rect },
                _ => AnnotationGeometry::Stroke {
                    points: (0..8)
                        .map(|step| point(f64::from(step) * 0.003, f64::from(step % 3) * 0.004))
                        .collect(),
                },
            };
            AnnotationNode::new(
                AnnotationId::new(format!("perf-{index}")).unwrap(),
                index + 1,
                geometry,
            )
            .unwrap()
        })
        .collect();
    SceneSnapshot::new(SceneRevision(1), annotations, None).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_detail_cannot_pass_the_window_workload_as_preview_only() {
        let (sender, receiver) = mpsc::channel();
        let observer = Observer(sender);
        observer.publish(ImageRenderEventDto::DetailAvailabilityChanged {
            session_id: 1,
            asset_generation: 2,
            resource_revision: 4,
            available: false,
        });
        assert!(matches!(
            receiver.try_recv(),
            Ok(Observation::DetailUnavailable(2))
        ));
        observer.publish(ImageRenderEventDto::DetailAvailabilityChanged {
            session_id: 1,
            asset_generation: 2,
            resource_revision: 4,
            available: true,
        });
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn frame_wait_rejects_unavailable_detail_for_its_generation() {
        let (sender, receiver) = mpsc::channel();
        sender.send(Observation::DetailUnavailable(2)).unwrap();
        drop(sender);
        let error = next_frame(
            &receiver,
            2,
            None,
            &mut ImageRendererDiagnostics::new(0),
            &mut None,
            &mut GpuCapture::default(),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "required image detail is unavailable");
    }

    #[test]
    fn first_lens_frame_waits_for_the_matching_generation_and_encoded_pass() {
        let (sender, receiver) = mpsc::channel();
        sender.send(Observation::DetailUnavailable(1)).unwrap();
        for (generation, index, lens) in [(1, 7, true), (2, 8, false), (2, 9, true)] {
            sender
                .send(Observation::Frame(
                    FrameReceipt {
                        frame_index: index,
                        scene_revision: SceneRevision(1),
                        cpu_time_ns: 1,
                        gpu_time_ns: 0,
                        gpu_resource_bytes: 4,
                        presented: true,
                        magnifier_rendered: lens,
                    },
                    index * 16_666_667,
                    generation,
                    0,
                    3,
                ))
                .unwrap();
        }
        drop(sender);
        let (receipt, _, key) = next_frame(
            &receiver,
            2,
            Some(true),
            &mut ImageRendererDiagnostics::new(0),
            &mut None,
            &mut GpuCapture::default(),
        )
        .unwrap();
        assert_eq!(key, (3, 2, 9));
        assert!(receipt.magnifier_rendered);
    }

    #[test]
    fn rejects_unknown_scenarios_instead_of_measuring_main_under_a_lens_label() {
        assert_eq!(
            MeasurementScenario::parse("main").unwrap(),
            MeasurementScenario::Main
        );
        assert_eq!(
            MeasurementScenario::parse("magnifier").unwrap(),
            MeasurementScenario::Magnifier
        );
        for invalid in ["", "lens", "Main", "magnifier "] {
            assert!(MeasurementScenario::parse(invalid).is_err());
        }
        assert!(MeasurementScenario::Main.preferences().is_none());
        let lens = MeasurementScenario::Magnifier.preferences().unwrap();
        assert_eq!(lens.width_px, 320.0);
        assert_eq!(lens.height_px, 320.0);
        assert_eq!(lens.magnification, 2.0);
        assert_eq!(lens.shape, MagnifierShape::Circle);
    }

    #[test]
    fn gpu_capture_requires_each_exact_renderer_generation_and_frame() {
        let mut capture = GpuCapture::default();
        let presented = BTreeSet::from([(3, 2, 8), (3, 2, 9)]);
        capture.support.insert(3, GpuTimingSupport::Available);
        let sample = GpuFrameTiming {
            renderer_id: 3,
            generation: AssetGeneration(2),
            frame_index: 8,
            scene_revision: SceneRevision(1),
            gpu_time_ns: 1_000,
        };
        capture.record(sample);
        capture.record(sample); // Duplicate completion does not fill frame 9.
        capture.record(GpuFrameTiming {
            renderer_id: 2,
            frame_index: 9,
            ..sample
        });
        capture.record(GpuFrameTiming {
            generation: AssetGeneration(1),
            frame_index: 9,
            ..sample
        });
        assert!(!capture.complete(&presented));
        capture.record(GpuFrameTiming {
            frame_index: 9,
            ..sample
        });
        assert!(capture.complete(&presented));
    }

    #[test]
    fn unsupported_is_explicit_not_inferred_from_missing_gpu_samples() {
        let mut capture = GpuCapture::default();
        let presented = BTreeSet::from([(3, 2, 8)]);
        assert!(!capture.complete(&presented));
        capture.support.insert(3, GpuTimingSupport::Unavailable);
        assert!(capture.complete(&presented));
        capture.support.insert(3, GpuTimingSupport::Available);
        assert!(!capture.complete(&presented));
    }
}
