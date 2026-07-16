# Viewer G1 Image Pipeline Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove that Viewer can generate grid thumbnails and color-correct high-resolution previews directly from project originals while meeting cancellation, security and memory constraints.

**Architecture:** `viewer-application` defines an async `ImagePort`; `viewer-platform-macos` implements it with Quick Look Thumbnailing and Image I/O. Generated representations live in a session cache and are exposed to the WebView only through opaque, expiring registry keys.

**Tech Stack:** Rust, objc2 framework bindings, Quick Look Thumbnailing, Image I/O, Core Graphics, ColorSync, Tauri 2 custom protocol, Tokio tests.

## Global Constraints

- JPG and PNG are the only accepted image inputs.
- Original files remain in place and are never copied into Viewer storage.
- Grid thumbnails prefer Quick Look; Image I/O is the mandatory fallback.
- Fit previews use Image I/O directly from the original URL.
- Full decode is forbidden above 100,000,000 pixels and may be refused below that limit when the session memory budget is insufficient.
- Two-to-four-image comparison uses viewport proxies by default.
- Large image bytes never cross JSON/Base64 IPC.
- Every artifact belongs to one `SessionId` and is invalid after session close.

---

## Target File Map

```text
crates/viewer-domain/src/image.rs
crates/viewer-application/src/image.rs
crates/viewer-application/src/ports.rs
crates/viewer-platform-macos/src/image/{mod.rs,image_io.rs,quick_look.rs,encode.rs}
crates/viewer-infrastructure/src/image_cache.rs
crates/viewer-test-support/src/image_fixtures.rs
src-tauri/src/image_protocol.rs
src-tauri/src/lib.rs
tests/fixtures/images/{srgb.jpg,p3.jpg,rotated-6.jpg,alpha.png,corrupt.jpg}
docs/adr/0001-macos-image-pipeline.md
```

### Task 1: Define image contracts and memory budgeting

**Files:**
- Create: `crates/viewer-domain/src/image.rs`
- Modify: `crates/viewer-domain/src/lib.rs`
- Create: `crates/viewer-application/src/image.rs`
- Modify: `crates/viewer-application/src/ports.rs`
- Modify: `crates/viewer-application/src/lib.rs`

**Interfaces:**
- Produces: `ImageProbe`, `ImageRepresentationKind`, `ImageRequest`, `ImageArtifact`, `DecodeBudget`, `ImagePort`.

- [ ] **Step 1: Write failing decode-budget tests**

```rust
#[test]
fn rgba_memory_is_width_times_height_times_four() {
    assert_eq!(DecodeBudget::rgba_bytes(12_000, 8_000), Some(384_000_000));
}

#[test]
fn hard_pixel_limit_rejects_full_decode() {
    let budget = DecodeBudget::new(700_000_000, 100_000_000);
    assert!(!budget.allows_full_decode(20_000, 6_000, 4));
}

#[test]
fn four_way_compare_reserves_all_visible_proxies() {
    let budget = DecodeBudget::new(700_000_000, 100_000_000);
    assert!(budget.allows_proxy_set(&[(3_000, 2_000); 4], 4));
}
```

Run `cargo test -p viewer-domain image`. Expected: FAIL because the types do not exist.

- [ ] **Step 2: Implement domain image types**

