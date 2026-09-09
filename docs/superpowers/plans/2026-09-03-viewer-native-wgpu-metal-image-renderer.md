# Viewer Native wgpu/Metal Image Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不新增产品功能、不改变用户可见行为和评审数据协议的前提下，把单图预览与图片评审的逐帧热路径迁移到原生 wgpu／Metal 渲染器，使缩放、平移、旋转、标记、命中检测和放大镜共享同一场景与同一坐标变换，并通过 8GB Apple Silicon 基线性能门禁。

> **执行状态（2026-09-09）：已完成。** 规划中的 Stage 1～6（边界/算法、wgpu/Metal 后端、macOS 视口与资源、Desktop/React 接入、评审与性能门禁、单引擎收口）均已完成，对应用户确认的三轮宏观开发阶段。最终证据、两轮真实窗口复验和未纳入范围的事项见 [`docs/quality/IMAGE_RENDERER_ACCEPTANCE.md`](../../quality/IMAGE_RENDERER_ACCEPTANCE.md)。

**Architecture:** 新增无平台依赖的 `viewer-render-core` 和只负责 GPU 绘制的 `viewer-render-wgpu`；`viewer-platform-macos` 负责 MTKView、AppKit 输入、Image I/O、色彩与内存压力；`viewer-desktop` 负责授权、生命周期和低频 Tauri 协议；React 继续负责产品外壳、工具栏、意见栏和文本输入。迁移期通过统一 RendererPort 双引擎共存，连续两轮完整验收通过后删除生产环境 Web 单图渲染器，不增加轻量兜底。

**Tech Stack:** Rust 2024、wgpu 30.0.1（Metal backend）、objc2／objc2-metal-kit／objc2-core-video 0.3.2、Tauri 2、React 19、TypeScript 6、Vitest、Cargo test、Node test、macOS Image I/O／Core Graphics／Core Video／AppKit。

**Spec:** [`docs/superpowers/specs/2026-09-03-viewer-native-wgpu-metal-image-renderer-design.md`](../specs/2026-09-03-viewer-native-wgpu-metal-image-renderer-design.md)

## Global Constraints

- 本计划只重构架构与性能，不增加产品功能，不改变工具、文案、快捷键、默认显示、评审流程或 Agent 可读取的数据含义。
- 默认适窗大小只由旋转后的图片宽高比和当前可用视口决定；像素总量、文件大小和图像内容不得进入适窗几何公式。
- 网格预览、对比视图和视频路径不在本轮迁移范围内；评审持久化、存档和外部 Agent 读取协议保持不变。
- 迁移期允许旧 Web 单图渲染器与原生渲染器并存；稳定后只保留原生引擎，不增加 CPU、Canvas 或其他轻量生产兜底。
- 不进行签名、公证、正式安装包、上架或发售工作；这些内容也不得作为本计划完成后的待办项。
- 任何文件路径只在 Rust 授权边界内解析；前端和渲染消息只能携带 `entityId`、会话标识、版本和几何数据。
- AppKit 对象只在主线程创建、挂载和销毁；wgpu 设备、队列、纹理和帧状态只由渲染 actor 持有。
- 不修改当前工作区中用户已有的未提交文件：`crates/viewer-infrastructure/src/review/continuous/migration_inspect.rs` 和 `crates/viewer-infrastructure/tests/continuous_review_migration.rs`。
- 每个任务先写会失败的测试，再写最小实现，再运行任务级测试和受影响的回归测试；每个任务单独提交。
- 只有任务 17 的两轮门禁均通过，才可执行任务 18；若门禁未通过，保留双引擎并修复根因，不降低指标。

## Stage 1 — 边界、协议与纯算法

### Task 1: 建立渲染 crate 边界和依赖政策

**Files:**

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/viewer-render-core/Cargo.toml`
- Create: `crates/viewer-render-core/src/lib.rs`
- Create: `crates/viewer-render-wgpu/Cargo.toml`
- Create: `crates/viewer-render-wgpu/src/lib.rs`
- Modify: `crates/viewer-platform-macos/Cargo.toml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `scripts/architecture-boundaries.mjs`
- Modify: `scripts/architecture-boundaries.test.mjs`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `THIRD_PARTY_NOTICES.md`

**Interfaces:**

- Consumes: 当前 Cargo workspace、依赖白名单、许可证清单。
- Produces: `viewer-render-core`（无内部生产依赖）、`viewer-render-wgpu -> viewer-render-core`、`viewer-platform-macos -> viewer-render-core + viewer-render-wgpu`、`viewer-desktop -> viewer-render-core + viewer-render-wgpu` 的单向依赖图。
- Produces dependency pins: `wgpu = 30.0.1`、`bytemuck = 1.25.1`、`objc2-metal-kit = 0.3.2`、`objc2-core-video = 0.3.2`；workspace `rust-version = "1.87"`，实际工具链继续使用已固定的 Rust 1.97.0。

- [ ] 在 `scripts/architecture-boundaries.mjs` 的测试旁先加入期望的新节点和边；在 `scripts/repository-policy.test.mjs` 中加入两个 manifest 和四个直接依赖名称，但暂不创建 crate。

- [ ] 运行边界与政策测试，确认因 manifest 缺失或 Cargo 图不匹配而失败：

```bash
node scripts/architecture-boundaries.mjs
node --test --test-name-pattern='workspace manifests declare the repository license|third-party notice inventory matches direct dependencies' scripts/repository-policy.test.mjs
```

- [ ] 在根 `Cargo.toml` 加入 workspace members 和依赖：

```toml
[workspace.dependencies]
bytemuck = { version = "1.25.1", features = ["derive"] }
objc2-metal-kit = { version = "0.3.2", default-features = false, features = ["std", "MTKView"] }
objc2-core-video = { version = "0.3.2", default-features = false, features = ["std", "CVDisplayLink"] }
wgpu = { version = "30.0.1", default-features = false, features = ["metal", "std", "wgsl"] }
```

- [ ] 创建两个 crate 的最小可编译入口；`viewer-render-core` 不依赖 Tauri、AppKit、Metal、存储或评审领域 crate，`viewer-render-wgpu` 只依赖 core、wgpu、bytemuck 和 thiserror。

```rust
// crates/viewer-render-core/src/lib.rs
#![forbid(unsafe_code)]

// crates/viewer-render-wgpu/src/lib.rs
#![deny(unsafe_op_in_unsafe_fn)]
```

- [ ] 在 macOS 平台 crate 中把 `objc2-metal-kit`、render core 和 wgpu renderer 设为 macOS target dependencies；在 desktop crate 中加入两个 render crate 的普通依赖，保持 Tauri 只存在于平台与 composition root。

- [ ] 更新 `THIRD_PARTY_NOTICES.md`：记录 `bytemuck`、`objc2-metal-kit`、`objc2-core-video`、`wgpu` 的锁定版本、许可证、用途和上游；运行 `cargo metadata` 生成 `Cargo.lock`。

