# Viewer Bundled Video Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a clean-machine, high-performance macOS video preview powered by a Viewer-bundled LGPL libmpv runtime while preserving a platform-neutral boundary for a later Windows adapter.

**Architecture:** React owns browse and control presentation, but never receives decoded frames. Typed Tauri commands drive a generation-scoped `VideoPreviewService`; `viewer-video-mpv` dynamically loads the exact bundled runtime, and `viewer-platform-macos` mounts a GPU-backed native render surface coordinated to a DOM rectangle. Metadata and thumbnail work runs in a single low-priority worker backed by a safe 1 GiB Viewer cache.

**Tech Stack:** Rust 1.85 / edition 2024, Tauri 2, React 19, TypeScript, libmpv 0.41.0 Render API, FFmpeg n8.0, libplacebo 6.338.2, VideoToolbox, AppKit/OpenGL, SQLite, Vitest, Cargo tests.

## Global Constraints

- The minimum supported macOS version remains macOS 13.
- This delivery implements macOS completely; Windows receives platform-neutral interfaces and resource-layout conventions only.
- Bundle libmpv v0.41.0 from the official source archive with SHA-256 `ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209` and record tag commit `41f6a64`.
- Build mpv with `-Dgpl=false`, `-Dcplayer=false`, and `-Dlibmpv=true`; do not distribute a default GPL mpv build.
- Bundle FFmpeg n8.0 from SHA-256 `dd4030dbfdc34d9ff255a116bdd1caade42500ac2981efa27f8b151cc54c7b9e` with GPL, nonfree, network, ffplay, and runtime device enumeration disabled.
- Bundle libplacebo v6.338.2 from SHA-256 `2f1e624e09d72a8c9db70f910f7560e764a1c126dae42acc5b3bcef836a7aec6`.
- Every downloaded source archive and every bundled dynamic library must be represented in the checked-in runtime lock and third-party notices before packaging can pass.
- Release builds load libmpv, ffmpeg, and ffprobe only from the Viewer application bundle; they never search Homebrew, `PATH`, or user directories.
- Runtime options disable user mpv configuration, profiles, scripts, input bindings, URL/network playback, external subtitles, and automatic sidecar discovery.
- Only canonical local paths resolved from active Viewer entity identities may reach the engine; frontend commands never contain paths, mpv command strings, property names, or protocols.
- Decoded video frames never cross React, JavaScript, or Tauri IPC. IPC contains commands, bounded state events, native-surface geometry, and registered thumbnail artifact identifiers only.
- The first-frame gate hides the native surface until the correct frame has its final fit-to-window geometry; no enlarged card cover, small-image jump, black flash, or artificial loading delay is permitted.
- Use one primary playback decoder and one low-priority thumbnail worker. Thumbnail work pauses while playback is active and stale generations are canceled or ignored.
- Progress events are coalesced to at most 10 Hz; discrete ready, pause, end, error, volume, rate, fullscreen, and close events remain immediate.
- Every preview open starts at time zero, 1× speed, 100% volume, unmuted, no loop, and no automatic next video. Playback state and history are not persisted.
- Controls contain play/pause, backward/forward frame, timeline, elapsed/total time, volume/mute, speed values `0.5`, `0.75`, `1`, `1.25`, `1.5`, `2`, and fullscreen.
- Controls hide after approximately 2.5 seconds only while playing and idle; they remain visible while paused, seeking, focused, adjusting a value, or showing an error.
- Keyboard shortcuts are Space play/pause, Left/Right pause then frame-step, M mute, F fullscreen, and Escape fullscreen exit before preview exit; shortcuts do not capture text-input or unrelated-dialog focus.
- The video cache lives only under Viewer-owned system cache storage, has a fixed 1 GiB LRU budget, and may be cleared without interrupting the current decoded frame.
- `FileKind::Video` uses the new stable persisted value `7`; existing values `0..=6` are never renumbered.
- The mandatory Task 3 rendering feasibility gate must pass before Task 4 starts. Failure pauses implementation and reopens the approved design; it does not authorize JavaScript frame copies, an external player process, WebView codecs, or a different engine.
- Product code remains unchanged until implementation execution begins; this document is the sole change in the planning phase.

---

## Locked File Structure

### Runtime and native rendering

- `crates/viewer-video-mpv/src/ffi.rs`: checked-in bindings for the used libmpv v0.41.0 API surface only.
- `crates/viewer-video-mpv/src/loader.rs`: bundle-relative dynamic-library resolution and symbol loading.
- `crates/viewer-video-mpv/src/client.rs`: safe client/property/command wrapper with no raw-string escape hatch outside the crate.
- `crates/viewer-video-mpv/src/render.rs`: safe render-context wrapper and update callback ownership.
- `crates/viewer-video-mpv/src/process.rs`: exact-binary ffprobe/ffmpeg invocation, cancellation, and bounded output.
- `crates/viewer-video-mpv/src/runtime_manifest.rs`: checked-in source/runtime lock parsing and integrity rules.
- `crates/viewer-platform-macos/src/video/surface.rs`: AppKit native surface mount, CSS-to-AppKit geometry, Retina scale, z-order, and removal.
- `crates/viewer-platform-macos/src/video/adapter.rs`: macOS implementation of the application `VideoEngine` port.
- `crates/viewer-platform-macos/src/video/render_loop.rs`: mpv render update scheduling and main-thread presentation.
- `crates/viewer-platform-macos/src/video/diagnostics.rs`: selected decoder/render path and leak-counter snapshots.

### Domain, index, cache, and services

- `crates/viewer-domain/src/video.rs`: video metadata, probe status, normalized failure kinds, session/request identifiers.
- `crates/viewer-application/src/video.rs`: platform-neutral engine contracts and generation-scoped preview state machine.
- `crates/viewer-application/src/video_thumbnail.rs`: cover/timeline scheduling policy, quantization, and stale-result rejection.
- `crates/viewer-infrastructure/migrations/session/0002_video_metadata.sql`: additive companion table; no changes to 0.1.5 column encodings.
- `crates/viewer-infrastructure/src/video_probe.rs`: ffprobe JSON normalization and derived-work orchestration.
- `crates/viewer-infrastructure/src/video_cache.rs`: cache identity, verified root, LRU budget, usage, and clear behavior.
- `crates/viewer-infrastructure/src/video_thumbnail.rs`: deterministic cover sampling and quantized timeline artifact generation.

### Tauri bridge and lifecycle

- `src-tauri/src/video_runtime.rs`: independently managed active video runtime and deterministic close.
- `src-tauri/src/commands/video.rs`: explicit video commands; no generic command/property passthrough.
- `src-tauri/src/dto/video.rs`: camelCase request, event, metadata, error, thumbnail, and cache DTOs.
- `src-tauri/src/video_events.rs`: `viewer://video-event` publishing with 10 Hz progress coalescing.
- `src-tauri/src/state/video_index.rs`: low-priority probe/cover scheduling and session-artifact registration.

### React browse and preview

- `ui/src/components/contentBrowser/VideoSection.tsx`: disclosure, default expansion, empty omission, and selection container.
- `ui/src/components/contentBrowser/VideoCard.tsx`: stable card geometry, cover fade, play badge, duration, unavailable state.
- `ui/src/components/VideoPreview.tsx`: shell, native-surface slot, loading/error gate, navigation, and session lifecycle.
- `ui/src/components/videoPreview/videoState.ts`: reducer keyed by session generation.
- `ui/src/components/videoPreview/videoGeometry.ts`: fitted rectangle and CSS-to-native surface DTO.
- `ui/src/components/videoPreview/VideoControls.tsx`: approved control set, focus rules, and auto-hide integration.
- `ui/src/components/videoPreview/VideoTimeline.tsx`: seek input, thumbnail preview state, clamping, and accessibility values.
- `ui/src/components/videoPreview/useVideoBridge.ts`: command/event lifecycle and stale-generation filtering.
- `ui/src/components/videoPreview/useVideoShortcuts.ts`: shortcut ownership and text-input/dialog suppression.
- `ui/src/components/videoPreview/useVideoControlsVisibility.ts`: 2.5-second idle rule and reduced motion.
- `ui/src/styles/videoBrowser.css`: video section/card styles.
- `ui/src/styles/videoPreview.css`: preview/loading/control/timeline/fullscreen/reduced-motion styles.

The work stays in one delivery plan because runtime loading, native compositing, entity authorization, and UI session generations form one safety boundary. Task 3 is a hard stop separating technical feasibility from the product expansion.

---

### Task 1: Pin and Verify the Bundled LGPL Video Runtime

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/viewer-video-mpv/Cargo.toml`
- Create: `crates/viewer-video-mpv/src/lib.rs`
- Create: `crates/viewer-video-mpv/src/runtime_manifest.rs`
- Create: `crates/viewer-video-mpv/tests/runtime_manifest.rs`
- Create: `scripts/video/runtime.lock.json`
- Create: `scripts/video/build-macos-runtime.sh`
- Create: `scripts/video/verify-runtime.sh`
- Create: `docs/video-runtime-build.md`

**Interfaces:**
- Consumes: repository root, macOS target triple, official source URLs, and the exact source hashes in Global Constraints.
- Produces: `RuntimeLayout::from_bundle_root(root: &Path) -> Result<RuntimeLayout, RuntimeLayoutError>`, `RuntimeManifest::load(path: &Path) -> Result<RuntimeManifest, ManifestError>`, and a staged `ViewerVideoRuntime/` directory containing `lib/libmpv.2.dylib`, `lib/` dependencies, `bin/ffmpeg`, `bin/ffprobe`, `licenses/`, and `runtime.lock.json`.

- [ ] **Step 1: Add a failing runtime-manifest contract test**

```rust
#[test]
fn release_manifest_rejects_gpl_or_unhashed_components() {
    let manifest = RuntimeManifest::from_json(include_str!(
        "../../../scripts/video/runtime.lock.json"
    )).unwrap();
    assert_eq!(manifest.mpv.meson_options["gpl"], "false");
    assert!(manifest.components.iter().all(|item| item.sha256.len() == 64));
    assert!(manifest.components.iter().all(|item| item.license != "GPL"));
}
```

- [ ] **Step 2: Run the focused test and observe the missing crate failure**

Run: `cargo test -p viewer-video-mpv --test runtime_manifest`

Expected: FAIL because `viewer-video-mpv` and `RuntimeManifest` do not exist.

- [ ] **Step 3: Add the workspace crate and exact manifest schema**

```rust
#[derive(Debug, Deserialize)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub target: String,
    pub mpv: MpvBuild,
    pub ffmpeg: FfmpegBuild,
    pub components: Vec<LockedComponent>,
}

