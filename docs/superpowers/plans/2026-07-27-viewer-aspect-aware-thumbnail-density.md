# Viewer Aspect-Aware Thumbnail Density Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove only thumbnail-container letterbox/pillarbox bands by rendering complete images at their natural aspect ratios, and add one globally persisted compact/standard/large thumbnail-density setting.

**Architecture:** Add a typed Rust settings service backed by an atomically written JSON file in Viewer’s application configuration directory, expose only read-density and update-density Tauri commands, and distribute the desired setting through a React provider. A pure TypeScript geometry module drives an aspect-aware horizontal filmstrip and a separate fixed-height flow grid with bounded virtualization, while a shared thumbnail surface performs natural-dimension recovery without cropping or stretching.

**Tech Stack:** Rust 2024, serde/serde_json, Tauri 2, React 19, TypeScript 6, CSS, Vitest 4, Testing Library, Cargo test/clippy

## Global Constraints

- The approved design in `docs/superpowers/specs/2026-07-27-viewer-aspect-aware-thumbnail-density-design.md` is the source of truth.
- Remove only gray space created by a mismatched container. Preserve every source pixel, including white and transparent pixels.
- Never crop, stretch, justify rows by rescaling, or perform content-aware background detection.
- Use orientation-corrected `BrowserFile.imageMetadata` when valid; otherwise load invisibly, recover `naturalWidth`/`naturalHeight`, reflow once, and only then reveal the image.
- Use image heights of exactly 96, 132, and 168 CSS pixels for `compact`, `standard`, and `large`; `standard` is the default and recovery value.
- Allow unrestricted proportional widths, including panoramas wider than the viewport and extremely narrow portrait cells.
- Preserve source order, selected IDs, active ID, preview context, drag behavior, marquee selection, keyboard navigation, and focused off-window items.
- Apply density only to folder filmstrips and the normal image grid. Do not change search results, compare, text lists, or full preview.
- Store the setting globally at the Tauri-resolved application configuration directory; never read or write a project directory or `.viewer` for this preference.
- Keep the Tauri boundary typed and narrow. The webview cannot choose a path, write arbitrary JSON, or update an arbitrary key.
- Cap thumbnail sampling requests at a device scale of 4 and a physical long edge of 4096 pixels. The cap may affect sharpness but never display geometry.
- Add no layout dependency, M4 acceptance phase, Viewer 0.1 delivery target, or unrelated architecture cleanup.
- Preserve the user-owned untracked fixture path `tests/fixtures/images/.viewer/`.

---

## File Map

### Rust settings path

- Create `crates/viewer-application/src/settings.rs`: density enum, settings value, repository port, service, and application tests.
- Modify `crates/viewer-application/src/lib.rs`: expose the settings module and public types.
- Create `crates/viewer-infrastructure/src/settings.rs`: versioned JSON repository with serialized atomic writes.
- Modify `crates/viewer-infrastructure/src/lib.rs`: expose the repository.
- Create `src-tauri/src/dto/settings.rs`: exact camel-case settings DTO and snake-case density values.
- Create `src-tauri/src/commands/settings.rs`: read settings and update only thumbnail density.
- Modify `src-tauri/src/dto/mod.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/error.rs`, and `src-tauri/src/lib.rs`: register the typed boundary and construct the service from `app_config_dir`.

### UI settings path

- Modify `ui/src/api/types.ts` and `ui/src/api/viewer.ts`: add the typed bridge contract.
- Create `ui/src/api/viewer.test.ts`: pin the two narrow invoke command names and argument shapes.
- Create `ui/src/settings/thumbnailDensity.ts`: density-to-height mapping and bounded physical request sizing.
- Create `ui/src/settings/ViewerSettingsProvider.tsx`: load-once state, immediate optimistic choice, serialized persistence, and latest-choice-wins recovery.
- Create `ui/src/settings/ViewerSettingsProvider.test.tsx`: loading, propagation, rapid updates, and failure rollback.
- Create `ui/src/components/SettingsDialog.tsx` and `ui/src/components/SettingsDialog.test.tsx`: centered accessible settings UI.
- Modify `ui/src/App.tsx` and `ui/src/App.test.tsx`: install the provider, add the project-only gear trigger, and connect the dialog.
- Modify bridge factories in `ui/src/components/EmptyProject.test.tsx` and `ui/src/state/useViewerController.test.tsx`.

### Aspect layout path