- [ ] 运行并通过：

```bash
cargo metadata --locked --no-deps --format-version 1 >/dev/null
node scripts/architecture-boundaries.mjs
node --test --test-name-pattern='workspace manifests declare the repository license|third-party notice inventory matches direct dependencies' scripts/repository-policy.test.mjs
cargo check --locked -p viewer-render-core -p viewer-render-wgpu -p viewer-platform-macos -p viewer-desktop
```

- [ ] 提交：

```bash
git add Cargo.toml Cargo.lock crates/viewer-render-core crates/viewer-render-wgpu crates/viewer-platform-macos/Cargo.toml src-tauri/Cargo.toml scripts/architecture-boundaries.mjs scripts/repository-policy.test.mjs THIRD_PARTY_NOTICES.md
git commit -m "build: establish native image renderer boundaries"
```

### Task 2: 实现唯一适窗公式和坐标变换

**Files:**

- Create: `crates/viewer-render-core/src/geometry.rs`
- Create: `crates/viewer-render-core/src/camera.rs`
- Modify: `crates/viewer-render-core/src/lib.rs`
- Create: `crates/viewer-render-core/tests/fit_geometry.rs`
- Create: `crates/viewer-render-core/tests/coordinate_properties.rs`

**Interfaces:**

- Consumes: `SourceSize { width: u32, height: u32 }`、`ViewportLayout { logical_size, scale_factor, fit_inset }`、`CameraState`。
- Produces: `TransformSnapshot::new(source, viewport, camera) -> Result<TransformSnapshot, GeometryError>`。
- Produces: `image_to_view(NormalizedPoint) -> LogicalPoint`、`view_to_image(LogicalPoint) -> Option<NormalizedPoint>`、`visible_image_rect()`、`physical_viewport()`。
- Invariant: 同一旋转后宽高比在相同视口中必须得到相同适窗矩形，与源像素总量无关。

- [ ] 先添加同宽高比、不同像素值的适窗等价测试，以及 0/90/180/270 度往返投影属性测试：

```rust
#[test]
fn fit_depends_on_oriented_aspect_ratio_not_pixel_count() {
    let viewport = ViewportLayout::new(LogicalSize::new(1200.0, 800.0), 2.0, 0.9).unwrap();
    let small = TransformSnapshot::fit(SourceSize::new(300, 200).unwrap(), viewport, Rotation::Deg0).unwrap();
    let large = TransformSnapshot::fit(SourceSize::new(6570, 4380).unwrap(), viewport, Rotation::Deg0).unwrap();
    assert_eq!(small.fitted_rect(), large.fitted_rect());
}
```

- [ ] 运行测试并确认因类型和实现不存在而失败：

```bash
cargo test --locked -p viewer-render-core --test fit_geometry --test coordinate_properties
```

- [ ] 实现有限值验证的几何值对象、四分之一旋转、仿射正逆变换和以下适窗公式：

```rust
let aspect = oriented_width / oriented_height;
let available_width = viewport.logical_size.width * viewport.fit_inset;
let available_height = viewport.logical_size.height * viewport.fit_inset;
let width = available_width.min(available_height * aspect);
let height = width / aspect;
```

- [ ] 实现以光标为锚点的 `CameraState::zoom_at`、有界平移和 `fit` 重置；缩放范围保持现有 `0.1..=8.0`，适窗模式显示为 100%。

- [ ] 加入至少 10,000 个确定性样本的往返测试，断言归一化点经 image→view→image 后误差不超过 `1e-5`；无效尺寸、NaN、无穷值必须返回结构化错误。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-core --test fit_geometry --test coordinate_properties
cargo clippy --locked -p viewer-render-core --all-targets -- -D warnings
```

- [ ] 提交：

```bash
git add crates/viewer-render-core
git commit -m "feat(renderer): define canonical image transforms"
```

### Task 3: 定义保留式场景和有序消息协议

**Files:**

- Create: `crates/viewer-render-core/src/ids.rs`
- Create: `crates/viewer-render-core/src/scene.rs`
- Create: `crates/viewer-render-core/src/protocol.rs`
- Modify: `crates/viewer-render-core/src/lib.rs`
- Create: `crates/viewer-render-core/tests/protocol_ordering.rs`
- Create: `crates/viewer-render-core/tests/scene_patching.rs`

**Interfaces:**

- Consumes: 已有 `ReviewAnchor` 的六种图片语义：point、arrow、rect、stroke、ellipse、asset；asset 意见不进入空间场景。
- Produces exact identity types: `RenderSessionId(u64)`、`AssetGeneration(u64)`、`SceneRevision(u64)`、`CommandId(u64)`、`AnnotationId(String)`。
- Produces: `RenderEnvelope<T> { session_id, asset_generation, scene_revision, command_id, payload }`。
- Produces: `RevisionGate::classify(&RenderEnvelope<RenderCommand>) -> RevisionDecision`，返回 `Apply`、`IgnoreDuplicate`、`IgnoreStale` 或 `RequireSnapshot`。
- Produces: `SceneSnapshot`、`ScenePatch::{Upsert, Remove, ReplaceAll, SetSelection, SetDraft}`。

- [ ] 先写乱序、重复、跨素材 generation 和 revision 跳跃测试；再写 scene patch 幂等测试。

```rust
#[test]
fn a_patch_from_the_previous_asset_generation_is_ignored() {
    let mut gate = RevisionGate::new(RenderSessionId(7), AssetGeneration(3));
    let stale = envelope(AssetGeneration(2), SceneRevision(9), CommandId(44));
    assert_eq!(gate.classify(&stale), RevisionDecision::IgnoreStale);
}
```

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-render-core --test protocol_ordering --test scene_patching
```

- [ ] 实现场景数据结构；坐标一律保存为归一化图片坐标，笔画点数上限保持 2,048，序号、选中态、草稿态和现有红色标记样式都由 scene node 描述。

```rust
pub enum AnnotationGeometry {
    Point { position: NormalizedPoint },
    Arrow { tail: NormalizedPoint, head: NormalizedPoint },
    Rectangle { rect: NormalizedRect },
    Ellipse { rect: NormalizedRect },
    Stroke { points: Vec<NormalizedPoint> },
}

pub struct AnnotationNode {
    pub id: AnnotationId,
    pub ordinal: u32,
    pub geometry: AnnotationGeometry,
    pub selected: bool,
    pub draft: bool,
}
```

