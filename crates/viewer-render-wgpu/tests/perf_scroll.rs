//! 冷滚动帧时间性能门禁（性能优化方案四，见 docs/architecture/performance-optimization-plan.md §5）。
//!
//! 门控 `VIEWER_RUN_PERF_TESTS=1`（与 Metal 测试同模式，不在默认 CI 跑）；
//! 由 `pnpm verify:perf` 显式执行，结果记入 `docs/quality/perf-baseline.md`。
//!
//! 场景：无头渲染器 + 合成分块大图（4096×4096、1024 瓦片），相机放大 2× 后
//! 逐帧向下平移 60 帧；每帧只上传新进入视口的瓦片（冷缓存语义），度量
//! CPU 侧帧时间（wall clock，含冷瓦片上传），断言 p95 < 8ms。
//! GPU 时间戳样本可用时附带报告（信息性，不作门禁——无头模式无法呈现）。

use std::collections::BTreeSet;
use std::time::Instant;

use viewer_render_core::{
    AssetGeneration, CameraState, ImageMemoryCoordinator, ImageMemoryPolicy, LogicalPoint,
    LogicalSize, PhysicalSize, Rotation, SceneRevision, SharedPixels, SourceSize, TileCoordinate,
    TransformSnapshot, ViewportLayout,
};
use viewer_render_wgpu::{
    DecodedResource, FrameReasons, FrameRequest, GpuTimingSupport, RendererDescriptor,
    WgpuImageRenderer,
};

const SOURCE: u32 = 4_096;
const TILE: u32 = 1_024;
const FRAMES: usize = 60;
const SCROLL_STEP_PX: f64 = 8.0;
const FRAME_BUDGET_P95_MS: f64 = 8.0;

const FRAME_REQUEST: FrameRequest = FrameRequest {
    scene_revision: SceneRevision(1),
    reasons: FrameReasons {
        camera: true,
        scene: false,
        resource: false,
        surface: false,
    },
};