- Create `ui/src/layout/aspectLayout.ts` and `ui/src/layout/aspectLayout.test.ts`: validated ratios, fractional geometry, row wrapping, visible windows, hit testing, directional navigation, and scroll anchors.
- Create `ui/src/components/AspectThumbnail.tsx` and `ui/src/components/AspectThumbnail.test.tsx`: shared no-crop image surface and natural-dimension recovery.
- Create `ui/src/components/AspectVirtualGrid.tsx` and `ui/src/components/AspectVirtualGrid.test.tsx`: variable-width flow layout, row virtualization, marquee, focus retention, and scroll anchoring.
- Modify `ui/src/components/marqueeSelection.ts` and its test: host the shared marquee event type and rectangle primitives.
- Modify `ui/src/components/FolderFilmstripRow.tsx`, `FolderFilmstripRow.test.tsx`, `FolderOverview.tsx`, and `FolderOverview.test.tsx`: use proportional horizontal geometry and global density.
- Modify `ui/src/components/ContentBrowser.tsx`, `ContentBrowser.test.tsx`, and `contentBrowser/ImageCell.tsx`: use the aspect grid, remove the local size control, and preserve all image interactions.
- Delete `ui/src/components/VirtualGrid.tsx` and `ui/src/components/VirtualGrid.test.tsx` after the new grid owns all callers.
- Modify `ui/src/styles/app.css` and `ui/src/styles/app.test.ts`: remove successful-image gray bands and style the new layouts/settings in light and dark appearances.

## Task 1: Define the typed application settings service

**Files:**

- Create: `crates/viewer-application/src/settings.rs`
- Modify: `crates/viewer-application/src/lib.rs`

- [ ] **Step 1: Write failing application-layer tests**

Add tests for the default, successful update, and write failure. Use an in-memory port so these tests do not touch the filesystem:

```rust
#[derive(Default)]
struct MemorySettingsPort {
    saved: Mutex<Vec<ViewerSettings>>,
    fail: AtomicBool,
}

impl ViewerSettingsPort for MemorySettingsPort {
    fn load(&self) -> ViewerSettings {
        self.saved
            .lock()
            .expect("settings lock")
            .last()
            .copied()
            .unwrap_or_default()
    }

    fn save(&self, settings: ViewerSettings) -> Result<(), ViewerSettingsError> {
        if self.fail.load(Ordering::Acquire) {
            return Err(ViewerSettingsError::Unavailable);
        }
        self.saved.lock().expect("settings lock").push(settings);
        Ok(())
    }
}
```

Assert that `ViewerSettings::default().thumbnail_density` is `Standard`, updating to `Large` saves exactly one complete settings value, and a failed save returns `Unavailable`.

- [ ] **Step 2: Run the focused test and verify red**

```bash
cargo test -p viewer-application settings
```

Expected: FAIL because `viewer_application::settings` does not exist.

- [ ] **Step 3: Implement the exact application contract**