#[derive(Debug, Deserialize)]
pub struct LockedComponent {
    pub name: String,
    pub version: String,
    pub source_url: String,
    pub sha256: String,
    pub license: String,
}
```

Set `schemaVersion` to `1`, `target` to `universal-apple-darwin`, mpv to `v0.41.0`, FFmpeg to `n8.0`, libplacebo to `v6.338.2`, and encode the exact hashes from Global Constraints. The build script must exit before compilation when any archive digest differs or a component lacks a license entry.

- [ ] **Step 4: Add the reproducible macOS build script**

```bash
#!/usr/bin/env bash
set -euo pipefail
: "${VIEWER_VIDEO_WORK_DIR:?set VIEWER_VIDEO_WORK_DIR to an empty build directory}"
: "${VIEWER_VIDEO_STAGE_DIR:?set VIEWER_VIDEO_STAGE_DIR to the output directory}"
meson setup "$VIEWER_VIDEO_WORK_DIR/mpv-build" "$VIEWER_VIDEO_WORK_DIR/mpv-src" \
  --prefix "$VIEWER_VIDEO_STAGE_DIR" \
  -Dgpl=false -Dcplayer=false -Dlibmpv=true -Dbuild-date=false \
  -Dtests=false -Dfuzzers=false -Djavascript=disabled -Dlua=disabled \
  -Dcplugins=disabled -Dlibarchive=disabled -Dlibbluray=disabled \
  -Ddvdnav=disabled -Drubberband=disabled -Dvapoursynth=disabled \
  -Dgl=enabled -Dplain-gl=enabled -Dvideotoolbox-gl=enabled
ninja -C "$VIEWER_VIDEO_WORK_DIR/mpv-build" install
```

The same script builds the manifest-listed FFmpeg/libplacebo/libass dependency closure before mpv, rewrites every non-system install name to `@loader_path`, and stages ffmpeg/ffprobe from the reviewed FFmpeg build. It may use system SDK frameworks, `/usr/lib/libSystem.B.dylib`, and Apple platform libraries; every other dependency must resolve inside `ViewerVideoRuntime/lib`.

- [ ] **Step 5: Add bundle-layout and linkage verification**

```bash
test -x "$VIEWER_VIDEO_STAGE_DIR/bin/ffmpeg"
test -x "$VIEWER_VIDEO_STAGE_DIR/bin/ffprobe"
test -f "$VIEWER_VIDEO_STAGE_DIR/lib/libmpv.2.dylib"
otool -L "$VIEWER_VIDEO_STAGE_DIR/lib/libmpv.2.dylib" > "$VIEWER_VIDEO_STAGE_DIR/linkage.txt"
if rg -n '/opt/homebrew|/usr/local|Cellar' "$VIEWER_VIDEO_STAGE_DIR/linkage.txt"; then
  exit 1
fi
```

`verify-runtime.sh` must also run `ffmpeg -buildconf`, fail if output contains `--enable-gpl`, `--enable-nonfree`, or network support, and compare staged filenames plus SHA-256 digests to `runtime.lock.json`.

- [ ] **Step 6: Run manifest and shell verification**

Run: `cargo test -p viewer-video-mpv --test runtime_manifest && bash -n scripts/video/build-macos-runtime.sh scripts/video/verify-runtime.sh`

Expected: PASS; no source fetch or runtime build is required for this focused test.

- [ ] **Step 7: Build and verify one local arm64 runtime**

Run:

```bash
VIEWER_VIDEO_WORK_DIR="$(mktemp -d)" \
VIEWER_VIDEO_STAGE_DIR="$(pwd)/target/viewer-video-runtime/aarch64-apple-darwin" \
scripts/video/build-macos-runtime.sh
VIEWER_VIDEO_STAGE_DIR="$(pwd)/target/viewer-video-runtime/aarch64-apple-darwin" \
scripts/video/verify-runtime.sh
```

Expected: PASS with no Homebrew/user-directory linkage and a verified runtime inventory.

- [ ] **Step 8: Document source acquisition and commit**

```bash
git add Cargo.toml Cargo.lock crates/viewer-video-mpv scripts/video docs/video-runtime-build.md
git commit -m "build: pin LGPL video runtime"
```

---

### Task 2: Add a Safe libmpv Loader and Capability Wrapper

**Files:**
- Modify: `crates/viewer-video-mpv/Cargo.toml`
- Modify: `crates/viewer-video-mpv/src/lib.rs`
- Create: `crates/viewer-video-mpv/src/ffi.rs`
- Create: `crates/viewer-video-mpv/src/loader.rs`
- Create: `crates/viewer-video-mpv/src/client.rs`
- Create: `crates/viewer-video-mpv/src/render.rs`
- Create: `crates/viewer-video-mpv/src/process.rs`
- Create: `crates/viewer-video-mpv/tests/client_contract.rs`
- Modify: `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Consumes: `RuntimeLayout` and the staged runtime from Task 1.
- Produces: `MpvLibrary::load(layout: &RuntimeLayout)`, `MpvClient::new`, `MpvClient::open_local_file`, typed playback methods, `MpvRenderContext`, `BundledMediaTools`, and no public arbitrary command/property API.

- [ ] **Step 1: Write a failing public-API contract test with a fake symbol table**

```rust
#[test]
fn open_applies_viewer_isolation_before_loading_media() {
    let fake = FakeMpv::default();
    let mut client = MpvClient::from_api(fake.api()).unwrap();
    client.open_local_file(Path::new("/tmp/sample.mp4")).unwrap();
    assert_eq!(fake.option("config"), Some("no"));
    assert_eq!(fake.option("load-scripts"), Some("no"));
    assert_eq!(fake.option("sub-auto"), Some("no"));
    assert_eq!(fake.option("network-timeout"), Some("0"));
    assert_eq!(fake.command_names(), vec!["loadfile"]);
}
```

- [ ] **Step 2: Run the contract test and verify missing wrapper symbols**

Run: `cargo test -p viewer-video-mpv --test client_contract`

Expected: FAIL because the safe wrapper modules do not exist.

- [ ] **Step 3: Check in the minimal v0.41.0 FFI surface and dynamic loader**

```rust
pub struct MpvApi {
    pub create: unsafe extern "C" fn() -> *mut mpv_handle,
    pub initialize: unsafe extern "C" fn(*mut mpv_handle) -> c_int,
    pub terminate_destroy: unsafe extern "C" fn(*mut mpv_handle),
    pub set_option_string: unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(*mut mpv_handle, *const *const c_char) -> c_int,
    pub observe_property: unsafe extern "C" fn(*mut mpv_handle, u64, *const c_char, mpv_format) -> c_int,
    pub wait_event: unsafe extern "C" fn(*mut mpv_handle, f64) -> *const mpv_event,
}
```

Use `libloading` and resolve only from `RuntimeLayout.libmpv`. Verify `mpv_client_api_version()` has the expected major API before creating a client. Unit tests inject `MpvApi`; release code never falls back to a process-global library name.

- [ ] **Step 4: Implement typed commands and properties**

```rust
impl MpvClient {
    pub fn play(&self) -> Result<(), MpvError>;
    pub fn pause(&self) -> Result<(), MpvError>;
    pub fn seek_absolute_us(&self, time_us: u64) -> Result<(), MpvError>;
    pub fn frame_step(&self, direction: FrameDirection) -> Result<(), MpvError>;
    pub fn set_volume_percent(&self, volume: u8) -> Result<(), MpvError>;
    pub fn set_muted(&self, muted: bool) -> Result<(), MpvError>;
    pub fn set_rate(&self, rate: PlaybackRate) -> Result<(), MpvError>;
}
```

`open_local_file` accepts only an already-canonical `&Path`, converts it directly to the one `loadfile` argument, and applies `vo=libmpv`, `hwdec=auto-safe`, `config=no`, `load-scripts=no`, `input-default-bindings=no`, `ytdl=no`, `autoload-files=no`, `sid=no`, `sub-auto=no`, `audio-file-auto=no`, `aid=auto`, and `loop-file=no` before initialization. Embedded/external subtitles remain hidden and mpv selects the first playable audio track without exposing track selection.

- [ ] **Step 5: Implement render-context and subprocess ownership**

```rust
pub trait RenderTarget {
    fn framebuffer(&self) -> i32;
    fn pixel_size(&self) -> (i32, i32);
    fn scale_factor(&self) -> f64;
}

pub struct BundledMediaTools {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    gate: Arc<Semaphore>,
}
```

The render callback sends a single coalesced wake signal and never renders on mpv's callback thread. `BundledMediaTools` uses exact absolute executable paths, `kill_on_drop(true)`, a one-permit semaphore, bounded stderr capture, `stdin(null)`, and no shell.

- [ ] **Step 6: Run crate tests and lint**

Run: `cargo test -p viewer-video-mpv && cargo clippy -p viewer-video-mpv --all-targets -- -D warnings`

Expected: PASS.

- [ ] **Step 7: Commit the safe engine boundary**

```bash
git add Cargo.toml Cargo.lock crates/viewer-video-mpv THIRD_PARTY_NOTICES.md
git commit -m "feat: add safe bundled libmpv boundary"
```

---

### Task 3: Pass the Mandatory macOS Render API Feasibility Gate