- [ ] 实现 revision gate：命令 ID 用于重试幂等；旧 generation 永不覆盖新素材；缺失 revision 触发完整场景重同步，不能猜测合并。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-core --test protocol_ordering --test scene_patching
cargo test --locked -p viewer-render-core
```

- [ ] 提交：

```bash
git add crates/viewer-render-core
git commit -m "feat(renderer): add retained scene protocol"
```

### Task 4: 迁移交互状态机和原生命中检测

**Files:**

- Create: `crates/viewer-render-core/src/interaction.rs`
- Create: `crates/viewer-render-core/src/hit_test.rs`
- Modify: `crates/viewer-render-core/src/lib.rs`
- Create: `crates/viewer-render-core/tests/interaction_sequences.rs`
- Create: `crates/viewer-render-core/tests/hit_testing.rs`

**Interfaces:**

- Consumes: `PointerSample { phase, location, pressure, modifiers, timestamp_ns }`、`ScrollSample`、`GestureSample`、`TransformSnapshot`、`SceneSnapshot`。
- Produces: `InteractionEvent::{CameraChanged, DraftStarted, DraftChanged, DraftCompleted, SelectionChanged, EditorPlacementChanged}`。
- Produces: `InteractionController::handle_input(&mut self, NativeInput, &TransformSnapshot, &SceneSnapshot) -> Vec<InteractionEvent>`。
- Produces: `HitIndex::rebuild(&SceneSnapshot)` 与 `hit_test(LogicalPoint, tolerance_px, &TransformSnapshot) -> Option<AnnotationId>`。

- [ ] 把现有 React reducer 的关键序列转成 Rust 规格测试：浏览平移、空格临时平移、画笔、矩形、箭头、圆圈、选中、重绘、Escape；验证事件语义与当前 UI action 一致。

- [ ] 写命中检测测试：不同缩放下使用固定屏幕像素容差；重叠图元优先选中视觉最上层；不可见图元不命中；500 个标记使用空间索引而非全量扫描。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-render-core --test interaction_sequences --test hit_testing
```

- [ ] 实现 `InteractionMode::{Browse, Point, Arrow, Brush, Rectangle, Ellipse}`、手势仲裁和草稿状态机；只有完成有效几何后才产生 `DraftCompleted`。

- [ ] 实现 uniform-grid 空间索引；索引按归一化包围盒构建，查询时用唯一 transform 将屏幕容差换算为图片空间，不读取 DOM，不依赖图像内容。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-core --test interaction_sequences --test hit_testing
cargo test --locked -p viewer-render-core
```

- [ ] 提交：

```bash
git add crates/viewer-render-core
git commit -m "feat(renderer): move image interaction into core"
```

### Task 5: 实现 LOD、分块调度和预算模型

**Files:**

- Create: `crates/viewer-render-core/src/resources.rs`
- Create: `crates/viewer-render-core/src/cache.rs`
- Modify: `crates/viewer-render-core/src/lib.rs`
- Create: `crates/viewer-render-core/tests/resource_planning.rs`
- Create: `crates/viewer-render-core/tests/cache_budget.rs`

**Interfaces:**

- Consumes: `ResourceRequest { source_size, visible_normalized_rect, display_scale, viewport_physical_size }`。
- Produces: `ResourcePlan { strategy, level, required_tiles, prefetch_tiles }`。
- Produces: `TextureStrategy::{SingleTexture, Tiled { tile_size: 512 }}`，选择条件由设备 `max_texture_dimension_2d` 和完整纹理及全部 mip 层的预估 RGBA 字节数共同决定；完整链最多占当前 GPU 图片预算的四分之一。2026-09-06 依据定稿规范 §8.2 纠正原计划“40% 且只计 base”的冲突；实际 GPU mip、瓦片图集和防接缝边界仍须分别实现与验收，不能仅凭策略估算宣称完成。
- Produces: `MemoryBudget::baseline_8gb()`，图片常驻 GPU 纹理软上限 `256 MiB`；`BudgetedLru` 支持 pin 当前可见资源、回收预取和响应内存压力。

- [ ] 先写测试覆盖 1K、4K、8K、超出设备纹理上限、旋转视口、连续快速缩放和内存压力。

```rust
#[test]
fn eight_k_uses_tiles_when_a_single_texture_breaks_the_budget() {
    let plan = ResourcePlanner::new(DeviceLimits::new(16_384), MemoryBudget::baseline_8gb())
        .plan(request(SourceSize::new(7680, 4320).unwrap(), 1.0));
    assert!(matches!(plan.strategy, TextureStrategy::Tiled { tile_size: 512 }));
}
```

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-render-core --test resource_planning --test cache_budget
```

- [ ] 实现按屏幕采样密度选择最低足够 mip level 的算法；先请求可见 tile，再请求一圈邻接 tile；相同 tile 请求去重，过时 generation 可取消。

- [ ] 实现 CPU staging、GPU texture 和磁盘派生缓存的独立计账；压力等级 `Normal/Warning/Critical` 分别回收预取、非当前 generation、除当前可见资源外全部可重建资源。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-core --test resource_planning --test cache_budget
cargo clippy --locked -p viewer-render-core --all-targets -- -D warnings
```

- [ ] 提交：

```bash
git add crates/viewer-render-core
git commit -m "feat(renderer): add bounded image resource planning"
```

## Stage 2 — wgpu／Metal 绘制后端

### Task 6: 建立 Metal-only wgpu 设备与帧所有权

**Files:**

- Create: `crates/viewer-render-wgpu/src/device.rs`
- Create: `crates/viewer-render-wgpu/src/frame.rs`
- Create: `crates/viewer-render-wgpu/src/diagnostics.rs`
- Modify: `crates/viewer-render-wgpu/src/lib.rs`
- Create: `crates/viewer-render-wgpu/tests/frame_state.rs`

**Interfaces:**

- Consumes: macOS 提供的安全封装 `SurfaceHandles`、`PhysicalSize`、scale factor。
- Produces: `WgpuImageRenderer::new(RendererDescriptor) -> Result<WgpuImageRenderer, RendererInitError>`。
- Produces: `resize`、`apply_scene`、`upload_resource`、`render(FrameRequest) -> Result<FrameReceipt, RenderError>`。
- Produces: `FrameReceipt { frame_index, scene_revision, cpu_time_ns, gpu_time_ns, presented }`。

- [ ] 先写不需要真实 GPU 的 frame dirty-state 测试：没有 camera、scene、resource 或 surface 变化时不提交新帧；变化合并为一次 frame request。

- [ ] 添加 macOS opt-in smoke test，环境变量 `VIEWER_RUN_METAL_TESTS=1` 时要求 adapter backend 为 Metal，默认 CI 中只编译不启动窗口。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-render-wgpu --test frame_state
cargo test --locked -p viewer-render-wgpu --no-run
```

- [ ] 实现只启用 `wgpu::Backends::METAL` 的 instance、adapter、device、queue 和 surface 配置；不启用 Vulkan、GL 或浏览器 WebGPU backend。

- [ ] 把 surface acquisition 的 `Outdated/Lost/Timeout/OutOfMemory` 映射为不同恢复动作；`OutOfMemory` 直接产生终止错误，不能静默降级。