Create `image.rs` with this public surface:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageFormat { Jpeg, Png }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageRepresentationKind {
    Thumbnail { max_pixels: u32, scale_milli: u16 },
    FitPreview { max_width: u32, max_height: u32, scale_milli: u16 },
    Original100Percent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImageProbe {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
    pub has_alpha: bool,
    pub icc_profile_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeBudget {
    max_bytes: u64,
    hard_pixel_limit: u64,
}

impl DecodeBudget {
    pub const fn new(max_bytes: u64, hard_pixel_limit: u64) -> Self {
        Self { max_bytes, hard_pixel_limit }
    }

    pub fn rgba_bytes(width: u32, height: u32) -> Option<u64> {
        u64::from(width).checked_mul(u64::from(height))?.checked_mul(4)
    }

    pub fn allows_full_decode(&self, width: u32, height: u32, bytes_per_pixel: u8) -> bool {
        let pixels = u64::from(width).saturating_mul(u64::from(height));
        let bytes = pixels.saturating_mul(u64::from(bytes_per_pixel));
        pixels <= self.hard_pixel_limit && bytes <= self.max_bytes
    }

    pub fn allows_proxy_set(&self, dimensions: &[(u32, u32)], bytes_per_pixel: u8) -> bool {
        dimensions.iter().try_fold(0_u64, |sum, (width, height)| {
            sum.checked_add(u64::from(*width) * u64::from(*height) * u64::from(bytes_per_pixel))
        }).is_some_and(|bytes| bytes <= self.max_bytes)
    }
}
```

- [ ] **Step 3: Define the application port**

```rust
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use viewer_domain::{EntityId, SessionId, image::{ImageProbe, ImageRepresentationKind}};

#[derive(Clone, Debug)]
pub struct ImageRequest {
    pub session_id: SessionId,
    pub entity_id: EntityId,
    pub source: PathBuf,
    pub kind: ImageRepresentationKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageBackend { QuickLook, ImageIo }

#[derive(Clone, Debug)]
pub struct ImageArtifact {
    pub cache_path: PathBuf,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub backend: ImageBackend,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unsupported image")]
    Unsupported,
    #[error("image is corrupt")]
    Corrupt,
    #[error("decode exceeds budget")]
    BudgetExceeded,
    #[error("image request was cancelled")]
    Cancelled,
    #[error("image io failed: {0}")]
    Io(String),
}

#[async_trait]
pub trait ImagePort: Send + Sync {
    async fn probe(&self, source: &Path) -> Result<ImageProbe, ImageError>;
    async fn render(&self, request: ImageRequest) -> Result<ImageArtifact, ImageError>;
    async fn cancel_session(&self, session_id: SessionId);
}
```

- [ ] **Step 4: Verify and commit**

Run `cargo test -p viewer-domain -p viewer-application`. Expected: all image contract tests PASS.

```bash
git add crates/viewer-domain crates/viewer-application Cargo.lock
git commit -m "feat: define image pipeline contracts"
```

### Task 2: Implement Image I/O probe and bounded thumbnail rendering

**Files:**
- Modify: `crates/viewer-platform-macos/Cargo.toml`
- Create: `crates/viewer-platform-macos/src/image/mod.rs`
- Create: `crates/viewer-platform-macos/src/image/image_io.rs`
- Create: `crates/viewer-platform-macos/src/image/encode.rs`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Create: `crates/viewer-test-support/src/image_fixtures.rs`
- Add: `tests/fixtures/images/srgb.jpg`
- Add: `tests/fixtures/images/p3.jpg`
- Add: `tests/fixtures/images/rotated-6.jpg`
- Add: `tests/fixtures/images/alpha.png`
- Add: `tests/fixtures/images/corrupt.jpg`
- Add: `tests/fixtures/images/manifest.json`

**Interfaces:**
- Consumes: `ImageProbe`, `ImageError`, `ImageRepresentationKind`.
- Produces: `ImageIoBackend::probe(Path)` and `ImageIoBackend::render(Path, kind, destination)`.

- [ ] **Step 1: Add deterministic fixtures and failing tests**

Add five redistribution-safe fixtures generated specifically for Viewer: sRGB JPEG, Display P3 JPEG, EXIF orientation 6 JPEG, alpha PNG and truncated JPEG. Store their expected metadata in `tests/fixtures/images/manifest.json`.

Write tests asserting:

```rust
#[test]
fn probe_reads_dimensions_orientation_alpha_and_profile() {
    let backend = ImageIoBackend::default();
    let rotated = backend.probe_sync(fixture("rotated-6.jpg")).unwrap();
    assert_eq!((rotated.width, rotated.height), (800, 600));
    assert_eq!(rotated.orientation, 6);

    let alpha = backend.probe_sync(fixture("alpha.png")).unwrap();
    assert_eq!((alpha.width, alpha.height), (640, 480));
    assert!(alpha.has_alpha);

    let p3 = backend.probe_sync(fixture("p3.jpg")).unwrap();
    assert_eq!(p3.icc_profile_name.as_deref(), Some("Display P3"));
}

#[test]
fn corrupt_jpeg_is_isolated() {
    let backend = ImageIoBackend::default();
    assert!(matches!(backend.probe_sync(fixture("corrupt.jpg")), Err(ImageError::Corrupt)));
}

#[test]
fn thumbnail_respects_max_pixel_size() {
    let output = tempfile::NamedTempFile::new().unwrap();
    ImageIoBackend::default()
        .render_thumbnail_sync(fixture("srgb.jpg"), 256, output.path())
        .unwrap();
    let (width, height) = png_dimensions(output.path()).unwrap();
    assert!(width <= 256 && height <= 256);
}
```

Run `cargo test -p viewer-platform-macos image_io`. Expected: FAIL because the backend does not exist.

- [ ] **Step 2: Add macOS framework bindings**

Add target-specific dependencies:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-core-foundation = "0.3"
objc2-core-graphics = "0.3"
objc2-foundation = { version = "0.3", features = ["NSURL"] }
objc2-image-io = { version = "0.3", features = ["CGImageSource", "CGImageDestination", "objc2-core-graphics"] }
```

Implement `ImageIoBackend` around `CGImageSource::with_url`, `properties_at_index`, `thumbnail_at_index` and `CGImageDestination`. Build thumbnail options with:

```text
kCGImageSourceCreateThumbnailFromImageAlways = true
kCGImageSourceCreateThumbnailWithTransform = true
kCGImageSourceThumbnailMaxPixelSize = requested maximum
kCGImageSourceShouldCacheImmediately = true
```

Map only public Image I/O metadata keys into `ImageProbe`; never return raw Core Foundation objects across the adapter boundary. Encode cache artifacts as PNG so alpha and transformed orientation remain deterministic.

- [ ] **Step 3: Run platform tests under Address Sanitizer-compatible debug settings**

Run:

```bash
cargo test -p viewer-platform-macos image_io -- --nocapture
```

Expected: valid fixtures PASS, corrupt fixture returns `ImageError::Corrupt`, no process crash.

- [ ] **Step 4: Commit**

```bash
git add crates/viewer-platform-macos crates/viewer-test-support tests/fixtures/images Cargo.lock
git commit -m "feat: add bounded Image I/O rendering"
```

### Task 3: Implement cancellable Quick Look thumbnails with Image I/O fallback

**Files:**
- Modify: `crates/viewer-platform-macos/Cargo.toml`
- Create: `crates/viewer-platform-macos/src/image/quick_look.rs`
- Modify: `crates/viewer-platform-macos/src/image/mod.rs`
- Test: `crates/viewer-platform-macos/src/image/quick_look.rs`

**Interfaces:**
- Produces: `QuickLookBackend::thumbnail(request, destination)` and `MacImagePort` implementing `ImagePort`.

- [ ] **Step 1: Write failing fallback and cancellation tests**

Use dependency-injected backends so the coordinator is testable without Quick Look:

```rust
#[tokio::test]
async fn quick_look_failure_falls_back_to_image_io() {
    let port = MacImagePort::with_backends(FailingQuickLook, SuccessfulImageIo);
    let artifact = port.render(thumbnail_request()).await.unwrap();
    assert_eq!(artifact.backend, ImageBackend::ImageIo);
}

#[tokio::test]
async fn cancel_session_rejects_pending_thumbnail() {
    let port = MacImagePort::with_backends(BlockingQuickLook::default(), SuccessfulImageIo);
    let request = thumbnail_request();
    port.cancel_session(request.session_id).await;
    assert!(matches!(port.render(request).await, Err(ImageError::Cancelled)));
}
```

Run the tests. Expected: FAIL.

- [ ] **Step 2: Add Quick Look bindings and dedicated executor**

Add:

```toml
block2 = "0.6"
objc2-quick-look-thumbnailing = { version = "0.3", features = ["QLThumbnailGenerationRequest", "QLThumbnailGenerator", "QLThumbnailRepresentation", "block2", "objc2-core-graphics"] }
tokio = { version = "1", features = ["rt", "sync", "time"] }
```

Create `QuickLookBackend` with a dedicated macOS-compatible executor because `QLThumbnailGenerator` is neither `Send` nor `Sync`. Each public request sends an owned command containing source URL, logical size, display scale, destination and a oneshot responder. The executor:

1. Constructs `QLThumbnailGenerationRequest` with thumbnail representation type and `iconMode = false`.
2. Calls `generateBestRepresentationForRequest_completionHandler`.
3. Encodes the returned `CGImage` to the requested destination.
4. Calls `cancelRequest` for registered requests when the owning session is cancelled.
5. Converts nil representation plus `NSError` into `ImageError::Io` without panicking.

The coordinator invokes Image I/O only for Quick Look error, timeout or a fixture flagged by the G1 consistency test.

- [ ] **Step 3: Verify fallback, cancellation and dimensions**

Run:

```bash
cargo test -p viewer-platform-macos quick_look -- --nocapture
```

Expected: coordinator unit tests PASS; real fixture integration test returns a thumbnail no larger than requested physical pixels.

- [ ] **Step 4: Commit**

```bash
git add crates/viewer-platform-macos Cargo.lock
git commit -m "feat: add Quick Look thumbnail backend"
```

### Task 4: Add session cache keys, deduplication and representation registry

**Files:**
- Create: `crates/viewer-infrastructure/src/image_cache.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Test: `crates/viewer-infrastructure/src/image_cache.rs`

**Interfaces:**
- Produces: `ImageCacheKey::from_request`, `ImageArtifactRegistry::insert`, `resolve`, `remove_session`.

- [ ] **Step 1: Write failing key and expiry tests**

```rust
#[test]
fn source_change_changes_cache_key() {
    let first = cache_input(100, 1_000);
    let changed = cache_input(101, 1_001);
    assert_ne!(ImageCacheKey::from_request(&first), ImageCacheKey::from_request(&changed));
}

#[test]
fn registry_cannot_resolve_artifact_from_another_session() {
    let registry = ImageArtifactRegistry::default();
    let owner = SessionId::new();
    let other = SessionId::new();
    let token = registry.insert(owner, EntityId::new(), fixture("srgb.jpg"), "image/jpeg").unwrap();
    assert!(registry.resolve(other, &token).is_none());
}

#[test]
fn closing_session_removes_all_registry_entries() {
    let registry = ImageArtifactRegistry::default();
    let session = SessionId::new();
    let token = registry.insert(session, EntityId::new(), fixture("srgb.jpg"), "image/jpeg").unwrap();
    registry.remove_session(session);
    assert!(registry.resolve(session, &token).is_none());
}
```

Run `cargo test -p viewer-infrastructure image_cache`. Expected: FAIL.

- [ ] **Step 2: Implement cache key and opaque registry**

The key input is exactly:

```rust
pub struct ImageCacheKeyInput<'a> {
    pub project_id: ProjectId,
    pub relative_path: &'a RelativePath,
    pub source_size: u64,
    pub source_mtime_ns: i128,
    pub kind: ImageRepresentationKind,
    pub renderer_version: u16,
}
```

Hash the stable serialization with BLAKE3. The registry returns a random 128-bit URL-safe token and stores `(SessionId, EntityId, cache_path, mime)` behind an `RwLock`. `resolve` requires both token and current `SessionId`; it never accepts a filesystem path from the caller.

- [ ] **Step 3: Verify and commit**

Run `cargo test -p viewer-infrastructure image_cache`. Expected: all tests PASS.

```bash
git add crates/viewer-infrastructure Cargo.lock
git commit -m "feat: add session image cache registry"
```

### Task 5: Serve artifacts through a restricted Tauri protocol

**Files:**
- Create: `src-tauri/src/image_protocol.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`
- Test: `src-tauri/src/image_protocol.rs`

**Interfaces:**
- Consumes: `ImageArtifactRegistry`.
- Produces: `viewer-image://localhost/<session-id>/<token>` responses.

- [ ] **Step 1: Write failing protocol resolver tests**

Test pure request parsing before registering Tauri:

```rust
#[test]
fn protocol_rejects_path_traversal_and_unknown_tokens() {
    let resolver = test_resolver();
    assert_eq!(resolver.resolve("/../secret"), Err(ProtocolError::BadRequest));
    assert_eq!(resolver.resolve(&format!("/{}/unknown", resolver.session_id())), Err(ProtocolError::NotFound));
}

#[test]
fn protocol_rejects_a_token_from_another_session() {
    let resolver = test_resolver();
    let token = resolver.insert_png();
    assert_eq!(resolver.resolve(&format!("/{}/{}", SessionId::new(), token)), Err(ProtocolError::Forbidden));
}

#[test]
fn protocol_returns_only_image_mime_types() {
    let resolver = test_resolver();
    let image = resolver.insert_png();
    assert_eq!(resolver.resolve(&format!("/{}/{}", resolver.session_id(), image)).unwrap().mime, "image/png");
    assert_eq!(resolver.insert_raw("text/plain"), Err(ProtocolError::UnsupportedMediaType));
}
```

Run `cargo test -p viewer-desktop image_protocol`. Expected: FAIL.

- [ ] **Step 2: Implement and register the protocol**

Register `viewer-image` with `register_asynchronous_uri_scheme_protocol`. Parse exactly two normalized path segments: session UUID and registry token. Resolve through the registry, verify `image/jpeg` or `image/png`, then respond with `Content-Type`, `Cache-Control: no-store` and the encoded file bytes. Return a short plain-text body for 400/403/404/500; never include an absolute path.

Update CSP `img-src` to add `viewer-image:` and `http://viewer-image.localhost`; do not broaden `connect-src` or add wildcard sources.

- [ ] **Step 3: Verify and commit**

Run:

```bash
cargo test -p viewer-desktop image_protocol
pnpm verify
```

Expected: protocol tests and complete verification PASS.

```bash
git add src-tauri Cargo.lock
git commit -m "feat: add restricted image protocol"
```

### Task 6: Produce G1 performance and consistency evidence

**Files:**
- Create: `crates/viewer-platform-macos/benches/image_pipeline.rs`
- Create: `scripts/run-g1-image-gate.sh`
- Create: `docs/adr/0001-macos-image-pipeline.md`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`

**Interfaces:**
- Produces: a pass/fail report deciding Quick Look versus Image I/O for JPG/PNG grid thumbnails.

- [ ] **Step 1: Add the benchmark harness**

Benchmark cold and repeated requests for each fixture at 256, 512 and 1024 physical pixels, fit preview at 2560×1600, rapid cancellation across 100 requests, and four simultaneous fit proxies. Record p50, p95, peak RSS and backend used.

Create `scripts/run-g1-image-gate.sh` that runs image unit/integration tests, the release benchmark, protocol tests and a fixture checksum check. Exit non-zero on any failure.

- [ ] **Step 2: Run the gate on the standard M4 device**

Run:

```bash
./scripts/run-g1-image-gate.sh
```

Expected:

- All fixture metadata is correct.
- Thumbnail/preview orientation is identical.
- ICC/Display P3 comparison has no visually detectable grid-to-preview jump.
- Corrupt fixture does not crash the process.
- Cancellation prevents stale publication.
- Four-proxy peak remains within the 700 MB application peak budget.

- [ ] **Step 3: Record the binding decision**

Write ADR 0001 with measured results and exactly one outcome:

- `Quick Look primary, Image I/O fallback`, or
- `Image I/O for all JPG/PNG thumbnails`.

If both paths fail color, stability or memory acceptance, stop G1 and update the system architecture to evaluate an AppKit/Metal preview surface. Do not continue to G2 while claiming G1 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/viewer-platform-macos/benches scripts/run-g1-image-gate.sh docs/adr docs/TECHNICAL_FOUNDATIONS.md
git commit -m "docs: record G1 image pipeline decision"
```

## G1 Completion Check

Run:

```bash
./scripts/run-g1-image-gate.sh
git status --short
```

Expected: gate exits 0, ADR contains a single selected backend policy, and the working tree is clean.