// Break caught: a regression that pushes cold-scroll frame times past the 60fps
// budget (tile upload or image pass cost) must fail the performance gate.
#[test]
fn cold_scroll_frame_time_p95_stays_within_budget() {
    if std::env::var_os("VIEWER_RUN_PERF_TESTS").is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let logical = LogicalSize::new(1_024.0, 1_024.0).unwrap();
    let mut renderer = WgpuImageRenderer::new(
        RendererDescriptor::headless(
            logical,
            PhysicalSize {
                width: 1_024,
                height: 1_024,
            },
            1.0,
        )
        .unwrap(),
    )
    .unwrap();

    let source_size = SourceSize::new(SOURCE, SOURCE).unwrap();
    let viewport = ViewportLayout::new(logical, 1.0, 1.0).unwrap();
    let fitted =
        TransformSnapshot::new(source_size, viewport, CameraState::fit(Rotation::Deg0)).unwrap();
    let mut camera = fitted
        .zoom_at(2.0, LogicalPoint { x: 512.0, y: 512.0 })
        .unwrap();

    let mut uploaded: BTreeSet<TileCoordinate> = BTreeSet::new();
    let mut frame_ms = Vec::with_capacity(FRAMES);
    for _ in 0..FRAMES {
        // pan_by 定义在 TransformSnapshot 上：先由当前相机构图，再求下一帧相机。
        let current = TransformSnapshot::new(source_size, viewport, camera).unwrap();
        camera = current
            .pan_by(LogicalPoint {
                x: 0.0,
                y: -SCROLL_STEP_PX,
            })
            .unwrap();
        let snapshot = TransformSnapshot::new(source_size, viewport, camera).unwrap();
        // 帧时间从冷瓦片上传开始计时：滚动挂载新内容的帧必须整体留在预算内。
        let started = Instant::now();
        for tile in visible_tiles(&snapshot) {
            if uploaded.insert(tile) {
                upload_tile(&mut renderer, tile, source_size);
            }
        }
        renderer.set_transform(snapshot).unwrap();
        renderer.render(FRAME_REQUEST).unwrap();
        frame_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
    }

    frame_ms.sort_by(|a, b| a.partial_cmp(b).expect("frame times are finite"));
    let last = frame_ms.len() - 1;
    let p95 = frame_ms[((frame_ms.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(last)];
    assert!(
        p95 < FRAME_BUDGET_P95_MS,
        "cold scroll p95 {p95:.2}ms must stay under the {FRAME_BUDGET_P95_MS}ms budget; frames={frame_ms:?}"
    );
    eprintln!(
        "冷滚动 {FRAMES} 帧（冷上传 {} 个瓦片）: p50={:.2}ms p95={p95:.2}ms max={:.2}ms（预算 {FRAME_BUDGET_P95_MS}ms）",
        uploaded.len(),
        frame_ms[last / 2],
        frame_ms[last],
    );
    report_gpu_p95(&mut renderer);
}

/// 视口四角映射回图像坐标，展开成可见瓦片集合（level 0）。
fn visible_tiles(snapshot: &TransformSnapshot) -> Vec<TileCoordinate> {
    let viewport = snapshot.viewport().logical_size;
    let corners = [
        snapshot.view_to_image_clamped(LogicalPoint { x: 0.0, y: 0.0 }),
        snapshot.view_to_image_clamped(LogicalPoint {
            x: viewport.width - 1.0,
            y: viewport.height - 1.0,
        }),
    ];
    let min_x = corners.iter().map(|c| c.x).fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|c| c.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners.iter().map(|c| c.y).fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|c| c.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let tiles_per_side = SOURCE / TILE;
    let clamp_tile =
        |value: f64| -> u32 { ((value * f64::from(SOURCE)) as u32 / TILE).min(tiles_per_side - 1) };
    let (x0, x1) = (clamp_tile(min_x), clamp_tile(max_x));
    let (y0, y1) = (clamp_tile(min_y), clamp_tile(max_y));
    let mut tiles = Vec::with_capacity(((x1 - x0 + 1) * (y1 - y0 + 1)) as usize);
    for y in y0..=y1 {
        for x in x0..=x1 {
            tiles.push(TileCoordinate { level: 0, x, y });
        }
    }
    tiles
}

fn upload_tile(renderer: &mut WgpuImageRenderer, tile: TileCoordinate, source_size: SourceSize) {
    let width = TILE.min(SOURCE - tile.x * TILE);
    let height = TILE.min(SOURCE - tile.y * TILE);
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for (index, byte) in pixels.iter_mut().enumerate() {
        *byte = ((index as u32).wrapping_mul(2_654_435_761) >> 24) as u8;
    }
    let shared = SharedPixels::try_copy_from_slice(
        &ImageMemoryCoordinator::new(ImageMemoryPolicy::baseline_8gb()),
        AssetGeneration(1),
        &pixels,
    )
    .unwrap();
    renderer
        .upload_resource(DecodedResource::Tile {
            generation: AssetGeneration(1),
            tile,
            tile_size: TILE,
            sample_border: 0,
            source_size,
            width,
            height,
            pixels: shared,
        })
        .unwrap();
    // 上传池一次只持有一个 pending 资源（生产语义：每帧一瓦片）；排空它，
    // 让下一个瓦片可以继续进入管线——与真实渲染循环的逐帧 poll 一致。
    let mut polls = 0;
    while renderer.has_pending_upload() {
        renderer.poll_gpu_timings().unwrap();
        polls += 1;
        assert!(polls < 1_000, "pending tile upload did not complete");
    }
}

/// 信息性报告：硬件支持时间戳时回收已完成的 GPU 样本并打印 p95。
fn report_gpu_p95(renderer: &mut WgpuImageRenderer) {
    if renderer.gpu_timing_support() != GpuTimingSupport::Available {
        return;
    }
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    let mut samples_ns = Vec::new();
    while Instant::now() < deadline {
        for sample in renderer.poll_gpu_timings().unwrap() {
            if sample.gpu_time_ns > 0 {
                samples_ns.push(sample.gpu_time_ns as f64 / 1e6);
            }
        }
        if samples_ns.len() >= FRAMES {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    if samples_ns.is_empty() {
        return;
    }
    samples_ns.sort_by(|a, b| a.partial_cmp(b).expect("gpu times are finite"));
    let p95 = samples_ns[samples_ns.len() * 95 / 100];
    eprintln!(
        "GPU 时间戳样本 {} 个: p50={:.2}ms p95={p95:.2}ms（信息性，无头模式不作门禁）",
        samples_ns.len(),
        samples_ns[samples_ns.len() / 2],
    );
}