- [ ] 实现单 owner 的 `FrameState` 和三帧 ring buffer 基础；所有 wgpu 资源只能在 renderer 所在线程访问。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-wgpu --test frame_state
cargo clippy --locked -p viewer-render-wgpu --all-targets -- -D warnings
```

- [ ] 提交：

```bash
git add crates/viewer-render-wgpu
git commit -m "feat(renderer): initialize Metal wgpu backend"
```

### Task 7: 实现图片纹理、分块上传和图片 pass

**Files:**

- Create: `crates/viewer-render-wgpu/src/resources.rs`
- Create: `crates/viewer-render-wgpu/src/image_pass.rs`
- Create: `crates/viewer-render-wgpu/src/shaders/image.wgsl`
- Modify: `crates/viewer-render-wgpu/src/lib.rs`
- Create: `crates/viewer-render-wgpu/tests/resource_upload.rs`
- Create: `crates/viewer-render-wgpu/tests/image_pass_plan.rs`

**Interfaces:**

- Consumes: `DecodedResource::{WholeImage, Tile}`，像素格式固定为 premultiplied BGRA8 sRGB，附带 generation、level 和 tile key。
- Produces: `ResourceRegistry::upsert(DecodedResource) -> Result<ResourceHandle, UploadError>`。
- Produces: `ImagePass::encode(&mut RenderPass, &TransformSnapshot, &VisibleResources)`。
- Invariant: row pitch 按 wgpu 对齐规则处理；旧 generation 的上传完成也不得进入当前 registry。

- [ ] 先写 row-padding、tile UV、旋转后可见 tile 和旧 generation 上传丢弃测试。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-render-wgpu --test resource_upload --test image_pass_plan
```

- [ ] 实现 staging belt/ring、纹理创建、sampler、bind group 和资源替换；上传使用 `queue.write_texture` 或复用 staging buffer，不能为每帧重新创建纹理。

- [ ] 实现图片 shader：顶点由 core transform 生成，采样保持 sRGB 语义，透明图片用 premultiplied alpha；适窗与自由缩放都只改变 camera uniform。

- [ ] 在资源 registry 中记录确切 GPU bytes，并把数字写入 `FrameReceipt` 的 diagnostics；当前可见资源 pin 到 present 完成。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-wgpu --test resource_upload --test image_pass_plan
cargo test --locked -p viewer-render-wgpu
```

- [ ] 提交：

```bash
git add crates/viewer-render-wgpu
git commit -m "feat(renderer): draw bounded image resources"
```

### Task 8: 实现评审图元 GPU pass

**Files:**

- Create: `crates/viewer-render-wgpu/src/annotation_pass.rs`
- Create: `crates/viewer-render-wgpu/src/annotation_mesh.rs`
- Create: `crates/viewer-render-wgpu/src/shaders/annotation.wgsl`
- Modify: `crates/viewer-render-wgpu/src/lib.rs`
- Create: `crates/viewer-render-wgpu/tests/annotation_mesh.rs`
- Create: `crates/viewer-render-wgpu/tests/annotation_pass_plan.rs`

**Interfaces:**

- Consumes: `SceneSnapshot` 与唯一 `TransformSnapshot`。
- Produces: point、arrow、rectangle、ellipse、stroke、selection handles、draft 和 ordinal badge 的 retained GPU mesh。
- Produces: `OrdinalGlyphAtlas` 接口，平台提供 0–9 和必要标点的系统字形 R8 atlas；wgpu 后端只消费像素与 metrics。
- Invariant: camera 改变只更新 uniform；scene revision 未改变时不重新 tessellate 500 个标记。

- [ ] 先写各图元 mesh 的顶点/索引边界、固定屏幕线宽、箭头退化保护、2,048 点笔画和 scene revision 复用测试。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-render-wgpu --test annotation_mesh --test annotation_pass_plan
```

- [ ] 实现 CPU 侧有界 tessellation 和可复用 vertex/index buffers；增长按下一档容量扩容，缩小时不逐帧抖动分配。

- [ ] 实现 annotation shader 和 alpha blending，使红色、线宽、虚线、序号圆点、选中框和控制柄与现有视觉验收容差一致。

- [ ] 加入 offscreen Metal 视觉 fixture：固定场景渲染到纹理，读取像素后与基准图进行结构差异比较；真实 GPU 执行仍由 `VIEWER_RUN_METAL_TESTS=1` 控制。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-wgpu --test annotation_mesh --test annotation_pass_plan
VIEWER_RUN_METAL_TESTS=1 cargo test --locked -p viewer-render-wgpu --test annotation_pass_plan -- --nocapture
```

- [ ] 提交：

```bash
git add crates/viewer-render-wgpu
git commit -m "feat(renderer): render retained review annotations"
```

### Task 9: 实现同场景放大镜与显示刷新调度

**Files:**

- Create: `crates/viewer-render-wgpu/src/magnifier_pass.rs`
- Create: `crates/viewer-render-wgpu/src/scheduler.rs`
- Modify: `crates/viewer-render-wgpu/src/frame.rs`
- Modify: `crates/viewer-render-wgpu/src/lib.rs`
- Create: `crates/viewer-render-wgpu/tests/magnifier_composition.rs`
- Create: `crates/viewer-render-wgpu/tests/frame_scheduler.rs`

**Interfaces:**

- Consumes: 当前图片 bind groups、annotation mesh、scene revision、cursor focus 和 magnifier preferences。
- Produces: `MagnifierPass::encode`，第二次绘制同一图片与标记资源到圆形 clip 区域。
- Produces: `FrameScheduler::on_display_tick(timestamp_ns)`；刷新率由显示器回调提供，不硬编码 60 Hz。
- Invariant: 放大镜不创建完整图片副本，不使用不同 scene revision，不遮掉与评审框相交的部分。

- [ ] 先写资源身份测试，断言主画面和放大镜 pass 使用相同 `ResourceHandle`、`SceneRevision` 和 annotation buffers；再写 60/120 Hz tick 合并测试。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-render-wgpu --test magnifier_composition --test frame_scheduler
```

- [ ] 实现圆形 stencil/clip、边框与同场景二次 draw；magnifier 只增加 pass，不增加整图纹理。