**Files:**
- Modify: `crates/viewer-platform-macos/Cargo.toml`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Create: `crates/viewer-platform-macos/src/video/mod.rs`
- Create: `crates/viewer-platform-macos/src/video/surface.rs`
- Create: `crates/viewer-platform-macos/src/video/render_loop.rs`
- Create: `crates/viewer-platform-macos/src/video/diagnostics.rs`
- Create: `crates/viewer-platform-macos/tests/video_surface_geometry.rs`
- Create: `src-tauri/src/video_feasibility.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `ui/src/acceptance/scenes/videoFeasibilityScene.tsx`
- Modify: `ui/src/acceptance/scenes/index.ts`
- Create: `scripts/video/run-render-feasibility.sh`
- Create: `docs/reviews/2026-08-10-video-render-feasibility.md`

**Interfaces:**
- Consumes: `MpvClient`, `MpvRenderContext`, `RenderTarget`, a Tauri `WebviewWindow`, and a CSS-pixel `SurfaceRect`.
- Produces: proven `MacVideoSurface::mount(window, rect)`, `MacVideoSurface::update_geometry`, `MacVideoSurface::hide/reveal/unmount`, `VideoRenderDiagnostics`, and a signed feasibility review with all gate rows marked PASS.

- [ ] **Step 1: Write failing Retina and coordinate-conversion tests**

```rust
#[test]
fn css_rect_converts_from_top_left_to_appkit_points() {
    let rect = SurfaceRect { x: 40.0, y: 120.0, width: 960.0, height: 540.0 };
    assert_eq!(
        appkit_frame(rect, 1200.0),
        AppKitFrame { x: 40.0, y: 540.0, width: 960.0, height: 540.0 }
    );
}

#[test]
fn backing_pixels_apply_retina_scale_once() {
    assert_eq!(backing_pixels((960.0, 540.0), 2.0), (1920, 1080));
}
```

- [ ] **Step 2: Run the geometry tests and verify missing native-surface types**

Run: `cargo test -p viewer-platform-macos --test video_surface_geometry`

Expected: FAIL because `SurfaceRect` and conversion helpers do not exist.

- [ ] **Step 3: Implement the native surface lifecycle**

```rust
pub struct MacVideoSurface {
    view: Retained<NSOpenGLView>,
    parent: Retained<NSView>,
    mounted: AtomicBool,
}

impl MacVideoSurface {
    pub fn mount(window: &WebviewWindow, rect: SurfaceRect) -> Result<Self, SurfaceError>;
    pub fn update_geometry(&self, rect: SurfaceRect) -> Result<(), SurfaceError>;
    pub fn reveal(&self) -> Result<(), SurfaceError>;
    pub fn hide(&self) -> Result<(), SurfaceError>;
    pub fn unmount(&self) -> Result<(), SurfaceError>;
}
```

All AppKit mutations execute on the main thread. The surface mounts behind the transparent WKWebView content, uses the window backing scale, starts hidden, ignores pointer events, and is removed idempotently before the render context and client are destroyed.

- [ ] **Step 4: Add the coalesced render loop and diagnostic counters**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoRenderDiagnostics {
    pub hwdec: String,
    pub video_output: String,
    pub active_clients: usize,
    pub active_render_contexts: usize,
    pub active_surfaces: usize,
    pub rendered_frames: u64,
}
```

The mpv update callback sets an atomic pending flag and schedules one main-thread draw. A completed draw clears the flag and calls `mpv_render_context_report_swap`. Counters decrement on every error and normal close path.

- [ ] **Step 5: Add a feature-gated Tauri feasibility route**

```rust
#[cfg(feature = "video-feasibility")]
#[tauri::command]
async fn run_video_feasibility(
    request: VideoFeasibilityRequest,
    window: tauri::WebviewWindow,
) -> Result<VideoFeasibilityReport, CommandError>;
```

The request accepts only fixture IDs `h264-1080p`, `hevc-portrait`, and `vfr-step`; Rust maps each ID to `tests/fixtures/videos/`. The acceptance scene supplies the final DOM rectangle, overlays React controls, and displays first-frame, frame-step, frame-back-step, hardware path, and leak counters.

- [ ] **Step 6: Run automated geometry and lifecycle tests**

Run: `cargo test -p viewer-platform-macos video_surface && pnpm ui:test -- videoFeasibilityScene`

Expected: PASS.

- [ ] **Step 7: Run the native feasibility matrix**

Run: `scripts/video/run-render-feasibility.sh`

Expected:

```text
PASS surface-at-dom-rect
PASS retina-resize
PASS react-overlay-z-order
PASS first-frame-ready
PASS frame-step-forward
PASS frame-step-backward
PASS h264-videotoolbox
PASS hevc-videotoolbox
PASS no-ipc-frame-buffer
PASS 30-mount-unmount-baseline
```

- [ ] **Step 8: Record the gate decision**

Write the actual machine, macOS version, sample hashes, selected `hwdec`/`vo`, before/after counters, and screenshot paths into `docs/reviews/2026-08-10-video-render-feasibility.md`. The final line must be exactly `Decision: PASS — product implementation may continue.`

If any matrix row fails, do not perform Step 9 and do not begin Task 4. Preserve logs, change the decision line to `Decision: FAIL — reopen the approved rendering design.`, and return the failure to the user.

- [ ] **Step 9: Commit the passed feasibility slice**

```bash
git add crates/viewer-platform-macos src-tauri ui/src/acceptance scripts/video docs/reviews/2026-08-10-video-render-feasibility.md
git commit -m "feat: prove native libmpv rendering"
```

---

### Task 4: Add Stable Video Domain, Browse, and Persistence Types

**Files:**
- Modify: `crates/viewer-domain/src/file.rs`
- Modify: `crates/viewer-domain/src/lib.rs`
- Create: `crates/viewer-domain/src/video.rs`
- Modify: `crates/viewer-application/src/metadata.rs`
- Modify: `crates/viewer-application/src/browse.rs`
- Modify: `crates/viewer-infrastructure/src/scan/file_classifier.rs`
- Modify: `crates/viewer-infrastructure/src/search/index/mod.rs`
- Modify: `crates/viewer-infrastructure/src/portable/markers.rs`
- Modify: `crates/viewer-infrastructure/src/search/index/schema.rs`
- Create: `crates/viewer-infrastructure/migrations/session/0002_video_metadata.sql`
- Modify: `crates/viewer-infrastructure/src/search/index/writer.rs`
- Modify: `crates/viewer-infrastructure/src/search/index/query.rs`
- Modify: `crates/viewer-infrastructure/src/search/index/projection.rs`
- Test: existing unit tests beside each modified Rust module

**Interfaces:**
- Consumes: stable file-kind values `0..=6` and the existing scan/index generation model.
- Produces: `FileKind::Video = 7`, `VideoMetadata`, `VideoProbeStatus`, video-aware `BrowserFile`, `FolderWorkspace::Content { images, videos, other_files }`, and additive SQLite video metadata.

- [ ] **Step 1: Add failing stable-encoding and classifier tests**

```rust
#[test]
fn video_uses_new_stable_value_without_shifting_existing_kinds() {
    assert_eq!(FileKind::Video.encode(), 7);
    assert_eq!(FileKind::Other.encode(), 6);
}

#[test]
fn common_local_video_extensions_are_candidates() {
    for name in ["a.mp4", "a.m4v", "a.mov", "a.mkv", "a.webm", "a.avi", "a.wmv",
                 "a.mpg", "a.mpeg", "a.ts", "a.mts", "a.m2ts", "a.flv", "a.ogv", "a.3gp"] {
        assert_eq!(classify(Path::new(name)), FileKind::Video);
    }
}
```

- [ ] **Step 2: Run focused domain and classifier tests**

Run: `cargo test -p viewer-domain file::tests && cargo test -p viewer-infrastructure file_classifier`

Expected: FAIL because `Video` is not represented.