Create these public types and keep filesystem/JSON knowledge out of this crate:

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const VIEWER_SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailDensity {
    Compact,
    #[default]
    Standard,
    Large,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewerSettings {
    pub thumbnail_density: ThumbnailDensity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ViewerSettingsError {
    #[error("viewer settings are unavailable")]
    Unavailable,
}

pub trait ViewerSettingsPort: Send + Sync {
    fn load(&self) -> ViewerSettings;
    fn save(&self, settings: ViewerSettings) -> Result<(), ViewerSettingsError>;
}

pub struct ViewerSettingsService {
    store: Arc<dyn ViewerSettingsPort>,
}

impl ViewerSettingsService {
    pub fn new(store: Arc<dyn ViewerSettingsPort>) -> Self {
        Self { store }
    }

    pub fn load(&self) -> ViewerSettings {
        self.store.load()
    }

    pub fn update_thumbnail_density(
        &self,
        thumbnail_density: ThumbnailDensity,
    ) -> Result<ViewerSettings, ViewerSettingsError> {
        let settings = ViewerSettings { thumbnail_density };
        self.store.save(settings)?;
        Ok(settings)
    }
}
```

Export these types from `crates/viewer-application/src/lib.rs`.

- [ ] **Step 4: Run focused tests and formatting**

```bash
cargo test -p viewer-application settings
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 5: Commit the application contract**

```bash
git add crates/viewer-application/src/settings.rs crates/viewer-application/src/lib.rs
git commit -m "feat: define global viewer settings service"
```

## Task 2: Persist versioned settings atomically outside projects

**Files:**

- Create: `crates/viewer-infrastructure/src/settings.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`

- [ ] **Step 1: Write failing repository tests**

Use `tempfile::tempdir()` and cover:

```rust
#[test]
fn missing_settings_use_standard() {
    let directory = tempfile::tempdir().unwrap();
    let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());
    assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Standard);
}

#[test]
fn round_trip_uses_the_exact_versioned_shape() {
    let directory = tempfile::tempdir().unwrap();
    let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());
    store
        .save(ViewerSettings {
            thumbnail_density: ThumbnailDensity::Large,
        })
        .unwrap();

    let json = std::fs::read_to_string(directory.path().join("settings.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap(),
        serde_json::json!({
            "schemaVersion": 1,
            "thumbnailDensity": "large"
        })
    );
    assert_eq!(store.load().thumbnail_density, ThumbnailDensity::Large);
}
```

Add separate cases for malformed JSON, an extra field, invalid density, schema version `2`, and a settings directory path that is actually a file. Invalid reads must return `standard`; the impossible write must return `ViewerSettingsError::Unavailable`. Place a sentinel file in a sibling fake project directory and assert it is unchanged.

- [ ] **Step 2: Run the focused test and verify red**

```bash
cargo test -p viewer-infrastructure settings
```

Expected: FAIL because `JsonViewerSettingsStore` does not exist.

- [ ] **Step 3: Implement the JSON store**

Use this exact serialized shape:

```rust
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredViewerSettings {
    schema_version: u32,
    thumbnail_density: ThumbnailDensity,
}

pub struct JsonViewerSettingsStore {
    directory: PathBuf,
    write_lock: Mutex<()>,
}
```

`load` must read only `<directory>/settings.json`, deserialize with `deny_unknown_fields`, require `schema_version == VIEWER_SETTINGS_SCHEMA_VERSION`, and return `ViewerSettings::default()` for every missing, unreadable, malformed, or unsupported value.

`save` must:

1. acquire `write_lock`;
2. create the owned configuration directory;
3. serialize a complete `StoredViewerSettings`;
4. create/truncate `<directory>/settings.json.tmp`;
5. `write_all`, `sync_all`, and close the temporary file;
6. rename it to `<directory>/settings.json`;
7. map every write failure to `ViewerSettingsError::Unavailable`.

No error string may contain the configuration path.

- [ ] **Step 4: Run repository and workspace tests**

```bash
cargo test -p viewer-infrastructure settings
cargo test -p viewer-application -p viewer-infrastructure
```

Expected: PASS, including exact JSON and isolation assertions.

- [ ] **Step 5: Commit the repository**

```bash
git add crates/viewer-infrastructure/src/settings.rs crates/viewer-infrastructure/src/lib.rs
git commit -m "feat: persist viewer settings atomically"
```

## Task 3: Expose the narrow Tauri and TypeScript settings boundary

**Files:**

- Create: `src-tauri/src/dto/settings.rs`
- Create: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/dto/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.ts`
- Create: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/EmptyProject.test.tsx`
- Modify: `ui/src/state/useViewerController.test.tsx`

- [ ] **Step 1: Write failing boundary tests**

In `src-tauri/src/dto/settings.rs`, test this exact response:

```rust
assert_eq!(
    serde_json::to_value(ViewerSettingsDto::from(ViewerSettings {
        thumbnail_density: ThumbnailDensity::Compact,
    }))
    .unwrap(),
    serde_json::json!({
        "schemaVersion": 1,
        "thumbnailDensity": "compact"
    })
);
```

Assert that `"thumbnail_density"` and `"dense"` do not deserialize as `ThumbnailDensityDto`. In `src-tauri/src/error.rs`, assert a settings write failure maps to:

```rust
CommandError::new(
    "settings_write_failed",
    ErrorCategory::Environment,
    "设置未能保存",
    true,
)
```

In `ui/src/api/viewer.test.ts`, mock `invoke` and assert command names/arguments are exactly:

```ts
await tauriViewerBridge.getViewerSettings()
expect(invoke).toHaveBeenCalledWith('get_viewer_settings')

await tauriViewerBridge.updateThumbnailDensity('large')
expect(invoke).toHaveBeenCalledWith('update_thumbnail_density', { density: 'large' })
```

- [ ] **Step 2: Run focused tests and verify red**

```bash
cargo test -p viewer-desktop settings
pnpm --dir ui exec vitest run src/api/viewer.test.ts
```

Expected: FAIL because the DTO, commands, and bridge methods are absent.

- [ ] **Step 3: Implement DTOs and commands**

Use these DTOs:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailDensityDto {
    Compact,
    Standard,
    Large,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerSettingsDto {
    pub schema_version: u32,
    pub thumbnail_density: ThumbnailDensityDto,
}
```

Implement `From` conversions in both directions for density and from `ViewerSettings` to `ViewerSettingsDto`. Add commands:

```rust
#[tauri::command]
pub fn get_viewer_settings(
    service: tauri::State<'_, Arc<ViewerSettingsService>>,
) -> ViewerSettingsDto {
    service.load().into()
}

#[tauri::command]
pub fn update_thumbnail_density(
    density: ThumbnailDensityDto,
    service: tauri::State<'_, Arc<ViewerSettingsService>>,
) -> Result<ViewerSettingsDto, CommandError> {
    service
        .update_thumbnail_density(density.into())
        .map(Into::into)
        .map_err(Into::into)
}
```

Register both commands. In `setup`, resolve `app.path().app_config_dir()?`, construct `JsonViewerSettingsStore` from that directory, wrap it in `ViewerSettingsService`, and manage `Arc<ViewerSettingsService>`. Do not add a filesystem capability.

- [ ] **Step 4: Add the frontend contract and update every bridge factory**

Add:

```ts
export type ThumbnailDensity = 'compact' | 'standard' | 'large'

export interface ViewerSettings {
  schemaVersion: 1
  thumbnailDensity: ThumbnailDensity
}
```

Extend `ViewerBridge` with:

```ts
getViewerSettings(): Promise<ViewerSettings>
updateThumbnailDensity(density: ThumbnailDensity): Promise<ViewerSettings>
```

Implement both `invoke` calls. Every test bridge must provide stable defaults:

```ts
getViewerSettings: vi.fn().mockResolvedValue({
  schemaVersion: 1,
  thumbnailDensity: 'standard',
}),
updateThumbnailDensity: vi.fn().mockImplementation(async (thumbnailDensity) => ({
  schemaVersion: 1,
  thumbnailDensity,
})),
```

- [ ] **Step 5: Verify Rust and TypeScript boundaries**

```bash
cargo test -p viewer-desktop settings
pnpm --dir ui exec vitest run src/api/viewer.test.ts src/App.test.tsx src/components/EmptyProject.test.tsx src/state/useViewerController.test.tsx
pnpm --dir ui typecheck
```

Expected: PASS with no path or arbitrary-key command.

- [ ] **Step 6: Commit the boundary**

```bash
git add crates/viewer-application crates/viewer-infrastructure src-tauri ui/src/api ui/src/App.test.tsx ui/src/components/EmptyProject.test.tsx ui/src/state/useViewerController.test.tsx
git commit -m "feat: expose typed thumbnail density settings"
```

## Task 4: Build the global settings provider and modal

**Files:**

- Create: `ui/src/settings/thumbnailDensity.ts`
- Create: `ui/src/settings/ViewerSettingsProvider.tsx`
- Create: `ui/src/settings/ViewerSettingsProvider.test.tsx`
- Create: `ui/src/components/SettingsDialog.tsx`
- Create: `ui/src/components/SettingsDialog.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/styles/app.css`

- [ ] **Step 1: Write failing provider tests**

Test first-load `standard`, backend-loaded `compact`, immediate optimistic propagation, ordered rapid updates (`compact → large`), stale completion immunity, latest failure rollback, and error clearing when a new choice begins. A deferred bridge test must prove the UI reads `large` before its save promise resolves.

Use the public context shape:

```ts
interface ViewerSettingsContextValue {
  thumbnailDensity: ThumbnailDensity
  thumbnailHeight: number
  settingsError: string | null
  setThumbnailDensity: (density: ThumbnailDensity) => void
}
```

- [ ] **Step 2: Run provider tests and verify red**

```bash
pnpm --dir ui exec vitest run src/settings/ViewerSettingsProvider.test.tsx
```

Expected: FAIL because the provider does not exist.

- [ ] **Step 3: Implement density constants and serialized latest-choice-wins persistence**

In `thumbnailDensity.ts`:

```ts
export const THUMBNAIL_HEIGHT: Record<ThumbnailDensity, number> = {
  compact: 96,
  standard: 132,
  large: 168,
}

export const MAX_THUMBNAIL_DEVICE_SCALE = 4
export const MAX_THUMBNAIL_PHYSICAL_EDGE = 4096

export function thumbnailRequestSize(width: number, height: number, devicePixelRatio: number) {
  const scale = Math.min(
    MAX_THUMBNAIL_DEVICE_SCALE,
    Math.max(1, Number.isFinite(devicePixelRatio) ? devicePixelRatio : 1),
  )
  return {
    maxPixels: Math.min(
      MAX_THUMBNAIL_PHYSICAL_EDGE,
      Math.max(1, Math.ceil(Math.max(width, height) * scale)),
    ),
    scaleMilli: Math.round(scale * 1_000),
  }
}
```

In the provider, keep `confirmedRef`, `latestSequenceRef`, and `writeTailRef`. Each choice must clear the previous save error, set visible state immediately, increment the sequence, and append a save to:

```ts
writeTailRef.current = writeTailRef.current
  .catch(() => undefined)
  .then(() => bridge.updateThumbnailDensity(nextDensity))
  .then(
    (saved) => {
      confirmedRef.current = saved.thumbnailDensity
      if (sequence === latestSequenceRef.current) {
        setThumbnailDensityState(saved.thumbnailDensity)
        setSettingsError(null)
      }
    },
    (error: unknown) => {
      if (sequence === latestSequenceRef.current) {
        setThumbnailDensityState(confirmedRef.current)
        setSettingsError(safeUserMessage(error))
      }
    },
  )
```

Ignore a late initial load after the first user sequence. Loading failure uses `standard` without a blocking error.

- [ ] **Step 4: Write and run failing dialog/App tests**

Cover:

- no `软件设置` button on the empty-project surface;
- one real `软件设置` button beside `项目菜单` after project open;
- a centered `软件设置` dialog with a `显示` section and one `缩略图密度` radio group;
- `紧凑`, `标准`, and `大图` radio options;
- immediate context update on radio selection;
- close button, Escape, focus trap, and trigger focus restoration;
- visible `设置未能保存` on latest save failure.

Run:

```bash
pnpm --dir ui exec vitest run src/components/SettingsDialog.test.tsx src/App.test.tsx
```

Expected: FAIL before the dialog and trigger are implemented.

- [ ] **Step 5: Implement the dialog and project-only trigger**

Build `SettingsDialog` on the existing `ModalSheet`; use `<fieldset>`/`<legend>` and three native radio inputs. The dialog must accept:

```ts
interface SettingsDialogProps {
  density: ThumbnailDensity
  error: string | null
  onDensityChange: (density: ThumbnailDensity) => void
  onClose: () => void
}
```

Wrap the current App workspace in `ViewerSettingsProvider`. Because the existing no-project branch returns before `workspace-header`, place this button only in the open-project header, immediately before `.project-menu`:

```tsx
<button
  ref={settingsButtonRef}
  type="button"
  className="settings-trigger"
  aria-label="软件设置"
  onClick={() => setSettingsOpen(true)}
>
  ⚙
</button>
```

Render `SettingsDialog` only while open. Closing must not change density.

- [ ] **Step 6: Verify and commit settings UI**

```bash
pnpm --dir ui exec vitest run src/settings/ViewerSettingsProvider.test.tsx src/components/SettingsDialog.test.tsx src/App.test.tsx
pnpm --dir ui check
git add ui/src/settings ui/src/components/SettingsDialog.tsx ui/src/components/SettingsDialog.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css
git commit -m "feat: add global thumbnail density settings"
```

## Task 5: Implement the pure aspect geometry engine

**Files:**

- Create: `ui/src/layout/aspectLayout.ts`
- Create: `ui/src/layout/aspectLayout.test.ts`
- Modify: `ui/src/components/marqueeSelection.ts`
- Modify: `ui/src/components/marqueeSelection.test.ts`

- [ ] **Step 1: Write the complete geometry test matrix**

Test:

- portrait `2:3`, square `1:1`, landscape `3:2`, panorama `10:1`, and extreme portrait `1:10`;
- invalid `null`, zero, negative, infinite, NaN, and overflowing quotients;
- fractional cumulative offsets without per-item rounding;
- filmstrip binary-search windows and focused-index retention;
- fixed-height source-order wrapping and left-aligned final rows;
- one natural-width item wider than the viewport;
- vertical row windows;
- rectangle hit testing against unmounted item geometry;
- Left/Right source order and nearest-center Up/Down;
- first-visible anchor reconstruction after density and viewport changes.

Representative exact assertions:

```ts
expect(proportionalWidth(132, { width: 2, height: 3 })).toBe(88)
expect(proportionalWidth(132, { width: 10, height: 1 })).toBe(1320)
expect(proportionalWidth(132, { width: 1, height: 10 })).toBe(13.2)

const strip = buildFilmstripGeometry(
  [
    { key: 'portrait', dimensions: { width: 2, height: 3 } },
    { key: 'landscape', dimensions: { width: 3, height: 2 } },
  ],
  132,
  8,
  12,
)
expect(strip.items.map(({ left, width }) => [left, width])).toEqual([
  [12, 88],
  [108, 198],
])
expect(strip.totalWidth).toBe(318)
```

- [ ] **Step 2: Run the layout tests and verify red**

```bash
pnpm --dir ui exec vitest run src/layout/aspectLayout.test.ts src/components/marqueeSelection.test.ts
```

Expected: FAIL because the geometry module does not exist.

- [ ] **Step 3: Implement stable geometry types and functions**

Use these types:

```ts
export interface ImageDimensions {
  width: number
  height: number
}

export interface AspectSource {
  key: string
  dimensions: ImageDimensions | null
}

export interface AspectRect {
  key: string
  index: number
  row: number
  left: number
  top: number
  width: number
  height: number
  imageWidth: number
  imageHeight: number
}

export interface AspectRow {
  index: number
  start: number
  end: number
  top: number
  height: number
}

export interface AspectGeometry {
  items: AspectRect[]
  rows: AspectRow[]
  indexByKey: Map<string, number>
  totalWidth: number
  totalHeight: number
}
```

Export:

```ts
export function validDimensions(value: ImageDimensions | null): value is ImageDimensions
export function proportionalWidth(imageHeight: number, dimensions: ImageDimensions | null): number
export function buildFilmstripGeometry(
  sources: readonly AspectSource[],
  imageHeight: number,
  gap: number,
  inlinePadding: number,
): AspectGeometry
export function buildFlowGeometry(
  sources: readonly AspectSource[],
  availableWidth: number,
  imageHeight: number,
  captionHeight: number,
  gap: number,
): AspectGeometry
export function horizontalVisibleIndexes(
  geometry: AspectGeometry,
  scrollLeft: number,
  viewportWidth: number,
  overscanPixels: number,
): { start: number; end: number }
export function verticalVisibleRows(
  geometry: AspectGeometry,
  scrollTop: number,
  viewportHeight: number,
  overscanRows: number,
): { start: number; end: number }
export function intersectingAspectIndexes(
  geometry: AspectGeometry,
  rect: MarqueeRect,
): number[]
export function directionalNeighbor(
  geometry: AspectGeometry,
  activeIndex: number,
  direction: 'left' | 'right' | 'up' | 'down',
): number
export function anchoredScrollOffset(
  previous: AspectGeometry,
  next: AspectGeometry,
  key: string,
  previousScroll: number,
  axis: 'horizontal' | 'vertical',
): number
```

For invalid dimensions, `proportionalWidth` returns the image height as the square skeleton width. Retain floating-point widths and offsets. Reject non-finite derived geometry by taking the same square fallback.

Move `MarqueePhase` and `MarqueeSelectionChange` from `VirtualGrid.tsx` to `marqueeSelection.ts` so both grid implementations can share them without importing a component.

- [ ] **Step 4: Run pure tests and property invariants**

```bash
pnpm --dir ui exec vitest run src/layout/aspectLayout.test.ts src/components/marqueeSelection.test.ts
```

Every generated item must have finite positive dimensions, monotonic offsets, stable source index, and an end within the reported scroll extent.

- [ ] **Step 5: Commit the geometry engine**

```bash
git add ui/src/layout ui/src/components/marqueeSelection.ts ui/src/components/marqueeSelection.test.ts
git commit -m "feat: add proportional thumbnail geometry"
```

## Task 6: Share thumbnail recovery and migrate folder filmstrips

**Files:**

- Create: `ui/src/components/AspectThumbnail.tsx`
- Create: `ui/src/components/AspectThumbnail.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.test.tsx`
- Modify: `ui/src/components/FolderOverview.tsx`
- Modify: `ui/src/components/FolderOverview.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

- [ ] **Step 1: Write failing shared-thumbnail tests**

Cover:

- valid metadata renders the complete image at the supplied proportional width/height;
- no `object-fit: cover` and no crop;
- missing metadata renders a square skeleton while an image loads with `visibility: hidden`;
- positive `naturalWidth`/`naturalHeight` calls recovery with `{ entityId, modifiedNs, width, height }`;
- recovered rerender reveals the image;
- invalid natural dimensions stay in the placeholder state;
- a request failure displays `缩略图不可用`;
- request sizing uses DPR and caps at `{ maxPixels: 4096, scaleMilli: 4000 }`.

- [ ] **Step 2: Run the component tests and verify red**

```bash
pnpm --dir ui exec vitest run src/components/AspectThumbnail.test.tsx
```

Expected: FAIL because `AspectThumbnail` does not exist.

- [ ] **Step 3: Implement the shared surface**

Use this contract:

```ts
interface AspectThumbnailProps {
  file: BrowserFile
  width: number
  height: number
  dimensionsKnown: boolean
  loadThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onNaturalDimensions: (
    identity: { entityId: string; modifiedNs: string },
    dimensions: ImageDimensions,
  ) => void
}
```

The outer `.aspect-thumbnail` and `<img>` must have the same explicit width and height. For unknown dimensions, mount the image to obtain its natural dimensions but keep it hidden; do not reveal it until the parent rebuilds geometry with `dimensionsKnown=true`.

- [ ] **Step 4: Replace fixed filmstrip tests with proportional geometry tests**

Update `FolderFilmstripRow.test.tsx` to assert mixed ratios at the chosen height, exact track width, bounded mounting with 1,000 mixed-ratio images, source order, focus retention, anchor retention after density change, preview context, missing-metadata recovery, failure/retry isolation, and an extreme panorama reachable by horizontal scrolling.

Change the row contract to:

```ts
interface FolderFilmstripRowProps {
  folder: ContentFolderCard
  density: ThumbnailDensity
  loadImages: (entityId: string, retry?: boolean) => Promise<BrowserFile[]>
  onSelect: (entityId: string) => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestThumbnail?: (
    file: BrowserFile,
    maxPixels: number,
    scaleMilli: number,
  ) => Promise<string>
}
```

Key recovered dimensions by `${file.entityId}:${file.modifiedNs}`. Rebuild `buildFilmstripGeometry` when density or recovered dimensions change. Use binary-search visibility plus 2 cells and 256 pixels overscan, and union the focused index into the mounted set. Preserve the first visible key and its inline offset when rebuilding.

- [ ] **Step 5: Propagate density through the category surface**

Read density from `ViewerSettingsProvider` in the App workspace and pass it through `FolderOverview` to every row. Change App’s folder thumbnail callback to the same `(file, maxPixels, scaleMilli)` signature used by content thumbnails.

Style successful thumbnail buttons with a transparent background and an inset/outline focus treatment that does not change image geometry. Gray may remain only for deferred, loading, and error states.

- [ ] **Step 6: Verify and commit the filmstrip migration**

```bash
pnpm --dir ui exec vitest run src/components/AspectThumbnail.test.tsx src/components/FolderFilmstripRow.test.tsx src/components/FolderOverview.test.tsx src/App.test.tsx src/styles/app.test.ts
pnpm --dir ui typecheck
git add ui/src/components/AspectThumbnail.tsx ui/src/components/AspectThumbnail.test.tsx ui/src/components/FolderFilmstripRow.tsx ui/src/components/FolderFilmstripRow.test.tsx ui/src/components/FolderOverview.tsx ui/src/components/FolderOverview.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: render aspect-aware folder filmstrips"
```

## Task 7: Build the aspect-aware virtual flow grid

**Files:**

- Create: `ui/src/components/AspectVirtualGrid.tsx`
- Create: `ui/src/components/AspectVirtualGrid.test.tsx`
- Modify: `ui/src/components/marqueeSelection.ts`

- [ ] **Step 1: Write failing variable-grid tests**

Test:

- `ResizeObserver` width changes rebuild rows without changing source order;
- all image surfaces use one supplied height and variable proportional widths;
- the final row remains left-aligned;
- an over-wide item remains natural width and the grid exposes horizontal overflow;
- only visible rows plus two overscan rows mount for 1,000 items;
- a focused item remains mounted outside the visibility window;
- marquee selects by geometry, including an item outside the mounted DOM;
- density/viewport reflow preserves the first visible entity’s visual offset;
- Escape cancels marquee and auto-scroll cleanup remains intact.

- [ ] **Step 2: Run focused tests and verify red**

```bash
pnpm --dir ui exec vitest run src/components/AspectVirtualGrid.test.tsx
```

Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement the variable-geometry grid**

Use this public contract:

```ts
interface AspectVirtualGridProps<T> {
  items: readonly T[]
  imageHeight: number
  captionHeight?: number
  viewportHeight?: number
  gap?: number
  overscanRows?: number
  getKey: (item: T) => string
  getDimensions: (item: T) => ImageDimensions | null
  renderItem: (item: T, index: number, rect: AspectRect) => ReactNode
  ariaLabel: string
  activeKey?: string
  activeDescendant?: string
  onNavigate?: (index: number, extendSelection: boolean) => void
  onKeyDown?: KeyboardEventHandler<HTMLDivElement>
  ariaMultiselectable?: boolean
  onMarqueeSelectionChange?: (change: MarqueeSelectionChange) => void
}
```

Defaults are `captionHeight=48`, `viewportHeight=520`, `gap=12`, and `overscanRows=2`. Measure the container with `ResizeObserver`, call `buildFlowGeometry`, binary-search visible rows, and absolutely position only visible items plus the active/focused key. The scroll surface uses `overflow: auto`, allowing a lone panorama to be reached horizontally.

On a geometry change, capture the first visible item key and its previous vertical offset, then apply `anchoredScrollOffset` in a layout effect. Marquee pointer coordinates include both `scrollLeft` and `scrollTop`, and hit testing always uses the complete geometry table. When an arrow key is pressed and `activeKey` is present, the grid calls `directionalNeighbor` and reports the resulting source index through `onNavigate`; it does not own selection state.

- [ ] **Step 4: Verify bounded rendering and interaction primitives**

```bash
pnpm --dir ui exec vitest run src/components/AspectVirtualGrid.test.tsx src/layout/aspectLayout.test.ts src/components/marqueeSelection.test.ts
pnpm --dir ui typecheck
```

Expected: PASS; the 1,000-item test must assert a bounded mounted count rather than a timing threshold.

- [ ] **Step 5: Commit the new grid**

```bash
git add ui/src/components/AspectVirtualGrid.tsx ui/src/components/AspectVirtualGrid.test.tsx ui/src/components/marqueeSelection.ts
git commit -m "feat: add aspect-aware virtual flow grid"
```

## Task 8: Migrate the normal image grid and preserve interactions

**Files:**

- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/contentBrowser/ImageCell.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`
- Delete: `ui/src/components/VirtualGrid.tsx`
- Delete: `ui/src/components/VirtualGrid.test.tsx`

- [ ] **Step 1: Write failing content-grid behavior tests**

Add mixed-ratio tests asserting:

- image surface heights are 96/132/168 from global density;
- widths are proportional and source order wraps left-to-right;
- no successful image has visible gray container area;
- the `视图` menu retains `全选当前文件夹` but has no `缩略图大小` control;
- click, Command-click, Shift range, marquee, Space preview, double-click preview, Finder drag, organization drag, and radial/context menu still carry the same entity IDs;
- Left/Right follow source order;
- Up/Down choose the closest horizontal center in the adjacent row;
- selection, active ID, and preview context survive a density change;
- missing metadata recovers and reflows without showing the wrong ratio;
- a panorama is horizontally reachable and not cropped;
- high-cardinality rendering remains bounded.

- [ ] **Step 2: Run focused tests and verify red**

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx src/App.test.tsx src/styles/app.test.ts
```

Expected: FAIL while `ContentBrowser` still uses `GridSize` and `VirtualGrid`.

- [ ] **Step 3: Replace local sizing and fixed-grid geometry**

Add a required `density: ThumbnailDensity` prop to `ContentBrowser`. Remove:

```ts
type GridSize = 'small' | 'medium' | 'large'
const GRID_PIXELS: Record<GridSize, number>
const [gridSize, setGridSize] = useState<GridSize>('medium')
```

Remove only the `缩略图大小` label/select from the `视图` menu.

Keep a recovered-dimensions map keyed by entity/modification identity. Render `workspace.images` through `AspectVirtualGrid` with `imageHeight={THUMBNAIL_HEIGHT[density]}`. Pass each returned `AspectRect` to `ImageCell` and use `AspectThumbnail` inside the cell.

Replace numeric `±4` keyboard navigation with a source-index callback passed to `AspectVirtualGrid`:

```ts
function navigateToIndex(nextIndex: number, extendSelection: boolean) {
  const file = workspace.images[nextIndex]
  if (file === undefined) return
  setActiveId(file.entityId)
  if (extendSelection && anchorId.current !== null) {
    const range = rangeSelection(
      workspace.images.map((candidate) => candidate.entityId),
      anchorId.current,
      file.entityId,
    )
    commitSelection(new Set([...selected, ...range]))
    return
  }
  anchorId.current = file.entityId
  commitSelection(new Set([file.entityId]))
}
```

Continue to compute Shift ranges from `workspace.images.map(file => file.entityId)`, not row or mounted DOM order.

- [ ] **Step 4: Preserve request caching and interaction ownership**

Use `thumbnailRequestSize(rect.imageWidth, rect.imageHeight, window.devicePixelRatio)` for each request. Keep the cache key as:

```ts
`${file.entityId}:${file.modifiedNs}:${maxPixels}:${scaleMilli}`
```

Do not change preview, selection, drag, Finder export, organization, or radial-menu callback payloads. Keep text files in their existing separate list and outside density changes.

- [ ] **Step 5: Remove the obsolete fixed grid**

After `rg "VirtualGrid" ui/src` returns only the new `AspectVirtualGrid` name, delete `VirtualGrid.tsx` and its fixed-layout tests. Import `MarqueeSelectionChange` from `marqueeSelection.ts`.

- [ ] **Step 6: Verify and commit the content migration**

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx src/components/AspectVirtualGrid.test.tsx src/components/AspectThumbnail.test.tsx src/App.test.tsx src/styles/app.test.ts
pnpm --dir ui check
git add ui/src/components ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: render proportional content image rows"
```

## Task 9: Run cross-layer regression and governance verification

**Files:**

- Modify only if a failing in-scope assertion requires correction.

- [ ] **Step 1: Scan for forbidden and obsolete behavior**

```bash
rg -n "THUMBNAIL_SIZE|THUMBNAIL_STRIDE|GRID_PIXELS|缩略图大小|object-fit:\\s*cover|M4|Viewer 0\\.1.*交付" ui/src crates src-tauri docs/superpowers/specs/2026-07-27-viewer-aspect-aware-thumbnail-density-design.md docs/superpowers/plans/2026-07-27-viewer-aspect-aware-thumbnail-density.md
```

Expected: no obsolete fixed-size/local-control/crop implementation and no newly introduced M4 or delivery target. Mentions that explicitly prohibit M4/Viewer 0.1 delivery goals in the approved design and this plan are allowed.

- [ ] **Step 2: Run focused cross-layer suites**

```bash
pnpm --dir ui exec vitest run src/settings src/layout src/components/SettingsDialog.test.tsx src/components/AspectThumbnail.test.tsx src/components/AspectVirtualGrid.test.tsx src/components/FolderFilmstripRow.test.tsx src/components/FolderOverview.test.tsx src/components/ContentBrowser.test.tsx src/App.test.tsx src/styles/app.test.ts
cargo test -p viewer-application settings
cargo test -p viewer-infrastructure settings
cargo test -p viewer-desktop settings
```

Expected: PASS.

- [ ] **Step 3: Run the repository quality gate**

```bash
pnpm verify:clean
```

Expected: repository policy, UI checks/tests/build, Rust formatting/clippy/tests, and security checks all pass. The pre-existing untracked fixture path remains preserved and must be handled according to the repository’s clean-verification allowlist rather than deleted.

- [ ] **Step 4: Run quality reporting**

```bash
pnpm quality:report
```

Expected: UI/Rust coverage and architecture-health thresholds pass. If a baseline update is required by an intentional measured improvement, review the generated delta before committing it.

- [ ] **Step 5: Perform an accessibility and visual smoke test**

Run the current development build and verify with a project containing portrait, square, landscape, panorama, extreme portrait, and missing-metadata images:

1. Successful thumbnails show the complete image with no container-generated gray bands.
2. Every folder filmstrip uses one height and proportional widths.
3. Normal image rows wrap in stable source order and remain left-aligned.
4. All three density choices apply immediately to both surfaces and persist after restart.
5. The settings trigger is absent before project open and restored focus after dialog close.
6. Search results, compare, text lists, and full preview do not change size.
7. Keyboard, marquee, preview, drag, and context interactions remain functional.

- [ ] **Step 6: Commit only verified corrective changes**

If Steps 1–5 required changes:

```bash
git add crates src-tauri ui docs/quality
git commit -m "test: verify aspect-aware thumbnail density"
```

If no corrections or intentional baseline changes were needed, do not create an empty commit.

## Completion Criteria

- Both thumbnail surfaces render successful images at exact natural proportions without crop, stretch, or container bands.
- Compact/standard/large map to 96/132/168 CSS pixels and are globally persisted in the app configuration directory.
- Rapid setting changes are serialized and only the latest choice can confirm or revert visible state.
- Invalid settings safely recover to `standard`; a failed latest write reverts and displays `设置未能保存`.
- Filmstrip and content-grid rendering remain bounded for high-cardinality folders.
- Selection, keyboard, marquee, preview, drag, context menu, focus, and scroll anchors pass regression tests.
- The old content-local thumbnail size control and fixed-grid implementation are gone.
- No project metadata, `.viewer`, new dependency, M4 gate, or Viewer 0.1 delivery goal is introduced.
- `pnpm verify:clean` and `pnpm quality:report` pass.