- [ ] 实现 display-link 驱动的 scheduler：输入可以高频更新 camera，但每个显示 tick 最多 present 一帧；静止时不空转渲染。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-render-wgpu --test magnifier_composition --test frame_scheduler
cargo clippy --locked -p viewer-render-wgpu --all-targets -- -D warnings
```

- [ ] 提交：

```bash
git add crates/viewer-render-wgpu
git commit -m "feat(renderer): compose magnifier in the retained scene"
```

## Stage 3 — macOS 原生视口、输入与资源

### Task 10: 挂载 MTKView 并建立严格生命周期

**Files:**

- Create: `crates/viewer-platform-macos/src/image_render/mod.rs`
- Create: `crates/viewer-platform-macos/src/image_render/surface.rs`
- Create: `crates/viewer-platform-macos/src/image_render/host.rs`
- Create: `crates/viewer-platform-macos/src/image_render/display_link.rs`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Create: `crates/viewer-platform-macos/tests/image_render_surface_geometry.rs`
- Create: `crates/viewer-platform-macos/tests/image_render_lifecycle.rs`

**Interfaces:**

- Consumes: `tauri::WebviewWindow` 和低频 `SurfaceLayout { left, top, width, height, scale_factor }`。
- Produces: `MacImageRenderHost::mount(&WebviewWindow, SurfaceLayout) -> Result<Self, SurfaceError>`。
- Produces: `set_layout`、`set_visible`、`start_display_link`、`stop_display_link`、`unmount`。
- Invariant: WKWebView 位于 MTKView 上方并保持透明；所有 AppKit 操作通过主线程调度，渲染 actor 不持有未封装 AppKit 引用。

- [ ] 复用 video surface 的几何转换测试模式，先写顶部原点 DOM rect 到底部原点 AppKit rect、Retina scale、窗口 resize、显示器切换和幂等 unmount 测试。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-platform-macos --test image_render_surface_geometry --test image_render_lifecycle
```

- [ ] 创建 `MTKView`，插入 WKWebView 下方；只让单图 stage 区域透明，工具栏、意见栏、文本编辑器继续由 React 绘制。

- [ ] 用安全 wrapper 提供 wgpu raw display/window handles；每个 unsafe block 写明 NSView 生命周期、retain 关系和线程不变量。

- [ ] 实现 CVDisplayLink/显示器刷新回调到 renderer actor 的有界信号；切换显示器时重建显示链接节奏，不能固定 16 ms timer。

- [ ] 在 AppKit 主线程用现有系统字体一次性生成 `OrdinalGlyphAtlas`，把不可变 R8 像素与 glyph metrics 交给 wgpu；序号字形生成不进入逐帧路径。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-platform-macos --test image_render_surface_geometry --test image_render_lifecycle
cargo clippy --locked -p viewer-platform-macos --all-targets -- -D warnings
```

- [ ] 提交：

```bash
git add crates/viewer-platform-macos/src/image_render crates/viewer-platform-macos/src/lib.rs crates/viewer-platform-macos/tests/image_render_surface_geometry.rs crates/viewer-platform-macos/tests/image_render_lifecycle.rs
git commit -m "feat(renderer): host Metal image surface on macOS"
```

### Task 11: 把高频指针与触控板输入移出 React

**Files:**

- Create: `crates/viewer-platform-macos/src/image_render/input.rs`
- Modify: `crates/viewer-platform-macos/src/image_render/host.rs`
- Modify: `crates/viewer-platform-macos/src/image_render/mod.rs`
- Create: `crates/viewer-platform-macos/tests/image_render_input_routing.rs`

**Interfaces:**

- Consumes: NSWindow local event monitor、当前 surface rect、`Vec<InputExclusionRect>`、当前 tool。
- Produces: `MacInputRouter::classify(WindowInput) -> RouteDecision::{Render(NativeInput), WebView, Ignore}`。
- Produces: renderer semantic events only；pointer-move、scroll 和 magnify samples 不经过 Tauri/React。
- Invariant: 工具栏、意见栏、InlineFeedbackEditor 和其保存/取消按钮始终路由到 WebView；stage 内输入按 tool 路由到 renderer。

- [ ] 先写输入矩阵测试：surface 内外、editor exclusion 内外、浏览/绘制工具、空格临时平移、鼠标捕获离开 stage、窗口失焦。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-platform-macos --test image_render_input_routing
```

- [ ] 实现 NSWindow local event monitor；在 surface 内且不属于 exclusion 时转换为 core input，并按需要消费 NSEvent；其他事件原样返回给 WKWebView。

- [ ] 实现 pointer capture：按下在 stage 内后，拖动与抬起保持同一 owner；窗口失焦、Escape 或 host unmount 必须取消 capture 和草稿。

- [ ] 验证路由器中没有 Tauri invoke、React callback 或 DOM geometry read；surface/exclusion 只在布局或 editor 状态改变时更新。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-platform-macos --test image_render_input_routing
cargo test --locked -p viewer-platform-macos image_render
```

- [ ] 提交：

```bash
git add crates/viewer-platform-macos/src/image_render crates/viewer-platform-macos/tests/image_render_input_routing.rs
git commit -m "feat(renderer): route preview gestures natively"
```

### Task 12: 实现 Image I/O 金字塔、色彩归一和内存压力响应

**Files:**

- Create: `crates/viewer-platform-macos/src/image_render/decode.rs`
- Create: `crates/viewer-platform-macos/src/image_render/tile_cache.rs`
- Create: `crates/viewer-platform-macos/src/image_render/color.rs`
- Create: `crates/viewer-platform-macos/src/image_render/memory_pressure.rs`
- Modify: `crates/viewer-platform-macos/src/image/image_io.rs`
- Modify: `crates/viewer-platform-macos/src/image_render/mod.rs`
- Create: `crates/viewer-platform-macos/tests/image_render_decode.rs`
- Create: `crates/viewer-platform-macos/tests/image_render_cache.rs`

**Interfaces:**

- Consumes: Rust 授权层提供的 `AuthorizedImageSource`，不接收前端路径。
- Produces: `MacImageResourceProvider::probe`、`request_preview`、`request_tiles`、`cancel_generation`。
- Produces: 方向已标准化、premultiplied BGRA8 sRGB 的 `DecodedResource`；派生缓存 key 包含 source fingerprint、orientation、color profile、level、tile coordinate 和 renderer cache schema。
- Produces: memory pressure callback 到 `BudgetedLru::shrink`。

- [ ] 先写 fixture 测试覆盖 JPEG/PNG、透明度、EXIF 方向、相同宽高比不同像素、损坏文件、cache key 失效和取消旧 generation。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-platform-macos --test image_render_decode --test image_render_cache
```

- [ ] 从现有 Image I/O 实现提取共享 probe，不重复解析元数据；首帧先生成受视口约束的适窗预览，后台按 ResourcePlan 生成 512×512 tile 金字塔。

- [ ] 每次只保留一个正在切片的 level 解码缓冲，tile 写入原子临时文件后 rename；失败或取消不得留下可命中的半成品。

- [ ] 用 Core Graphics 明确绘制到 sRGB premultiplied BGRA8 buffer 并应用 EXIF 方向；保持现阶段 SDR，不悄悄引入 HDR 行为。