- [ ] **Step 3: Add exact video domain types**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoMetadata {
    pub duration_us: Option<u64>,
    pub display_width: Option<u32>,
    pub display_height: Option<u32>,
    pub rotation_degrees: i16,
    pub frame_rate_millihertz: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub probe_status: VideoProbeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoProbeStatus { Pending, Ready, Failed(VideoFailureKind) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFailureKind {
    Unsupported, Damaged, Unreadable, Missing, EngineInitialization,
    DecodeFallbackFailed, RenderSurface, ThumbnailUnavailable,
}
```

Add UUID-backed `VideoSessionId` and `VideoThumbnailRequestId` beside existing identifier newtypes.

- [ ] **Step 4: Add the extension allowlist and browse partition**

```rust
pub struct BrowserFile {
    pub entity_id: EntityId,
    pub name: String,
    pub kind: FileKind,
    pub image_metadata: Option<ImageMetadata>,
    pub video_metadata: Option<VideoMetadata>,
}

pub enum FolderWorkspace {
    Content {
        images: Vec<BrowserFile>,
        videos: Vec<BrowserFile>,
        other_files: Vec<BrowserFile>,
    },
}
```

`SelectionTypeCounts` gains `videos: usize`; `ContentFolderCard` gains `video_count: usize`; `FileKind::is_other_file()` returns false for Video.

- [ ] **Step 5: Add and register the companion-table migration**

```sql
CREATE TABLE IF NOT EXISTS video_metadata (
    node_id BLOB PRIMARY KEY NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    duration_us INTEGER,
    display_width INTEGER,
    display_height INTEGER,
    rotation_degrees INTEGER NOT NULL DEFAULT 0,
    frame_rate_millihertz INTEGER,
    video_codec TEXT,
    audio_codec TEXT,
    probe_status INTEGER NOT NULL,
    failure_kind INTEGER,
    updated_generation INTEGER NOT NULL
);
```

Register migration 2 after migration 1. Query uses a `LEFT JOIN video_metadata`; writer uses `replace_video_metadata(node_id, metadata, generation)` inside the same transaction as node projection.

- [ ] **Step 6: Add a 0.1.5 migration regression test**

```rust
#[test]
fn opens_v1_index_then_adds_video_metadata_without_reencoding_nodes() {
    let db = open_fixture_index("session-v1").unwrap();
    assert_eq!(db.kind_for_name("sample.txt").unwrap(), FileKind::Text);
    assert!(db.has_table("video_metadata").unwrap());
}
```

- [ ] **Step 7: Run domain, application, and infrastructure suites**

Run: `cargo test -p viewer-domain -p viewer-application -p viewer-infrastructure`

Expected: PASS.

- [ ] **Step 8: Commit stable video indexing**

```bash
git add crates/viewer-domain crates/viewer-application crates/viewer-infrastructure
git commit -m "feat: index video candidates and metadata"
```

---

### Task 5: Probe Video Metadata Without Blocking Folder Publication

**Files:**
- Modify: `crates/viewer-application/src/scheduler.rs`
- Modify: `crates/viewer-infrastructure/Cargo.toml`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Create: `crates/viewer-infrastructure/src/video_probe.rs`
- Modify: `src-tauri/src/state/mod.rs`
- Modify: `src-tauri/src/state/scan_index.rs`
- Create: `src-tauri/src/state/video_index.rs`
- Create: `crates/viewer-test-support/src/video_fixtures.rs`
- Modify: `crates/viewer-test-support/src/lib.rs`
- Create: `tests/fixtures/videos/manifest.json`
- Test: `tests/m1_desktop_runtime.rs`

**Interfaces:**
- Consumes: Video candidates from Task 4 and `BundledMediaTools` from Task 2.
- Produces: `VideoProbe::probe(path, cancel) -> Result<VideoMetadata, VideoProbeError>`, scheduler class `DerivedWorkClass::VideoProbe`, and generation-safe index updates after initial workspace publication.

- [ ] **Step 1: Write failing probe-normalization tests**

```rust
#[test]
fn normalizes_rotated_vfr_video_without_audio() {
    let metadata = normalize_ffprobe(include_str!("fixtures/rotated-vfr-no-audio.json")).unwrap();
    assert_eq!(metadata.rotation_degrees, 90);
    assert_eq!(metadata.frame_rate_millihertz, Some(29_970));
    assert_eq!(metadata.audio_codec, None);
    assert_eq!(metadata.probe_status, VideoProbeStatus::Ready);
}

#[test]
fn missing_duration_and_frame_rate_remain_valid() {
    let metadata = normalize_ffprobe(include_str!("fixtures/no-duration.json")).unwrap();
    assert_eq!(metadata.duration_us, None);
    assert_eq!(metadata.frame_rate_millihertz, None);
}
```

- [ ] **Step 2: Run the probe tests and verify the missing normalizer**

Run: `cargo test -p viewer-infrastructure video_probe`

Expected: FAIL because `video_probe` is absent.

- [ ] **Step 3: Implement bounded ffprobe invocation and normalization**

```rust
pub async fn probe(
    &self,
    canonical_path: &Path,
    cancellation: CancellationToken,
) -> Result<VideoMetadata, VideoProbeError> {
    self.tools.ffprobe_json(
        canonical_path,
        &["-show_streams", "-show_format", "-show_entries",
          "stream=codec_type,codec_name,width,height,avg_frame_rate,r_frame_rate:stream_tags=rotate:stream_side_data=rotation:format=duration"]
    ).await.and_then(normalize_ffprobe)
}
```

Enforce a 15-second timeout, 4 MiB stdout limit, 64 KiB stderr limit, locale-independent number parsing, and cancellation that kills and awaits the child. Map invalid JSON, nonzero exit, missing file, permission denied, and unsupported stream to stable `VideoFailureKind` values.

- [ ] **Step 4: Add a low-priority scheduler class**

```rust
pub enum DerivedWorkClass {
    CurrentQuery,
    FolderPublication,
    VisibleDerived,
    TextIndex,
    VideoProbe,
    Fingerprint,
}
```

`VideoProbe` executes only after current query/folder publication and at most one probe runs concurrently. A generation check precedes both process launch and `replace_video_metadata`.

- [ ] **Step 5: Prove browse publication precedes probe completion**

```rust
#[tokio::test]
async fn video_candidate_is_published_pending_before_probe_finishes() {
    let harness = RuntimeHarness::with_blocked_video_probe();
    let workspace = harness.open_folder_with("clip.mp4").await;
    assert_eq!(workspace.videos()[0].video_metadata.as_ref().unwrap().probe_status,
               VideoProbeStatus::Pending);
    harness.release_probe(VideoMetadata::ready(2_000_000)).await;
    assert_eq!(harness.next_workspace().await.videos()[0].duration_us(), Some(2_000_000));
}
```

- [ ] **Step 6: Run focused runtime and infrastructure tests**

Run: `cargo test -p viewer-infrastructure video_probe && cargo test -p viewer-desktop --test m1_desktop_runtime video`

Expected: PASS.

- [ ] **Step 7: Commit asynchronous video probing**

```bash
git add crates/viewer-application crates/viewer-infrastructure crates/viewer-test-support src-tauri tests/fixtures/videos
git commit -m "feat: probe video metadata asynchronously"
```

---

### Task 6: Generate Covers and Timeline Frames in a Safe 1 GiB Cache

**Files:**
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Create: `crates/viewer-infrastructure/src/video_cache.rs`
- Create: `crates/viewer-infrastructure/src/video_thumbnail.rs`
- Create: `crates/viewer-infrastructure/tests/video_cache.rs`
- Create: `crates/viewer-infrastructure/tests/video_thumbnail.rs`
- Modify: `crates/viewer-infrastructure/src/image_cache.rs`
- Modify: `src-tauri/src/state/video_index.rs`
- Modify: `src-tauri/src/image_protocol.rs`

**Interfaces:**
- Consumes: canonical video identity/size/mtime, `BundledMediaTools`, `ImageArtifactRegistry`, playback-active signal, session/request generation.
- Produces: `VideoCache`, `VideoCacheKey`, `VideoCacheStats`, `VideoThumbnailService::cover`, `VideoThumbnailService::timeline`, and session-scoped `viewer-image` URLs for PNG artifacts.

- [ ] **Step 1: Write failing cache identity, LRU, and safe-clear tests**

```rust
#[test]
fn identity_changes_after_source_mtime_or_algorithm_version() {
    let a = VideoCacheKey::cover(source("a.mp4", 12, 100), 1);
    let b = VideoCacheKey::cover(source("a.mp4", 12, 101), 1);
    let c = VideoCacheKey::cover(source("a.mp4", 12, 100), 2);
    assert_ne!(a, b);
    assert_ne!(a, c);
}

#[test]
fn clear_refuses_a_root_without_the_viewer_video_cache_marker() {
    let root = tempdir().unwrap();
    assert_eq!(VideoCache::open(root.path()).unwrap_err(), CacheError::UnverifiedRoot);
}
```

- [ ] **Step 2: Write failing cover-selection tests**

```rust
#[test]
fn first_useful_frame_skips_black_intro() {
    let frames = [sample(0.01, 0.00), sample(0.08, 0.02), sample(0.42, 0.16)];
    assert_eq!(choose_cover_sample(&frames), Some(2));
}

#[test]
fn fallback_uses_first_decodable_frame() {
    let frames = [failed_sample(), sample(0.03, 0.01), sample(0.02, 0.01)];
    assert_eq!(choose_cover_sample(&frames), Some(1));
}
```

- [ ] **Step 3: Run focused tests and verify missing cache/selector failures**

Run: `cargo test -p viewer-infrastructure --test video_cache --test video_thumbnail`

Expected: FAIL because cache and thumbnail types are absent.

- [ ] **Step 4: Implement the verified cache root and LRU budget**

```rust
pub const VIDEO_CACHE_BUDGET_BYTES: u64 = 1_073_741_824;

pub struct VideoCacheStats {
    pub bytes_used: u64,
    pub entry_count: u64,
    pub budget_bytes: u64,
}

impl VideoCache {
    pub fn put_atomic(&self, key: &VideoCacheKey, png: &[u8]) -> Result<PathBuf, CacheError>;
    pub fn get(&self, key: &VideoCacheKey) -> Result<Option<PathBuf>, CacheError>;
    pub fn stats(&self) -> Result<VideoCacheStats, CacheError>;
    pub async fn clear(&self) -> Result<VideoCacheStats, CacheError>;
}
```

Create `.viewer-video-cache-v1` only when initializing the exact app cache child `video/`. Writes use a temp file plus atomic rename. Each hit updates a small access-stamp file. Eviction sorts stamp modification times and removes oldest verified entry directories until total bytes are at or below 1 GiB.

- [ ] **Step 5: Implement deterministic cover and timeline generation**

```rust
const COVER_SAMPLE_FRACTIONS: [f64; 6] = [0.03, 0.08, 0.15, 0.25, 0.40, 0.55];
const TIMELINE_BUCKET_US: u64 = 500_000;

pub fn quantize_timeline_time(time_us: u64, duration_us: u64) -> u64 {
    time_us.min(duration_us) / TIMELINE_BUCKET_US * TIMELINE_BUCKET_US
}
```

Sample a width-160 grayscale frame for luminance/variance scoring, accept luminance `>= 0.08` and variance `>= 0.03`, then render the selected cover as a width-640 PNG. Timeline thumbnails are width-320 PNGs. All work uses the one-permit media-tools semaphore, checks cancellation before cache publication, and waits while playback is active.

- [ ] **Step 6: Register generated PNGs through the existing image artifact protocol**

```rust
pub fn register_video_png(
    registry: &ImageArtifactRegistry,
    session_id: SessionId,
    path: &Path,
) -> Result<ArtifactId, ImageArtifactError>;
```

Keep `viewer-image` MIME/type enforcement unchanged except for accepting registered PNGs from the verified video cache root. A cache path is never returned directly to React.

- [ ] **Step 7: Run cache, thumbnail, and protocol tests**

Run: `cargo test -p viewer-infrastructure video_ && cargo test -p viewer-desktop image_protocol`

Expected: PASS, including concurrency-one and active-playback-yield tests.

- [ ] **Step 8: Commit thumbnail and cache support**

```bash
git add crates/viewer-infrastructure src-tauri
git commit -m "feat: cache video covers and timeline frames"
```

---

### Task 7: Implement the Platform-Neutral Video Session State Machine

**Files:**
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-application/src/ports.rs`
- Create: `crates/viewer-application/src/video.rs`
- Create: `crates/viewer-application/src/video_thumbnail.rs`
- Create: `crates/viewer-application/tests/video_preview_service.rs`
- Create: `crates/viewer-application/tests/video_thumbnail_service.rs`
- Modify: `crates/viewer-test-support/src/lib.rs`
- Create: `crates/viewer-test-support/src/video_engine.rs`

**Interfaces:**
- Consumes: domain identifiers/metadata and cache/thumbnail results from Tasks 4–6.
- Produces: the `VideoEngine` port, `VideoPreviewService`, generation-scoped `VideoCommand`, `VideoEvent`, and a fake adapter contract reusable by the future Windows implementation.

- [ ] **Step 1: Write failing transition and stale-generation tests**

```rust
#[tokio::test]
async fn first_frame_reveals_then_starts_playback() {
    let (service, fake) = harness();
    let generation = service.open(source()).await.unwrap();
    fake.emit(generation, EngineEvent::Prepared(media())).await;
    fake.emit(generation, EngineEvent::FirstFrameReady).await;
    assert_eq!(fake.calls(), vec![Call::OpenPaused, Call::RevealSurface, Call::Play]);
    assert_eq!(service.snapshot().state, VideoPlaybackState::Playing);
}

#[tokio::test]
async fn stale_first_frame_cannot_reveal_after_navigation() {
    let (service, fake) = harness();
    let old = service.open(source()).await.unwrap();
    let new = service.open(next_source()).await.unwrap();
    fake.emit(old, EngineEvent::FirstFrameReady).await;
    assert_eq!(service.snapshot().generation, new);
    assert!(!fake.was_revealed(old));
}
```

- [ ] **Step 2: Run service tests and verify missing state machine**

Run: `cargo test -p viewer-application --test video_preview_service`

Expected: FAIL because video services do not exist.

- [ ] **Step 3: Define the exact engine port**

```rust
#[async_trait]
pub trait VideoEngine: Send + Sync {
    async fn open_paused(&self, request: EngineOpenRequest) -> Result<(), VideoEngineError>;
    async fn reveal_surface(&self, generation: u64) -> Result<(), VideoEngineError>;
    async fn close(&self, generation: u64) -> Result<(), VideoEngineError>;
    async fn play(&self, generation: u64) -> Result<(), VideoEngineError>;
    async fn pause(&self, generation: u64) -> Result<(), VideoEngineError>;
    async fn seek(&self, generation: u64, time_us: u64) -> Result<(), VideoEngineError>;
    async fn step(&self, generation: u64, direction: FrameDirection) -> Result<(), VideoEngineError>;
    async fn set_volume(&self, generation: u64, percent: u8) -> Result<(), VideoEngineError>;
    async fn set_muted(&self, generation: u64, muted: bool) -> Result<(), VideoEngineError>;
    async fn set_rate(&self, generation: u64, rate: PlaybackRate) -> Result<(), VideoEngineError>;
    async fn set_surface_rect(&self, generation: u64, rect: SurfaceRect) -> Result<(), VideoEngineError>;
}
```

Only `PlaybackRate::Half`, `ThreeQuarters`, `Normal`, `OneAndQuarter`, `OneAndHalf`, and `Double` are constructible.

- [ ] **Step 4: Implement the serialized state machine**

```rust
pub enum VideoPlaybackState {
    Idle, Preparing, Ready, Playing, Paused, Seeking, FrameStepping,
    Ended, Failed(VideoFailureKind), Closing,
}

pub struct VideoPreviewSnapshot {
    pub generation: u64,
    pub session_id: Option<VideoSessionId>,
    pub state: VideoPlaybackState,
    pub time_us: u64,
    pub duration_us: Option<u64>,
    pub volume_percent: u8,
    pub muted: bool,
    pub rate: PlaybackRate,
}
```

Every open first closes the active generation, increments generation, resets time/rate/volume/mute, and opens paused. `FirstFrameReady` causes reveal then play exactly once. Ended remains paused on the final frame. `close` is idempotent and leaves Idle even after duplicate or late engine events.

- [ ] **Step 5: Implement frame-step and navigation policy tests**

```rust
#[tokio::test]
async fn frame_step_while_playing_pauses_before_step() {
    let (service, fake) = playing_harness().await;
    service.step(FrameDirection::Forward).await.unwrap();
    assert!(fake.calls().ends_with(&[Call::Pause, Call::Step(FrameDirection::Forward)]));
}

#[test]
fn navigation_is_video_only() {
    assert_eq!(video_neighbors(files_with_images_text_and_videos(), video_id(2)),
               (Some(video_id(1)), Some(video_id(3))));
}
```

- [ ] **Step 6: Implement thumbnail request generations**

```rust
pub struct TimelineThumbnailRequest {
    pub session_id: VideoSessionId,
    pub generation: u64,
    pub request_id: VideoThumbnailRequestId,
    pub bucket_us: u64,
}
```

Keep only the newest pending hover request per active generation. A result publishes only when session, generation, request ID, and bucket all match.

- [ ] **Step 7: Run application and fake-adapter tests**

Run: `cargo test -p viewer-application video_ && cargo test -p viewer-test-support`

Expected: PASS.

- [ ] **Step 8: Commit the neutral service contract**

```bash
git add crates/viewer-application crates/viewer-test-support
git commit -m "feat: add generation-safe video services"
```

---

### Task 8: Connect macOS libmpv Through a Typed Tauri Video Runtime

**Files:**
- Modify: `crates/viewer-platform-macos/src/video/mod.rs`
- Create: `crates/viewer-platform-macos/src/video/adapter.rs`
- Modify: `crates/viewer-platform-macos/src/video/surface.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/video.rs`
- Modify: `src-tauri/src/dto/mod.rs`
- Create: `src-tauri/src/dto/video.rs`
- Create: `src-tauri/src/video_events.rs`
- Create: `src-tauri/src/video_runtime.rs`
- Modify: `src-tauri/src/state/mod.rs`
- Modify: `src-tauri/src/state/preview.rs`
- Modify: `src-tauri/src/commands/project.rs`
- Create: `tests/video_security_boundaries.rs`
- Create: `tests/video_runtime_lifecycle.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: `VideoEngine` and service contracts from Task 7, safe mpv wrapper from Task 2, proven surface from Task 3, active project/session entity resolution.
- Produces: managed `VideoRuntime`, explicit Tauri commands, tagged `viewer://video-event` events, path authorization, and close hooks for preview/project/window/app lifecycle.

- [ ] **Step 1: Write failing bridge security and lifecycle tests**

```rust
#[tokio::test]
async fn open_rejects_non_video_stale_and_outside_project_entities() {
    let runtime = bridge_harness();
    assert_eq!(runtime.open(image_entity()).await.unwrap_err().code(), "not_video");
    assert_eq!(runtime.open(stale_video_entity()).await.unwrap_err().code(), "stale_session");
    assert_eq!(runtime.open(escaped_video_entity()).await.unwrap_err().code(), "path_not_authorized");
}

#[tokio::test]
async fn project_close_closes_video_before_database_and_artifacts() {
    let harness = lifecycle_harness().await;
    harness.close_project().await.unwrap();
    assert_eq!(harness.order(), ["video-close", "artifact-revoke", "session-close"]);
}
```

- [ ] **Step 2: Run focused desktop tests and observe missing runtime**

Run: `cargo test -p viewer-desktop --test video_security_boundaries --test video_runtime_lifecycle`

Expected: FAIL because video bridge/runtime modules are absent.

- [ ] **Step 3: Add canonical entity resolution to `DesktopRuntime`**

```rust
pub struct AuthorizedVideoSource {
    pub entity_id: EntityId,
    pub session_id: SessionId,
    pub canonical_path: PathBuf,
    pub metadata: VideoMetadata,
}

pub async fn resolve_video_entity(
    &self,
    entity_id: EntityId,
) -> Result<AuthorizedVideoSource, RuntimeError>;
```

Resolution requires the active project/session, indexed kind Video, a regular local file, and a canonical path contained by the canonical project root. It never accepts a frontend path.

- [ ] **Step 4: Define the typed command surface**

```rust
#[tauri::command] async fn video_open(request: VideoOpenRequestDto, window: WebviewWindow,
    desktop: State<'_, DesktopRuntime>, video: State<'_, VideoRuntime>) -> CommandResult<VideoSessionDto>;
#[tauri::command] async fn video_close(request: GenerationDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_play(request: GenerationDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_pause(request: GenerationDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_seek(request: VideoSeekDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_step(request: VideoStepDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_set_volume(request: VideoVolumeDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_set_muted(request: VideoMutedDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_set_rate(request: VideoRateDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_set_surface_rect(request: VideoSurfaceRectDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_set_fullscreen(request: VideoFullscreenDto, window: WebviewWindow,
    video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_request_thumbnail(request: VideoThumbnailRequestDto, video: State<'_, VideoRuntime>) -> CommandResult<()>;
#[tauri::command] async fn video_cache_stats(video: State<'_, VideoRuntime>) -> CommandResult<VideoCacheStatsDto>;
#[tauri::command] async fn video_cache_clear(video: State<'_, VideoRuntime>) -> CommandResult<VideoCacheStatsDto>;
```

Fullscreen remains a Viewer/window action invoked by `video_set_fullscreen { generation, fullscreen }`; no engine command is exposed. The runtime rejects a generation mismatch before mutating the window.

- [ ] **Step 5: Add the tagged event contract and 10 Hz coalescer**

```rust
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VideoEventDto {
    Prepared { generation: u64, media: VideoMediaDto },
    FirstFrameReady { generation: u64 },
    StateChanged { generation: u64, state: VideoStateDto },
    Progress { generation: u64, time_us: u64, duration_us: Option<u64> },
    SettingsChanged { generation: u64, volume_percent: u8, muted: bool, rate: f32 },
    FullscreenChanged { generation: u64, fullscreen: bool },
    TimelineThumbnailReady { generation: u64, request_id: String, bucket_us: u64, artifact_url: String },
    Ended { generation: u64 },
    Failed { generation: u64, error: VideoErrorDto },
    Closed { generation: u64 },
}
```

Only `Progress` is coalesced; emit at most one every 100 ms and always flush the latest value before Ended/Failed/Closed. `FullscreenChanged` is emitted immediately after the window confirms the transition.

- [ ] **Step 6: Implement `MacOsLibmpvAdapter` and lifecycle ordering**

```rust
pub struct MacOsLibmpvAdapter {
    client: Mutex<Option<MpvClient>>,
    render: Mutex<Option<MpvRenderContext>>,
    surface: Mutex<Option<MacVideoSurface>>,
    diagnostics: Arc<VideoDiagnosticsCounters>,
}
```

Close order is: hide surface, stop playback/audio, stop render callbacks, destroy render context, destroy client, unmount surface, revoke active thumbnail artifacts. Invoke the same idempotent close from preview close, project close, window close request, and app exit.

- [ ] **Step 7: Run bridge security, lifecycle, and regression tests**

Run: `cargo test -p viewer-desktop --test video_security_boundaries --test video_runtime_lifecycle && cargo test -p viewer-desktop --tests`

Expected: PASS.

- [ ] **Step 8: Commit the typed native bridge**

```bash
git add crates/viewer-platform-macos src-tauri tests/video_security_boundaries.rs tests/video_runtime_lifecycle.rs
git commit -m "feat: bridge native video playback"
```

---

### Task 9: Add Video API Types, Search Filtering, and the Browse Section

**Files:**
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/fileKinds.ts`
- Modify: `ui/src/components/SearchToolbar.tsx`
- Modify: `ui/src/components/SearchToolbar.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.ts`
- Modify: `ui/src/components/contentBrowser/adaptiveOtherFilePanelModel.test.ts`
- Create: `ui/src/app/useVideoPanelPreference.ts`
- Create: `ui/src/components/contentBrowser/VideoSection.tsx`
- Create: `ui/src/components/contentBrowser/VideoSection.test.tsx`
- Create: `ui/src/components/contentBrowser/VideoCard.tsx`
- Create: `ui/src/components/contentBrowser/VideoCard.test.tsx`
- Create: `ui/src/styles/videoBrowser.css`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: video-aware workspace DTOs and Tauri command/event contracts from Tasks 4 and 8.
- Produces: TypeScript `VideoMetadata`, `VideoEvent`, bridge methods, Video file-kind filter, `VideoSection`, `VideoCard`, default expansion, and video-aware selection scopes.

- [ ] **Step 1: Write failing browse behavior tests**

```tsx
it("shows videos expanded between images and other files", () => {
  render(<ContentBrowser workspace={workspace({ videos: [video("clip.mp4")], otherFiles: [text("a.txt")] })} />)
  expect(screen.getByRole("button", { name: "收起视频" })).toHaveAttribute("aria-expanded", "true")
  expect(screen.getByText("视频 · 1").compareDocumentPosition(screen.getByText("其它文件 · 1"))
    & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
})

it("omits the video disclosure when no videos exist", () => {
  render(<ContentBrowser workspace={workspace({ videos: [] })} />)
  expect(screen.queryByText(/视频 ·/)).not.toBeInTheDocument()
})
```

- [ ] **Step 2: Run focused UI tests and verify missing Video types/components**

Run: `pnpm ui:test -- ContentBrowser VideoSection VideoCard SearchToolbar viewer`

Expected: FAIL because TypeScript and components do not represent Video.

- [ ] **Step 3: Add exact frontend bridge types**

```ts
export type FileKind = "directory" | "jpeg" | "png" | "markdown" | "text" | "unsupportedImage" | "other" | "video"

export interface VideoMetadata {
  durationUs: number | null
  displayWidth: number | null
  displayHeight: number | null
  rotationDegrees: number
  frameRateMillihertz: number | null
  videoCodec: string | null
  audioCodec: string | null
  probeStatus: "pending" | "ready" | "failed"
  failureKind: VideoFailureKind | null
  coverUrl: string | null
}
```

Add explicit `ViewerBridge.videoOpen`, `videoClose`, `videoPlay`, `videoPause`, `videoSeek`, `videoStep`, `videoSetVolume`, `videoSetMuted`, `videoSetRate`, `videoSetSurfaceRect`, `videoSetFullscreen`, `videoRequestThumbnail`, `videoCacheStats`, and `videoCacheClear`; each invokes only the matching Rust command.

- [ ] **Step 4: Implement the dedicated section and stable cards**

```tsx
export function VideoSection({ videos, expanded, onExpandedChange, selection, onOpen }: VideoSectionProps) {
  if (videos.length === 0) return null
  return <section aria-labelledby="video-section-title">
    <button type="button" aria-expanded={expanded} onClick={() => onExpandedChange(!expanded)}>
      <span id="video-section-title">视频 · {videos.length}</span>
    </button>
    {expanded && <div role="list">{videos.map(video =>
      <VideoCard key={video.entityId} video={video} selected={selection.has(video.entityId)}
        onDoubleClick={() => onOpen(video.entityId)} />
    )}</div>}
  </section>
}
```

Use an aspect-ratio `16 / 9` cover stage with reserved geometry, a centered play icon, duration formatted from microseconds, filename, focus/selection state, and an unavailable badge for failed probe status. Fade a loaded cover inside the reserved stage; loading never changes card height.

- [ ] **Step 5: Add video selection and filter behavior**

`ContentBrowser` partitions `images`, `videos`, and `otherFiles`. Select-all scopes are `images`, `videos`, `otherFiles`, and `all`; file operations receive video entity IDs through the existing selection safety path. Add a `视频` choice to `SearchToolbar` that emits `FileKind.Video`.

- [ ] **Step 6: Preserve presentation-only expansion state**

```ts
export function useVideoPanelPreference(projectSessionKey: string | null) {
  const [collapsedBySession, setCollapsedBySession] = useState<Record<string, boolean>>({})
  const collapsed = projectSessionKey ? collapsedBySession[projectSessionKey] ?? false : false
  return { expanded: !collapsed, setExpanded: (expanded: boolean) => setForSession(!expanded) }
}
```

Changing disclosure state must not issue probe/cover commands or clear selection.

- [ ] **Step 7: Run browse, selection, API, and style tests**

Run: `pnpm ui:test -- ContentBrowser VideoSection VideoCard SearchToolbar adaptiveOtherFilePanelModel viewer && pnpm ui:check`

Expected: PASS.

- [ ] **Step 8: Commit video browsing**

```bash
git add ui/src
git commit -m "feat: browse videos in a dedicated section"
```

---

### Task 10: Add the First-Frame-Gated Video Preview Shell

**Files:**
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/app/usePreviewSession.ts`
- Modify: `ui/src/state/previewPolicy.ts`
- Modify: `ui/src/state/previewPolicy.test.ts`
- Create: `ui/src/components/VideoPreview.tsx`
- Create: `ui/src/components/VideoPreview.test.tsx`
- Create: `ui/src/components/videoPreview/videoState.ts`
- Create: `ui/src/components/videoPreview/videoState.test.ts`
- Create: `ui/src/components/videoPreview/videoGeometry.ts`
- Create: `ui/src/components/videoPreview/videoGeometry.test.ts`
- Create: `ui/src/components/videoPreview/useVideoBridge.ts`
- Create: `ui/src/components/videoPreview/useVideoBridge.test.tsx`
- Create: `ui/src/styles/videoPreview.css`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: video bridge/types and cards from Task 9.
- Produces: dedicated video preview routing, video-only navigation list, generation reducer, fitted native surface geometry, and one-time first-frame reveal.

- [ ] **Step 1: Write failing first-reveal and navigation tests**

```tsx
it("keeps the native surface hidden until the current generation first frame", async () => {
  const bridge = videoBridgeHarness()
  render(<VideoPreview file={video("a.mp4")} files={[video("a.mp4")]} bridge={bridge} />)
  expect(screen.getByRole("status", { name: "正在加载视频" })).toBeVisible()
  bridge.emit({ type: "firstFrameReady", generation: 1 })
  await waitFor(() => expect(screen.queryByRole("status", { name: "正在加载视频" })).not.toBeInTheDocument())
  expect(bridge.calls("videoSetSurfaceRect")).toHaveLength(1)
})

it("ignores a late first frame from the previous video", () => {
  const state = reduceVideoState(opened(2), { type: "firstFrameReady", generation: 1 })
  expect(state.surfaceVisible).toBe(false)
})
```

- [ ] **Step 2: Run focused preview tests and observe missing implementation**

Run: `pnpm ui:test -- VideoPreview videoState videoGeometry previewPolicy App`

Expected: FAIL because video preview routing and state do not exist.

- [ ] **Step 3: Add generation-keyed reducer and bridge lifecycle**

```ts
export interface VideoPreviewState {
  generation: number
  phase: "preparing" | "playing" | "paused" | "seeking" | "ended" | "failed" | "closing"
  prepared: boolean
  firstFrameReady: boolean
  surfaceVisible: boolean
  timeUs: number
  durationUs: number | null
  error: VideoError | null
}
```

Subscribe before invoking `videoOpen`. Filter every event by generation. On file change, preview Done, project close, or unmount, invoke `videoClose` once for the current generation and detach the event listener.

- [ ] **Step 4: Implement rotation-aware fit geometry**

```ts
export function fitVideoRect(stage: DOMRectReadOnly, media: VideoMedia): SurfaceRectDto {
  const rotated = Math.abs(media.rotationDegrees % 180) === 90
  const sourceWidth = rotated ? media.displayHeight : media.displayWidth
  const sourceHeight = rotated ? media.displayWidth : media.displayHeight
  const scale = Math.min(stage.width / sourceWidth, stage.height / sourceHeight)
  return centeredRect(stage, sourceWidth * scale, sourceHeight * scale)
}
```

Send geometry only after both media dimensions and measured stage size exist. ResizeObserver changes update geometry without reopening media.

- [ ] **Step 5: Implement the neutral loading gate and error shell**

The preview shell appears immediately. A neutral indeterminate bar labeled `正在加载视频` covers the native slot until `firstFrameReady` for the active generation. It has no percentage and no minimum duration. The cover URL is never rendered in the preview. Failure keeps shell/navigation visible and exposes `重试` and `完成`.

- [ ] **Step 6: Route only videos through `VideoPreview`**

`App.tsx` chooses `VideoPreview` when `activePreview.kind === "video"`; its neighbor list is `workspace.videos` or video-filtered search results. Image, text, and unsupported preview routes remain unchanged.

- [ ] **Step 7: Run preview tests, type check, and build**

Run: `pnpm ui:test -- VideoPreview videoState videoGeometry previewPolicy App && pnpm ui:check && pnpm ui:build`

Expected: PASS with no preview cover element and one reveal per generation.

- [ ] **Step 8: Commit the video preview shell**

```bash
git add ui/src
git commit -m "feat: gate video preview on first frame"
```

---

### Task 11: Add Playback Controls, Timeline Frames, Shortcuts, and Fullscreen

**Files:**
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/VideoPreview.test.tsx`
- Create: `ui/src/components/videoPreview/VideoControls.tsx`
- Create: `ui/src/components/videoPreview/VideoControls.test.tsx`
- Create: `ui/src/components/videoPreview/VideoTimeline.tsx`
- Create: `ui/src/components/videoPreview/VideoTimeline.test.tsx`
- Create: `ui/src/components/videoPreview/useVideoControlsVisibility.ts`
- Create: `ui/src/components/videoPreview/useVideoControlsVisibility.test.tsx`
- Create: `ui/src/components/videoPreview/useVideoShortcuts.ts`
- Create: `ui/src/components/videoPreview/useVideoShortcuts.test.tsx`
- Modify: `ui/src/components/videoPreview/useVideoBridge.ts`
- Modify: `ui/src/styles/videoPreview.css`
- Add: approved Lucide SVGs and license coverage under `ui/src/assets/icons/lucide/`

**Interfaces:**
- Consumes: active generation/state/commands from Task 10 and timeline artifacts from Tasks 6/8.
- Produces: approved control overlay, 2.5-second visibility hook, accessible timeline thumbnails, shortcut handling, fullscreen state, and stop-on-final-frame behavior.

- [ ] **Step 1: Write failing shortcut and auto-hide tests**

```tsx
it("pauses before stepping when ArrowRight is pressed while playing", async () => {
  const bridge = playingBridge()
  render(<VideoPreviewHarness bridge={bridge} />)
  fireEvent.keyDown(window, { key: "ArrowRight" })
  expect(bridge.orderedCalls()).toEqual(["videoPause", "videoStep:forward"])
})

it("does not hide controls while paused or focused", () => {
  vi.useFakeTimers()
  const { result, rerender } = renderHook(props => useVideoControlsVisibility(props),
    { initialProps: { playing: true, focused: false, reducedMotion: false } })
  act(() => vi.advanceTimersByTime(2500))
  expect(result.current.visible).toBe(false)
  rerender({ playing: false, focused: true, reducedMotion: false })
  expect(result.current.visible).toBe(true)
})
```

- [ ] **Step 2: Write failing timeline stale/clamping tests**

```tsx
it("clamps the thumbnail and ignores a stale request", () => {
  render(<VideoTimelineHarness pointerFraction={0.99} />)
  expect(screen.getByTestId("timeline-thumbnail")).toHaveStyle({ right: "0px" })
  emitThumbnail({ generation: 1, requestId: "old", bucketUs: 500_000 })
  expect(screen.queryByAltText("00:00.5 预览")).not.toBeInTheDocument()
})
```

- [ ] **Step 3: Run control tests and observe missing components**

Run: `pnpm ui:test -- VideoControls VideoTimeline useVideoControlsVisibility useVideoShortcuts VideoPreview`

Expected: FAIL because controls and hooks are absent.

- [ ] **Step 4: Implement approved control commands and rate values**

```ts
export const VIDEO_RATES = [0.5, 0.75, 1, 1.25, 1.5, 2] as const
export type VideoRate = (typeof VIDEO_RATES)[number]
```

Play/pause, previous frame, next frame, timeline, elapsed/total time, mute, volume `0..100`, rate, and fullscreen each invoke one typed bridge method for the active generation. While playing, frame controls await pause before step. Ended renders paused state on the final time and never opens the next video.

- [ ] **Step 5: Implement timeline hover, drag, and thumbnail requests**

Pointer time clamps to `[0, duration]`; requests debounce for 80 ms and use the 500 ms bucket from Task 6. Drag updates the displayed time immediately, sends coalesced seeks, and keeps controls visible. A pending thumbnail uses a fixed-size neutral preview tile. Ready artifacts render only when generation, request ID, and bucket match the current pointer target.

- [ ] **Step 6: Implement control visibility and reduced motion**

```ts
const CONTROL_IDLE_MS = 2500
const mustRemainVisible = !playing || seeking || adjusting || focusedWithin || error !== null
```

Pointer movement and owned keyboard input reveal controls and restart the timer only when they may hide. Reduced motion removes opacity/transform transitions and loading sweep while preserving immediate show/hide state.

- [ ] **Step 7: Implement shortcut ownership and Escape ordering**

Ignore shortcuts when `event.target` is `input`, `textarea`, `select`, contenteditable, or inside an unrelated modal. Space prevents scroll and toggles playback; Left/Right pause then step; M toggles mute; F toggles fullscreen; Escape exits fullscreen first and closes preview only on a subsequent press when not fullscreen.

- [ ] **Step 8: Add accessible control and timeline semantics**

Every button has a Chinese accessible name and shortcut in `aria-keyshortcuts`/help text. Timeline is `role="slider"` with `aria-valuemin="0"`, microsecond-derived `aria-valuemax`, current seconds, and formatted `aria-valuetext`. Do not announce 10 Hz progress events through a live region.

- [ ] **Step 9: Run control, accessibility, and full UI tests**

Run: `pnpm ui:test -- VideoControls VideoTimeline useVideoControlsVisibility useVideoShortcuts VideoPreview visualAccessibility && pnpm ui:check`

Expected: PASS.

- [ ] **Step 10: Commit controls and timeline inspection**

```bash
git add ui/src
git commit -m "feat: add video playback controls"
```

---

### Task 12: Add Cache Settings, Error States, and Acceptance Scenes

**Files:**
- Modify: `ui/src/components/SettingsDialog.tsx`
- Modify: `ui/src/components/SettingsDialog.test.tsx`
- Modify: `ui/src/components/VideoPreview.tsx`
- Modify: `ui/src/components/VideoPreview.test.tsx`
- Modify: `ui/src/components/contentBrowser/VideoCard.tsx`
- Modify: `ui/src/components/contentBrowser/VideoCard.test.tsx`
- Create: `ui/src/acceptance/scenes/videoScenes.tsx`
- Create: `ui/src/acceptance/scenes/videoScenes.test.tsx`
- Modify: `ui/src/acceptance/scenes/index.ts`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.test.ts`
- Modify: `ui/src/acceptance/acceptanceFixtures.ts`
- Modify: `docs/reviews/viewer-native-smoke-matrix.md`

**Interfaces:**
- Consumes: cache commands, normalized errors, browse/preview components.
- Produces: Video Cache settings row, clear confirmation/result handling, contained retryable errors, reduced-motion/accessibility scenes, and acceptance catalog coverage.

- [ ] **Step 1: Write failing cache settings tests**

```tsx
it("reports usage and clears only after confirmation", async () => {
  const bridge = bridgeWithVideoCache({ bytesUsed: 268_435_456, budgetBytes: 1_073_741_824, entryCount: 24 })
  render(<SettingsDialog bridge={bridge} />)
  expect(await screen.findByText("256 MiB / 1 GiB")).toBeVisible()
  await userEvent.click(screen.getByRole("button", { name: "清除视频缓存" }))
  await userEvent.click(screen.getByRole("button", { name: "确认清除视频缓存" }))
  expect(bridge.videoCacheClear).toHaveBeenCalledTimes(1)
})
```

- [ ] **Step 2: Write failing error presentation tests**

```tsx
it.each([
  ["unsupported", "不支持此视频的容器或编码"],
  ["damaged", "视频已损坏或无法读取"],
  ["missing", "视频文件已移动或删除"],
  ["engineInitialization", "视频引擎无法启动"],
  ["decodeFallbackFailed", "硬件与软件解码均失败"],
  ["renderSurface", "视频显示区域无法创建"],
])("maps %s to a concise retryable state", (kind, message) => {
  render(<FailedVideoPreview kind={kind} />)
  expect(screen.getByText(message)).toBeVisible()
  expect(screen.getByRole("button", { name: "重试" })).toBeEnabled()
  expect(screen.getByRole("button", { name: "完成" })).toBeEnabled()
})
```

- [ ] **Step 3: Run settings/error tests and observe failures**

Run: `pnpm ui:test -- SettingsDialog VideoPreview VideoCard videoScenes acceptanceStateCatalog`

Expected: FAIL because cache UI and video acceptance states are missing.

- [ ] **Step 4: Add the non-persisted Video Cache settings row**

Fetch stats only while Settings is open. Display current usage, fixed 1 GiB limit, entry count, and `清除视频缓存`. Use the existing confirmation surface. Do not change settings schema version because cache usage and the fixed budget are runtime state, not user preferences.

- [ ] **Step 5: Add normalized error and unavailable-card behavior**

Playback errors retain shell, navigation, Retry, Done, and Escape. Retry closes the failed generation before reopening the same entity. Thumbnail failure shows a fixed-size neutral card or hover tile but does not mark a playable video unavailable. User-facing errors omit absolute paths; diagnostic copy may include engine category and redacted filename.

- [ ] **Step 6: Add acceptance scenes and catalog entries**

Register exact scene IDs:

```json
[
  "workspace-video-expanded",
  "workspace-video-unavailable",
  "video-preparing",
  "video-playing-controls",
  "video-paused-controls",
  "video-timeline-pending",
  "video-timeline-ready",
  "video-ended",
  "video-failed-retry",
  "video-fullscreen-controls",
  "settings-video-cache",
  "video-reduced-motion"
]
```

Acceptance fixtures use static image artifacts for visual review and a fake bridge; they do not start libmpv.

- [ ] **Step 7: Run acceptance, accessibility, and UI suites**

Run: `pnpm ui:test -- SettingsDialog VideoPreview VideoCard videoScenes acceptanceStateCatalog visualAccessibility && pnpm ui:check && pnpm ui:build`

Expected: PASS.

- [ ] **Step 8: Commit settings and acceptance coverage**

```bash
git add ui/src docs/reviews/viewer-native-smoke-matrix.md
git commit -m "feat: complete video states and cache settings"
```

---

### Task 13: Package, Sign, Attribute, and Audit the Runtime

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/build.rs`
- Modify: `package.json`
- Modify: `.github/workflows/ci.yml`
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: `ACKNOWLEDGEMENTS.md`
- Modify: `docs/quality/DEPENDENCY_HEALTH.md`
- Modify: `docs/quality/dependency-exceptions.json`
- Create: `scripts/video/stage-tauri-resources.sh`
- Create: `scripts/video/verify-app-bundle.sh`
- Create: `scripts/video/verify-licenses.sh`
- Create: `scripts/video/source-offer.txt`
- Test: `crates/viewer-video-mpv/tests/release_layout.rs`

**Interfaces:**
- Consumes: verified runtime inventory from Task 1 and completed Tauri app from Tasks 8–12.
- Produces: bundle-relative runtime resources, loader-path/signature/license verification, dependency-health ownership, and repeatable CI/release commands.

- [ ] **Step 1: Write a failing release-layout test**

```rust
#[test]
fn release_layout_never_uses_host_fallbacks() {
    let layout = RuntimeLayout::from_bundle_root(Path::new("/Viewer.app/Contents/Resources"));
    assert_eq!(layout.libmpv, PathBuf::from("/Viewer.app/Contents/Resources/ViewerVideoRuntime/lib/libmpv.2.dylib"));
    assert_eq!(layout.ffmpeg, PathBuf::from("/Viewer.app/Contents/Resources/ViewerVideoRuntime/bin/ffmpeg"));
    assert_eq!(layout.ffprobe, PathBuf::from("/Viewer.app/Contents/Resources/ViewerVideoRuntime/bin/ffprobe"));
}
```

- [ ] **Step 2: Run the layout test and bundle verifier before resource staging**

Run: `cargo test -p viewer-video-mpv --test release_layout && bash scripts/video/verify-app-bundle.sh target/release/bundle/macos/Viewer.app`

Expected: layout test PASS and bundle verification FAIL because resources are not staged.

- [ ] **Step 3: Stage exact Tauri resources**

```json
{
  "bundle": {
    "resources": [
      "../target/viewer-video-runtime/universal-apple-darwin/ViewerVideoRuntime"
    ]
  }
}
```

`stage-tauri-resources.sh` verifies the runtime first, copies with preserved executable bits, and fails if the target contains an unexpected file. `build.rs` embeds the runtime manifest schema version used by `RuntimeLayout`.

- [ ] **Step 4: Add nested signing and loader verification**

```bash
find "$APP/Contents/Resources/ViewerVideoRuntime" -type f \( -name '*.dylib' -o -perm -111 \) -print0 \
  | xargs -0 -n1 codesign --verify --strict --verbose=2
codesign --verify --deep --strict --verbose=2 "$APP"
spctl --assess --type execute --verbose=2 "$APP"
```

The verifier also checks every `otool -L` path is `@loader_path`, `@rpath` resolved inside the bundle, or an allowlisted Apple system library; confirms architectures with `lipo -archs`; compares file hashes to the staged inventory; and runs ffprobe offline with network disabled.

- [ ] **Step 5: Complete notices and dependency health**

List libmpv/mpv, FFmpeg, libplacebo, libass, and every linked non-Apple dependency with exact version, license, source URL, archive digest, enabled build options, security owner, review date, and upgrade procedure. Include LGPL 2.1-or-later text and `scripts/video/source-offer.txt` pointing to exact reproducible source acquisition commands.

- [ ] **Step 6: Add CI gates**

Add `pnpm video:runtime:verify`, `pnpm video:licenses:verify`, and a macOS runtime-cache/build job. Normal PR tests validate the lock, API, and notices without downloading/building the full runtime; the scheduled/release job builds both `aarch64-apple-darwin` and `x86_64-apple-darwin`, stages the release shape, and runs bundle verification.

- [ ] **Step 7: Build and verify a local app bundle**

Run: `pnpm tauri build && pnpm video:runtime:verify && pnpm video:licenses:verify && scripts/video/verify-app-bundle.sh target/release/bundle/macos/Viewer.app`

Expected: PASS with no host-path dependency, missing signature, license omission, or inventory mismatch.

- [ ] **Step 8: Commit release packaging**

```bash
git add src-tauri package.json .github/workflows/ci.yml scripts/video THIRD_PARTY_NOTICES.md ACKNOWLEDGEMENTS.md docs/quality crates/viewer-video-mpv/tests/release_layout.rs
git commit -m "build: package and audit video runtime"
```

---

### Task 14: Run Media, Performance, Lifecycle, and Clean-Machine Acceptance

**Files:**
- Create: `scripts/video/generate-test-fixtures.sh`
- Modify: `tests/fixtures/videos/manifest.json`
- Add: redistribution-safe generated files under `tests/fixtures/videos/`
- Create: `tests/video_fixture_matrix.rs`
- Create: `tests/video_performance_gate.rs`
- Create: `tests/video_resource_lifecycle.rs`
- Create: `scripts/video/run-clean-machine-smoke.sh`
- Create: `docs/reviews/2026-08-10-video-preview-acceptance.md`
- Modify: `docs/reviews/viewer-native-smoke-matrix.md`
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`
- Modify: `docs/README.md`

**Interfaces:**
- Consumes: the complete bundled video feature and signed app bundle.
- Produces: reproducible media fixtures, hardware-path/performance evidence, 30-cycle leak evidence, offline clean-machine evidence, complete project documentation, and a final PASS/FAIL acceptance decision.

- [ ] **Step 1: Add the exact fixture manifest and generator**

```json
[
  { "id": "h264-aac", "container": "mp4", "videoCodec": "h264", "audioCodec": "aac" },
  { "id": "hevc-portrait", "container": "mov", "videoCodec": "hevc", "rotation": 90 },
  { "id": "prores", "container": "mov", "videoCodec": "prores" },
  { "id": "vp9-opus", "container": "webm", "videoCodec": "vp9", "audioCodec": "opus" },
  { "id": "av1", "container": "mkv", "videoCodec": "av1" },
  { "id": "vfr", "container": "mp4", "variableFrameRate": true },
  { "id": "silent", "container": "mp4", "audioCodec": null },
  { "id": "truncated", "container": "mp4", "expectedFailure": "damaged" },
  { "id": "unsupported-codec", "container": "mkv", "expectedFailure": "unsupported" }
]
```

Generate synthetic color bars, motion, and tones with the bundled ffmpeg; truncate only a generated fixture. Record SHA-256 and redistribution status in the manifest.

- [ ] **Step 2: Write the fixture matrix test**

```rust
#[test]
#[ignore = "requires bundled video runtime"]
fn fixture_matrix_matches_manifest_expectations() {
    for fixture in fixtures() {
        assert_eq!(probe_and_open(&fixture).normalized_outcome(), fixture.expected_outcome);
    }
}
```

- [ ] **Step 3: Write the lifecycle and performance gates**

```rust
#[test]
#[ignore = "requires macOS native video acceptance"]
fn thirty_open_close_navigation_cycles_return_to_baseline() {
    let before = diagnostics_snapshot();
    run_open_close_navigation_cycles(30);
    wait_for_idle(Duration::from_secs(3));
    assert_eq!(diagnostics_snapshot().owned_resources(), before.owned_resources());
}
```

The performance gate records machine, macOS, codec, resolution, bit depth, `hwdec`, `vo`, average command latency, post-warmup dropped frames, and thumbnail concurrency. It requires command-to-engine response below 100 ms for local playback and dropped frames below 1% for documented 1080p60 and 4K30 reference samples.

- [ ] **Step 4: Run the media and lifecycle gates**

Run:

```bash
VIEWER_VIDEO_RUNTIME_DIR="$(pwd)/target/viewer-video-runtime/universal-apple-darwin" \
cargo test -p viewer-desktop --test video_fixture_matrix -- --ignored --nocapture
VIEWER_VIDEO_RUNTIME_DIR="$(pwd)/target/viewer-video-runtime/universal-apple-darwin" \
cargo test -p viewer-desktop --test video_resource_lifecycle -- --ignored --nocapture
VIEWER_VIDEO_RUNTIME_DIR="$(pwd)/target/viewer-video-runtime/universal-apple-darwin" \
cargo test -p viewer-desktop --test video_performance_gate -- --ignored --nocapture
```

Expected: PASS; diagnostics show one playback decoder, thumbnail concurrency one, VideoToolbox for supported H.264/HEVC samples, and baseline resource counts after 30 cycles.

- [ ] **Step 5: Run the offline clean-machine smoke**

Run: `scripts/video/run-clean-machine-smoke.sh target/release/bundle/macos/Viewer.app`

Expected: PASS in a network-disabled clean macOS 13-or-newer account/VM with no mpv, VLC, Homebrew, or downloaded codecs. The script verifies launch, browse covers, H.264 playback, HEVC playback, frame steps, timeline preview, close-to-idle, and app signature.

- [ ] **Step 6: Run the complete repository verification**

Run: `pnpm verify`

Expected: PASS with format, type, UI, Rust, security, dependency, architecture, acceptance catalog, and build checks green.

- [ ] **Step 7: Record final acceptance evidence**

`docs/reviews/2026-08-10-video-preview-acceptance.md` must map all 12 acceptance criteria from the approved design to an automated command or named manual evidence, include fixture hashes and performance tables, and end with exactly one of:

```text
Decision: PASS — Viewer bundled video preview is ready for release integration.
Decision: FAIL — Viewer bundled video preview remains blocked from release integration.
```

- [ ] **Step 8: Commit final verification and documentation**

```bash
git add scripts/video tests/fixtures/videos tests/video_fixture_matrix.rs tests/video_performance_gate.rs tests/video_resource_lifecycle.rs docs
git commit -m "test: verify bundled video preview"
```

---

## Final Execution Checklist

- [ ] Task 3 feasibility review says PASS before any Task 4 commit exists.
- [ ] `FileKind` values `0..=6` are unchanged and Video is `7` in every database/portable encoding.
- [ ] No Rust-to-TypeScript DTO contains a source filesystem path or frame buffer.
- [ ] No public wrapper accepts arbitrary mpv command/property strings.
- [ ] No release linkage contains `/opt/homebrew`, `/usr/local`, or a user directory.
- [ ] UI never renders the card cover in the full video preview.
- [ ] First-frame reveal occurs once at final fit geometry for each generation.
- [ ] Reopen resets time, rate, volume, and mute; no playback history is stored.
- [ ] Video navigation contains videos only and end state does not advance.
- [ ] Timeline/cover results verify session, generation, request, and bucket before publication.
- [ ] Cache usage is at or below 1 GiB after eviction and clear refuses unverified roots.
- [ ] Project/window/app close returns decoder, render, audio, surface, worker, and artifacts to baseline.
- [ ] `pnpm verify`, native fixture matrix, performance gate, clean-machine smoke, bundle audit, and license audit all pass.