- [ ] 接入 macOS memory pressure source；Warning/Critical 行为严格调用 core 预算策略，并把回收前后字节数写入 diagnostics。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-platform-macos --test image_render_decode --test image_render_cache
cargo test --locked -p viewer-platform-macos
```

- [ ] 提交：

```bash
git add crates/viewer-platform-macos/src/image crates/viewer-platform-macos/src/image_render crates/viewer-platform-macos/tests/image_render_decode.rs crates/viewer-platform-macos/tests/image_render_cache.rs
git commit -m "feat(renderer): stream color-correct image levels"
```

## Stage 4 — Desktop 与 React 双引擎接入

### Task 13: 建立授权的 desktop runtime 与低频 IPC

**Files:**

- Create: `src-tauri/src/image_render_runtime.rs`
- Create: `src-tauri/src/image_render_events.rs`
- Create: `src-tauri/src/commands/image_render.rs`
- Create: `src-tauri/src/dto/image_render.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/dto/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/state.rs`
- Create: `tests/image_render_runtime.rs`
- Create: `tests/image_render_security_boundaries.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**

- Consumes: `ImageRenderCommandDto { session_id, asset_generation, scene_revision, command_id, command }`。
- Command variants: `Open { entity_id }`、`Close`、`SetSurface`、`SetInputExclusions`、`SetTool`、`SetScene`、`ApplyScenePatch`、`Camera`、`SetMagnifier`。
- Produces: `ImageRenderAckDto { disposition, accepted_revision, backend }`，backend 在迁移期为 `native` 或 `web`。
- Produces event `viewer://image-render`，仅发送 `Ready`、`FramePresented`、`CameraChanged`、`Draft*`、`SelectionChanged`、`EditorPlacementChanged`、`Recovering`、`Failed`；不发送高频原始输入。

- [ ] 先写 security test，断言 DTO JSON schema 中不存在 `path`、`sourcePath`、`url`；未经当前项目 session 授权的 entity、旧 generation 和旧 revision 被拒绝或忽略。

- [ ] 写 runtime lifecycle test：open→resize→scene→close、切换素材取消旧 decode、窗口关闭幂等销毁、同一命令重试只应用一次。

- [ ] 运行并确认失败：

```bash
cargo test --locked -p viewer-desktop --test image_render_runtime --test image_render_security_boundaries
```

- [ ] 在 desktop state 中增加只对 image render runtime 可见的 `authorized_image_source(entity_id)`；复用当前项目索引解析路径，返回拥有明确生命周期的授权 source，不把路径序列化给 UI。

- [ ] 实现 `ImageRenderRuntime` actor：desktop 校验 envelope 后解析 source，主线程挂载 host，decode worker 生成资源，renderer actor 按 generation 接收；close 时按 input→display link→renderer→surface 顺序退出。

- [ ] 注册一个 `image_render_command` Tauri command 和一个事件通道，避免为每个动作增加无序 invoke；在现有 window close/project close 路径中加入 renderer cleanup。

- [ ] 运行并通过：

```bash
cargo test --locked -p viewer-desktop --test image_render_runtime --test image_render_security_boundaries
pnpm architecture:contracts
cargo clippy --locked -p viewer-desktop --all-targets -- -D warnings
```

- [ ] 提交：

```bash
git add src-tauri/src/image_render_runtime.rs src-tauri/src/image_render_events.rs src-tauri/src/commands src-tauri/src/dto src-tauri/src/lib.rs src-tauri/src/state.rs src-tauri/Cargo.toml tests/image_render_runtime.rs tests/image_render_security_boundaries.rs
git commit -m "feat(renderer): add authorized image render runtime"
```

### Task 14: 在前端建立 RendererPort 和双引擎会话

**Files:**

- Create: `ui/src/rendering/imageRendererTypes.ts`
- Create: `ui/src/rendering/imageRendererPort.ts`
- Create: `ui/src/rendering/nativeImageRenderer.ts`
- Create: `ui/src/rendering/webImageRenderer.ts`
- Create: `ui/src/rendering/imageRendererSession.ts`
- Create: `ui/src/rendering/imageRendererPort.test.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/viewer.test.ts`

**Interfaces:**

- Consumes TypeScript contract:

```ts
export interface ImageRendererPort {
  readonly backend: 'native' | 'web'
  open(request: OpenImageRendererRequest): Promise<ImageRendererSession>
  listen(handler: (event: ImageRendererEvent) => void): Promise<() => void>
}

export interface ImageRendererSession {
  readonly sessionId: string
  readonly assetGeneration: number
  dispatch(command: ImageRendererCommand): Promise<ImageRendererAck>
  close(): Promise<void>
}
```

- Produces: `createImageRendererPort({ bridge, migrationPolicy })`；只有这里决定 native/web，产品组件不读取环境变量。
- Migration policy: native 初始化失败或同一会话连续两次资源恢复失败才切到 Web；后续任务 18 删除此 policy 和 Web adapter。

- [ ] 先写 contract tests：open/close 幂等、command/revision 单调、事件过滤、native 初始化失败切 Web、一次恢复失败不切 Web、两次恢复失败切 Web。

- [ ] 更新 `ui/src/api/viewer.test.ts`，先要求所有 Tauri imports 仍只出现在 `ui/src/api/viewer.ts`，并要求 bridge 暴露 `imageRenderCommand` 与 `listenImageRender`。

- [ ] 运行并确认失败：

```bash
pnpm --dir ui exec vitest run src/rendering/imageRendererPort.test.ts src/api/viewer.test.ts
```

- [ ] 在 `viewer.ts` 中加入单一 invoke/listen transport；renderer 模块只依赖 typed bridge，不直接导入 Tauri。

- [ ] 实现 native session 和 legacy Web session 的相同有序协议；Web adapter 包装现有行为，不复制几何算法，不改变产品默认行为。

- [ ] 运行并通过：

```bash
pnpm --dir ui exec vitest run src/rendering/imageRendererPort.test.ts src/api/viewer.test.ts
pnpm architecture:boundaries
```

- [ ] 提交：

```bash
git add ui/src/rendering ui/src/api/viewer.ts ui/src/api/viewer.test.ts
git commit -m "feat(renderer): hide image engines behind a port"
```

### Task 15: 把普通单图预览接入原生视口

**Files:**

- Create: `ui/src/components/imagePreview/ImagePreviewChrome.tsx`
- Create: `ui/src/components/imagePreview/NativeImageViewport.tsx`
- Create: `ui/src/components/imagePreview/WebImageViewport.tsx`
- Create: `ui/src/components/imagePreview/NativeImageViewport.test.tsx`
- Modify: `ui/src/components/imagePreview/ImagePreviewSurface.tsx`
- Modify: `ui/src/components/imagePreview/ImagePreviewSurface.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/App.css`

**Interfaces:**

- Consumes: `ImageRendererSession`、`BrowserFile`、现有 toolbar/navigation/magnifier settings。
- Produces: `NativeImageViewport` 只发送 open、surface layout、camera intents 和 magnifier preferences；不注册 pointermove、wheel 或 gesture handlers。
- Produces: 共同 chrome 继续显示现有名称、尺寸、适应窗口、缩放、旋转、浏览和返回网格控件。
- Invariant: default fit 保持现有 0.9 inset，且同一比例的图片默认画面尺寸相同。

- [ ] 先把现有 `ImagePreviewSurface` 行为测试复制为 engine contract cases；增加 native case，断言 mount 后没有高频 DOM pointer handler，ResizeObserver 只在 rect 改变时发命令。

- [ ] 加入用户已明确要求的适窗回归：`300×200` 与 `6570×4380` 在同一 viewport 中产生相同 native surface image rect；文件大小和 representation backend 改变不影响几何。

- [ ] 运行并确认失败：

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/NativeImageViewport.test.tsx src/components/imagePreview/ImagePreviewSurface.test.tsx
```

- [ ] 把 toolbar、导航、loading/error chrome 从当前组件提取；把现有 DOM `<img>`、`useImageViewport`、`usePreviewGestures` 和 Canvas magnifier 原样移入 `WebImageViewport`，作为迁移期实现。

- [ ] 实现 `NativeImageViewport` 的透明 stage 与低频 rect 同步；收到 `Ready` 后无全屏 loading 跳切，切图时保留 chrome 并由 native 首帧事件更新。

- [ ] 连接适窗、缩放、旋转和 magnifier 控件到 semantic commands；键盘导航和 focus restore 保持现有行为。

- [ ] 运行并通过：

```bash
pnpm --dir ui exec vitest run src/components/imagePreview src/components/ImagePreview.test.tsx
pnpm --dir ui check
```

- [ ] 提交：

```bash
git add ui/src/components/imagePreview ui/src/components/ImagePreview.tsx ui/src/App.css
git commit -m "feat(renderer): integrate native single image viewport"
```

## Stage 5 — 评审、恢复与性能门禁

### Task 16: 把评审创建、编辑、保存和放大镜接入同一原生场景

**Files:**

- Create: `ui/src/app/review/nativeReviewScene.ts`
- Create: `ui/src/app/review/nativeReviewScene.test.ts`
- Create: `ui/src/components/review/NativeReviewViewportBridge.tsx`
- Create: `ui/src/components/review/NativeReviewViewportBridge.test.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkspace.tsx`
- Modify: `ui/src/components/review/InlineFeedbackEditor.tsx`
- Modify: `ui/src/app/review/useImageReviewWorkbench.ts`
- Modify: `ui/src/app/review/imageReviewWorkbenchAdapter.ts`
- Modify: `ui/src/app/review/imageReviewWorkbenchAdapter.test.ts`

**Interfaces:**

- Consumes: `ImageReviewWorkbenchFeedback[]`、`AnnotationInteractionState` 和 native semantic events。
- Produces: `toNativeScene(feedback, editor, revision) -> ImageRendererScene`；保持 ordinal 排序和所有 ReviewAnchor 坐标不变。
- Produces: renderer events 到现有 reducer action 的纯映射 `toAnnotationAction(event) -> AnnotationEditorAction | null`。
- Save protocol: UI 先用 `clientMutationId` 提交 provisional scene patch；持久化成功后原子替换成服务端 item id/revision；失败则保留草稿并进入现有 `save_error`。

- [ ] 先写所有 anchor 类型的双向转换测试，断言归一化坐标逐值相等、stroke 顺序不变、asset 意见不进入 viewport scene。

- [ ] 写完整交互测试：绘制→弹出文字框→保存、取消、编辑文字、调整区域、重画、删除；按钮位于 input exclusion 中，在 100%/200%/380% 和平移后仍可点击。

- [ ] 写保存性能语义测试：点击保存后的下一 renderer patch 立即包含 provisional 标记；持久化 Promise 未完成时不清空画面、不切 loading；失败时保留文字和几何供重试。

- [ ] 运行并确认失败：

```bash
pnpm --dir ui exec vitest run src/app/review/nativeReviewScene.test.ts src/components/review/NativeReviewViewportBridge.test.tsx src/app/review/imageReviewWorkbenchAdapter.test.ts
```

- [ ] 实现 scene converter 和 event mapper；`ImageReviewWorkspace` 在 native backend 时不挂载 `AnnotationCanvas` 或 DOM magnifier painter，Web backend 迁移期保持原实现。

- [ ] 由 `EditorPlacementChanged` 定位 InlineFeedbackEditor，并把编辑器 client rect 作为 input exclusion 发给 native host；编辑器打开期间 camera 变化时只更新 placement，不夺取文本输入。

- [ ] 实现 provisional→durable scene patch；不改变 continuous review 的保存、存档、历史和 Agent 输出协议，也不让 renderer 等待 publication/materialization。

- [ ] 运行并通过：

```bash
pnpm --dir ui exec vitest run src/app/review src/components/review
pnpm test:review-protocol
pnpm test:review-loop
```

- [ ] 提交：

```bash
git add ui/src/app/review ui/src/components/review
git commit -m "feat(renderer): integrate review scene with native viewport"
```

### Task 17: 完成恢复策略、诊断、两轮全量验收和性能证据

> 2026-09-09 已完成：压力期间标记上传的原子预留与可见场景保留、高清资源链路、GPU/上传内存记账、系统压力恢复、协议版本识别、输入与编辑器排除区、主线程计时和完整双场景收据均已通过针对性复核。`pnpm gate:m2`、`pnpm gate:image-render` 及 Rust/React 全量门禁通过；同一候选代码系列完成最新构建冷启动和关闭重启后的两轮真实窗口验收。完整证据、数值和已知边界见 `docs/quality/IMAGE_RENDERER_ACCEPTANCE.md`。

**Files:**

- Create: `scripts/image-render/generate-performance-fixtures.swift`
- Create: `scripts/image-render/run-performance-gate.sh`
- Create: `scripts/image-render/performance-gate.test.mjs`
- Create: `scripts/image-render/native-acceptance.test.mjs`
- Create: `tests/image_render_recovery.rs`
- Create: `tests/image_render_performance_gate.rs`
- Create: `docs/quality/IMAGE_RENDERER_ACCEPTANCE.md`
- Create: `docs/quality/image-renderer-baseline.json`
- Modify: `package.json`
- Modify: `src-tauri/src/image_render_runtime.rs`
- Modify: `crates/viewer-render-wgpu/src/diagnostics.rs`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `scripts/viewer-native-acceptance.mjs`

**Interfaces:**

- Consumes: deterministic 8K fixture、500 个混合标记、60/120 Hz display diagnostics、cache cold/warm run、注入的 surface/device/resource failure。
- Produces: machine-readable receipt，至少包含 `p50/p95/p99 frame_ms`、`presented_fps`、`first_interactive_ms`、`warm_first_interactive_ms`、`gpu_texture_bytes_peak`、`dropped_input_samples`、`recovery_count`、`save_commit_ms`。
- Produces scripts: `pnpm test:image-render`、`pnpm gate:image-render`；gate 任一硬指标失败时非零退出。
- Hard gates: 8K+500 annotations 持续缩放/平移 60 FPS、60 Hz p95 ≤16.7 ms、调度不锁死 60 Hz、冷首帧 ≤300 ms、暖缓存 ≤100 ms、GPU 图片纹理峰值约束 ≤256 MiB、正常本地存储 save p95 ≤100 ms/p99 ≤300 ms。

- [x] 先写 gate parser tests，给每个超限字段提供失败 fixture，并验证缺字段也失败；再写 injected recovery test，验证恢复始终留在 native，并由诊断状态提供重试。

- [ ] 运行并确认失败：

```bash
node --test scripts/image-render/performance-gate.test.mjs scripts/image-render/native-acceptance.test.mjs
cargo test --locked -p viewer-desktop --test image_render_recovery --test image_render_performance_gate
```

- [x] 实现 diagnostics 聚合和 JSON receipt；性能统计使用 present timestamp 与 GPU timestamp query（设备不支持时明确标记 unavailable，但 frame gate 仍使用 present 时间），不能只报平均值。

- [x] 实现 deterministic fixture generator 和真实应用驱动脚本；自动 fixture 不依赖个人 Downloads 路径，手工验收可通过 `VIEWER_IMAGE_RENDER_FIXTURE_ROOT` 指向用户的“下载/测试图”。

- [x] 将 PRE-08、RVW-16～20、RVW-24、RVW-33～37 纳入 native acceptance；浏览器视觉验收使用注入式测试 renderer，不参与桌面生产组合。

- [x] 运行第一轮完整验收并把 receipt、机器型号、内存、显示刷新率和候选代码系列写入 `IMAGE_RENDERER_ACCEPTANCE.md`：

```bash
pnpm gate:image-render
pnpm gate:m1
pnpm gate:m2
pnpm test:visual-acceptance
pnpm test:native-acceptance
pnpm quality
```

- [x] 关闭并重新启动应用，清空进程内状态但保留派生缓存，运行第二轮相同验收；两轮均无阻断问题，性能 gate 独立通过。

- [x] 对两轮结果执行基线稳定性检查；基线文件未被修改，所有超限项均未出现。

- [x] 运行最终本阶段验证：

```bash
pnpm verify
git status --short
```

- [ ] 提交（只加入本任务文件，不加入用户原有未提交文件）：

```bash
git add scripts/image-render tests/image_render_recovery.rs tests/image_render_performance_gate.rs docs/quality/IMAGE_RENDERER_ACCEPTANCE.md docs/quality/image-renderer-baseline.json package.json src-tauri/src/image_render_runtime.rs crates/viewer-render-wgpu/src/diagnostics.rs ui/src/acceptance/acceptanceStateCatalog.json scripts/viewer-native-acceptance.mjs
git commit -m "test(renderer): enforce native image performance gates"
```

## Stage 6 — 单引擎收口

### Task 18: 删除生产 Web 单图渲染器和迁移开关

**Precondition:** `docs/quality/IMAGE_RENDERER_ACCEPTANCE.md` 中记录了同一候选 commit 系列的连续两轮完整验收，两轮均通过任务 17 的全部硬门禁；没有未解决的阻断级 native renderer 问题。

**Files:**

- Delete: `ui/src/rendering/webImageRenderer.ts`
- Delete: `ui/src/components/imagePreview/WebImageViewport.tsx`
- Delete if no remaining production import: `ui/src/components/imagePreview/ImageMagnifier.tsx`
- Delete if no remaining production import: `ui/src/components/imagePreview/useCurrentOriginal.ts`
- Delete if no remaining production import: `ui/src/components/imagePreview/useImageViewport.ts`
- Delete if no remaining production import: `ui/src/components/imagePreview/usePreviewGestures.ts`
- Delete if no remaining production import: `ui/src/components/review/AnnotationCanvas.tsx`
- Modify: `ui/src/rendering/imageRendererPort.ts`
- Modify: `ui/src/rendering/imageRendererSession.ts`
- Modify: `ui/src/components/imagePreview/ImagePreviewSurface.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkspace.tsx`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `scripts/architecture-boundaries.mjs`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `docs/quality/IMAGE_RENDERER_ACCEPTANCE.md`

**Interfaces:**

- Consumes: 已通过门禁的 native `ImageRendererPort`。
- Produces: production `createImageRendererPort()` 只返回 native 实现；初始化或恢复失败显示可诊断错误与重试，不存在 Web/Canvas/CPU 图片视口降级。
- Preserves: 网格、对比和视频仍可使用它们各自原有实现；删除范围只针对普通单图预览和图片评审的旧热路径。

- [x] 先修改 port contract test，要求 production factory 只有 `native` backend，注入初始化错误时返回 typed failure 而不是 Web session。

- [x] 使用 `rg` 精确确认候选旧文件只被单图/评审路径引用；网格、对比、视频和 acceptance 不受影响。

```bash
rg -n "webImageRenderer|WebImageViewport|ImageMagnifier|useCurrentOriginal|useImageViewport|usePreviewGestures|AnnotationCanvas" ui/src scripts
```

- [x] 删除 Web adapter、迁移开关和 fallback 计数；保留设备恢复、surface 重建、资源重传和用户可重试错误状态。

- [x] 桌面生产路径不再挂载 DOM `<img>` transform、Canvas annotation 或 Canvas magnifier；浏览器/单元测试保留隔离的 DOM oracle，不进入桌面组合根。

- [x] 加入 repository policy 断言：production UI 不得重新引入迁移开关，native 初始化失败不得选择 Canvas/Web engine。

- [x] 运行针对性验证：

```bash
pnpm --dir ui exec vitest run src/rendering src/components/imagePreview src/components/review src/app/review
node scripts/architecture-boundaries.mjs
node --test scripts/repository-policy.test.mjs
pnpm gate:image-render
```

- [x] 再运行全量验证：

```bash
pnpm verify
```

- [x] 更新 `IMAGE_RENDERER_ACCEPTANCE.md`，记录生产路径已收敛为原生单引擎、无轻量兜底以及最终验收证据；不添加签名、公证、正式安装包、上架或发售事项。

- [ ] 提交：

```bash
git add -A ui/src/rendering ui/src/components/imagePreview ui/src/components/review ui/src/acceptance scripts/architecture-boundaries.mjs scripts/repository-policy.test.mjs docs/quality/IMAGE_RENDERER_ACCEPTANCE.md
git commit -m "refactor(renderer): complete native single-engine cutover"
```

## Final Verification Matrix

执行者完成任务 18 后必须保留以下证据；任何失败都表示计划尚未完成：

```bash
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm test:review-protocol
pnpm test:review-loop
pnpm architecture:boundaries
pnpm gate:m1
pnpm gate:m2
pnpm gate:image-render
pnpm test:visual-acceptance
pnpm test:native-acceptance
pnpm security
git status --short
```

最后一次 `git status --short` 允许继续显示用户原有的两处未提交 continuous-review 文件，但不得出现本计划产生的未提交文件。完成报告只描述架构迁移、验证结果和已知限制，不列出签名、公证、正式安装包、上架或发售工作。
