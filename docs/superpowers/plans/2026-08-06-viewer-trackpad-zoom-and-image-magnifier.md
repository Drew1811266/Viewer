# Viewer Trackpad Zoom and Image Magnifier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add center-anchored MacBook trackpad zoom, bounded two-axis trackpad pan, and an original-detail magnifier that is toggled by Q or a preview-toolbar button and configured through persisted Viewer settings.

**Architecture:** Keep `ImagePreview` as the session orchestrator, but move viewport math, gesture normalization, current-original ownership, and lens rendering into focused modules under `ui/src/components/imagePreview`. Upgrade the existing settings schema from v1 to v2 and save complete settings snapshots through Rust, Tauri, the TypeScript bridge, and the optimistic provider. Use the existing bounded `original100_percent` pipeline for the lens and retain one current original only; pointer motion remains frontend-only and updates lens CSS variables at most once per animation frame.

**Tech Stack:** React 19, TypeScript 6, CSS custom properties, Vitest, Testing Library, Tauri 2, Rust, serde/serde_json, Image I/O, Lucide assets, Viewer visual/native acceptance harnesses, and a physical MacBook trackpad gate.

## Global Constraints

- The approved design is `docs/superpowers/specs/2026-08-06-viewer-trackpad-zoom-and-image-magnifier-design.md`; implementation decisions must not weaken it.
- Trackpad pinch zooms continuously around the gesture center; ordinary two-finger deltas pan in both axes and never navigate images.
- Keep the existing zoom range of 10% through 800%, the current fit representation for free zoom, and the existing `original100_percent` representation for original detail.
- Every zoom path—trackpad and toolbar minus/plus—must use the same anchor-preserving viewport action.
- Q toggles only when the preview dialog owns the unmodified, non-repeating, non-composing, non-prevented key event. Command-Q and every modified Q remain untouched.
- The magnifier button has the stable accessible name `放大镜`, reports `aria-pressed`, and exposes Q through `aria-keyshortcuts="Q"` and its tooltip.
- Lens enabled state survives previous/next navigation inside one mounted preview and resets when the preview closes because the component unmounts. It is never persisted.
- Hide the normal cursor and show the lens only over actual transformed image pixels. Gray stage, toolbar, navigation, window exit, unsupported images, and unavailable images restore the pointer without changing the enabled flag.
- Lens detail always comes from the oriented original representation. Never enlarge the fit preview and label it original detail.
- Preserve the existing 700 MB / 100 MP decode budget. A budget failure is visible and contained; no bypass, second decoder, or unbounded canvas copy is allowed.
- Retain only one current original request and decoded current original. Abort or invalidate it on entity change, removal, or unmount. Pointer movement performs no backend calls.
- Magnifier preferences are bounded to shape `circle | rounded_rectangle`, magnification `3 | 4 | 5 | 6`, and area `small | medium | large` with defaults `circle`, `4`, and `small`.
- Exact lens geometry is fixed: circle 160/220/300 px and rounded rectangle 180×120/240×160/330×220 px for small/medium/large.
- Settings schema v1 migrates by preserving a valid thumbnail density and filling magnifier defaults. Schema v2 requires the exact bounded shape. Unknown, malformed, or future versions use complete defaults.
- Save a complete settings snapshot atomically. Serialized writes, optimistic UI, last-confirmed rollback, and latest-write-wins behavior must work across different fields without stale-field overwrite.
- Add no runtime dependency. Vendor the Lucide `zoom-in` asset with the existing asset script and retain its existing license record.
- Keep compare mode unchanged. Do not add magnifier support to `CompareWorkspace`.
- The frontend WebKit wheel implementation is the primary path. Do not add a native AppKit adapter unless the physical trackpad gate fails and the failure is documented; a fallback requires a separate reviewed implementation amendment.
- Follow test-driven development: each production change follows an observed focused-test failure for the missing behavior.
- Keep every production function below the repository architecture-health thresholds; use the planned module boundaries instead of growing `ImagePreview` into another mixed interaction subsystem.

---

### Task 1: Freeze the settings-v2 domain and migration contract

**Files:**
- Modify: `crates/viewer-application/src/settings.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-infrastructure/src/settings.rs`

**Interfaces:**
- Produces domain types `MagnifierShape`, `MagnifierMagnification`, `MagnifierArea`, `MagnifierPreferences`, and schema-v2 `ViewerSettings`.
- Replaces `ViewerSettingsService::update_thumbnail_density` with `ViewerSettingsService::update(settings)`.
- Persists exact v2 JSON:

```json
{
  "schemaVersion": 2,
  "thumbnailDensity": "standard",
  "magnifier": {
    "shape": "circle",
    "magnification": 4,
    "area": "small"
  }
}
```

- [x] **Step 1: Add failing application-domain tests for bounded defaults and complete saves**

In `crates/viewer-application/src/settings.rs`, replace the density-only service expectations with tests that assert:

```rust
assert_eq!(VIEWER_SETTINGS_SCHEMA_VERSION, 2);
assert_eq!(
    ViewerSettings::default(),
    ViewerSettings {
        thumbnail_density: ThumbnailDensity::Standard,
        magnifier: MagnifierPreferences {
            shape: MagnifierShape::Circle,
            magnification: MagnifierMagnification::Four,
            area: MagnifierArea::Small,
        },
    }
);
```

Add a table test proving that `MagnifierMagnification::try_from` accepts only 3, 4, 5, and 6 and that `u8::from(value)` round-trips each accepted value. Update the memory-port test so `service.update(expected)` saves exactly `expected`; retain the failing-store test and assert that it returns `ViewerSettingsError::Unavailable` without changing the last confirmed value.

- [x] **Step 2: Add failing infrastructure tests for v1 migration and exact v2 parsing**

In `crates/viewer-infrastructure/src/settings.rs`, write tests for all of these cases before production edits:

```rust
let migrated = load_json(serde_json::json!({
    "schemaVersion": 1,
    "thumbnailDensity": "extra_large"
}));
assert_eq!(migrated.thumbnail_density, ThumbnailDensity::ExtraLarge);
assert_eq!(migrated.magnifier, MagnifierPreferences::default());
```

```rust
let loaded = load_json(serde_json::json!({
    "schemaVersion": 2,
    "thumbnailDensity": "maximum",
    "magnifier": {
        "shape": "rounded_rectangle",
        "magnification": 6,
        "area": "large"
    }
}));
assert_eq!(loaded.thumbnail_density, ThumbnailDensity::Maximum);
assert_eq!(loaded.magnifier.shape, MagnifierShape::RoundedRectangle);
assert_eq!(loaded.magnifier.magnification, MagnifierMagnification::Six);
assert_eq!(loaded.magnifier.area, MagnifierArea::Large);
```

Also add one case for each invalid v2 magnification (`2`, `7`, and a string), invalid shape, invalid area, extra top-level field, extra nested field, malformed JSON, and future schema version `3`; every case must equal `ViewerSettings::default()`. Change the round-trip assertion from schema 1 to the exact schema-2 JSON shown in this task.

- [x] **Step 3: Run the focused Rust tests and observe the intended failures**

```bash
cargo test -p viewer-application settings
cargo test -p viewer-infrastructure settings
```

Expected: FAIL because schema version 1 and the density-only domain/store do not expose the magnifier contract or migrate v1.

- [x] **Step 4: Implement typed settings defaults and full-value service updates**

In `crates/viewer-application/src/settings.rs`, define:

```rust
pub const VIEWER_SETTINGS_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MagnifierShape {
    #[default]
    Circle,
    RoundedRectangle,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MagnifierArea {
    #[default]
    Small,
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MagnifierMagnification {
    Three,
    #[default]
    Four,
    Five,
    Six,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MagnifierPreferences {
    pub shape: MagnifierShape,
    pub magnification: MagnifierMagnification,
    pub area: MagnifierArea,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewerSettings {
    pub thumbnail_density: ThumbnailDensity,
    pub magnifier: MagnifierPreferences,
}
```

Implement `TryFrom<u8>` and `From<MagnifierMagnification> for u8` with the four exact values. Change the service method to:

```rust
pub fn update(&self, settings: ViewerSettings) -> Result<ViewerSettings, ViewerSettingsError> {
    self.store.save(settings)?;
    Ok(settings)
}
```

Re-export every new public type from `crates/viewer-application/src/lib.rs`.

- [x] **Step 5: Implement strict version-dispatched storage**

In `crates/viewer-infrastructure/src/settings.rs`, define separate `StoredViewerSettingsV1`, `StoredViewerSettingsV2`, and `StoredMagnifierPreferences` structs with `camelCase` plus `deny_unknown_fields`. Read JSON once as `serde_json::Value`, inspect only `schemaVersion`, then deserialize the complete matching struct. Convert v1 to `ViewerSettings { thumbnail_density, magnifier: MagnifierPreferences::default() }`; convert v2 through `MagnifierMagnification::try_from`; return defaults for every failed branch. Save only `StoredViewerSettingsV2` with `schemaVersion: 2` using the existing temporary-file, sync, and atomic-rename path.

Do not silently interpret v2 as v1 and do not partially preserve one field from malformed v2.

- [x] **Step 6: Run the focused Rust tests and confirm green**

```bash
cargo test -p viewer-application settings
cargo test -p viewer-infrastructure settings
```

Expected: both settings test groups pass, including v1 density preservation, exact v2 round-trip, invalid bounded-value fallback, and save failure.

- [x] **Step 7: Commit the domain and persistence boundary**

```bash
git add crates/viewer-application/src/settings.rs crates/viewer-application/src/lib.rs crates/viewer-infrastructure/src/settings.rs
git commit -m "feat: persist magnifier preferences"
```

---

### Task 2: Replace the field-specific Tauri command with complete settings updates

**Files:**
- Modify: `src-tauri/src/dto/settings.rs`
- Modify: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/error.rs`
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/acceptance/acceptanceBridge.ts`
- Modify: `ui/src/acceptance/acceptanceBridge.test.ts`
- Modify: `ui/src/acceptance/scenes/workspaceScenes.tsx`
- Modify: `ui/src/components/EmptyProject.test.tsx`
- Modify: `ui/src/state/useViewerController.test.tsx`
- Modify: `ui/src/settings/ViewerSettingsProvider.tsx`
- Modify: `ui/src/settings/ViewerSettingsProvider.test.tsx`

**Interfaces:**
- Produces TypeScript `ViewerSettingsUpdate` and `ViewerBridge.updateViewerSettings(settings)`.
- Produces Tauri command `update_viewer_settings(settings)` and removes the internal command `update_thumbnail_density`.
- Rejects invalid native magnification with code `invalid_viewer_settings`, category `validation`, and user message `设置值无效，请重新选择。`.

- [x] **Step 1: Add failing frozen-shape DTO tests**

In `src-tauri/src/dto/settings.rs`, assert the output DTO serializes to the exact v2 shape and the input DTO accepts the same object without `schemaVersion`. Add rejection tests for magnification 2 and 7, unknown shape/area strings, and unknown fields. The successful expected domain value is:

```rust
ViewerSettings {
    thumbnail_density: ThumbnailDensity::Compact,
    magnifier: MagnifierPreferences {
        shape: MagnifierShape::RoundedRectangle,
        magnification: MagnifierMagnification::Five,
        area: MagnifierArea::Medium,
    },
}
```

Add a command-level unit test that converts invalid magnification to `CommandError::new("invalid_viewer_settings", ErrorCategory::Validation, "设置值无效，请重新选择。", false)`.

- [x] **Step 2: Add failing TypeScript bridge contract tests**

In `ui/src/api/viewer.test.ts`, replace the density-only bridge test and assert:

```ts
await tauriViewerBridge.updateViewerSettings({
  thumbnailDensity: 'large',
  magnifier: { shape: 'rounded_rectangle', magnification: 5, area: 'medium' },
})

expect(invoke).toHaveBeenCalledWith('update_viewer_settings', {
  settings: {
    thumbnailDensity: 'large',
    magnifier: { shape: 'rounded_rectangle', magnification: 5, area: 'medium' },
  },
})
```

- [x] **Step 3: Run the focused contracts and observe failure**

```bash
cargo test -p viewer-desktop settings
pnpm --dir ui exec vitest run src/api
```

Expected: FAIL because the DTO and bridge still expose schema v1 and `update_thumbnail_density`.

- [x] **Step 4: Implement the v2 DTO boundary**

Add DTO enums for shape and area, `MagnifierPreferencesDto` for output, and this strict input:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewerSettingsUpdateDto {
    pub thumbnail_density: ThumbnailDensityDto,
    pub magnifier: MagnifierPreferencesUpdateDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MagnifierPreferencesUpdateDto {
    pub shape: MagnifierShapeDto,
    pub magnification: u8,
    pub area: MagnifierAreaDto,
}
```

Implement `TryFrom<ViewerSettingsUpdateDto> for ViewerSettings`. Convert the bounded number through the application-domain `TryFrom<u8>` implementation. Serialize output with numeric magnification and `schemaVersion: 2`.

- [x] **Step 5: Implement and register the complete update command**

In `src-tauri/src/commands/settings.rs`, replace the field command with:

```rust
#[tauri::command]
pub fn update_viewer_settings(
    settings: ViewerSettingsUpdateDto,
    service: State<'_, Arc<ViewerSettingsService>>,
) -> Result<ViewerSettingsDto, CommandError> {
    let settings = settings.try_into().map_err(|()| invalid_viewer_settings())?;
    service.update(settings).map(Into::into).map_err(Into::into)
}
```

Define `invalid_viewer_settings` in `src-tauri/src/error.rs` or locally using the exact error contract above. Register `commands::settings::update_viewer_settings` in `src-tauri/src/lib.rs` and remove the old registration.

- [x] **Step 6: Implement matching TypeScript public types and bridge**

In `ui/src/api/types.ts`, define:

```ts
export type MagnifierShape = 'circle' | 'rounded_rectangle'
export type MagnifierMagnification = 3 | 4 | 5 | 6
export type MagnifierArea = 'small' | 'medium' | 'large'

export interface MagnifierPreferences {
  shape: MagnifierShape
  magnification: MagnifierMagnification
  area: MagnifierArea
}

export interface ViewerSettingsUpdate {
  thumbnailDensity: ThumbnailDensity
  magnifier: MagnifierPreferences
}

export interface ViewerSettings extends ViewerSettingsUpdate {
  schemaVersion: 2
}
```

Replace `updateThumbnailDensity` in `ViewerBridge` with:

```ts
updateViewerSettings(settings: ViewerSettingsUpdate): Promise<ViewerSettings>
```

and invoke `update_viewer_settings` with `{ settings }`. Update all bridge fixtures to return schema 2 and complete defaults; do not keep a compatibility method in the internal API.

Mechanically update the provider and every listed bridge mock to expose `updateViewerSettings`. Until Task 3 adds editable magnifier fields, the density setter must send the complete currently loaded snapshot with only `thumbnailDensity` replaced. Keep the complete snapshot in a ref so the application compiles and a density write cannot erase migrated magnifier preferences.

- [x] **Step 7: Run the focused contracts and confirm green**

```bash
cargo test -p viewer-desktop settings
pnpm --dir ui exec vitest run src/api
```

- [x] **Step 8: Commit the transport boundary**

```bash
git add src-tauri/src/dto/settings.rs src-tauri/src/commands/settings.rs src-tauri/src/lib.rs src-tauri/src/error.rs ui/src/api/types.ts ui/src/api/viewer.ts ui/src/api/viewer.test.ts ui/src/App.test.tsx ui/src/acceptance/acceptanceBridge.ts ui/src/acceptance/acceptanceBridge.test.ts ui/src/acceptance/scenes/workspaceScenes.tsx ui/src/components/EmptyProject.test.tsx ui/src/state/useViewerController.test.tsx ui/src/settings/ViewerSettingsProvider.tsx ui/src/settings/ViewerSettingsProvider.test.tsx
git commit -m "refactor: save complete viewer settings"
```

---

### Task 3: Make optimistic settings updates safe across all fields

**Files:**
- Create: `ui/src/settings/viewerSettings.ts`
- Create: `ui/src/settings/viewerSettings.test.ts`
- Modify: `ui/src/settings/ViewerSettingsProvider.tsx`
- Modify: `ui/src/settings/ViewerSettingsProvider.test.tsx`

**Interfaces:**
- Produces `DEFAULT_VIEWER_SETTINGS_UPDATE` and typed choice lists.
- Extends `useViewerSettings()` with `magnifier`, `setMagnifierShape`, `setMagnifierMagnification`, and `setMagnifierArea`.
- Preserves `thumbnailDensity`, `thumbnailHeight`, `settingsError`, and `setThumbnailDensity`.

- [x] **Step 1: Add failing default and bounded-choice tests**

Assert the settings helper exports exactly:

```ts
expect(DEFAULT_VIEWER_SETTINGS_UPDATE).toEqual({
  thumbnailDensity: 'standard',
  magnifier: { shape: 'circle', magnification: 4, area: 'small' },
})
expect(MAGNIFIER_SHAPES).toEqual(['circle', 'rounded_rectangle'])
expect(MAGNIFIER_MAGNIFICATIONS).toEqual([3, 4, 5, 6])
expect(MAGNIFIER_AREAS).toEqual(['small', 'medium', 'large'])
```

- [x] **Step 2: Replace density-only provider fixtures and add multi-field queue tests**

Change the test helper to build schema-v2 settings. Extend the consumer with outputs for shape, magnification, and area plus buttons that call each setter. Add tests proving:

1. Initial load adopts all four persisted values.
2. Shape changes optimistically and sends the complete current settings snapshot.
3. A shape change followed immediately by magnification change serializes two writes; the second call contains the new shape and new magnification.
4. If the first write succeeds and the second fails, all fields roll back to the first saved complete snapshot.
5. A late initial load cannot overwrite any first user choice.
6. A new field change clears a prior save error.

The critical second call assertion is:

```ts
expect(updateViewerSettings).toHaveBeenNthCalledWith(2, {
  thumbnailDensity: 'standard',
  magnifier: { shape: 'rounded_rectangle', magnification: 6, area: 'small' },
})
```

- [x] **Step 3: Run provider tests and observe failure**

```bash
pnpm --dir ui exec vitest run src/settings/viewerSettings.test.ts src/settings/ViewerSettingsProvider.test.tsx
```

- [x] **Step 4: Implement one complete optimistic snapshot**

Store a `ViewerSettingsUpdate` object in React state and in `confirmedRef`, not separate confirmed fields. Use one `commit(next)` callback that clears the error, updates the optimistic object, increments the sequence, appends `bridge.updateViewerSettings(next)` to the existing serialized promise tail, adopts the returned complete snapshot on success, and restores `confirmedRef.current` only when the latest write fails.

Each field setter must derive from an optimistic ref outside the React state updater so Strict Mode cannot enqueue a write twice:

```ts
const updateOptimistically = useCallback(
  (recipe: (current: ViewerSettingsUpdate) => ViewerSettingsUpdate) => {
    const next = recipe(optimisticRef.current)
    optimisticRef.current = next
    setSettingsState(next)
    enqueue(next)
  },
  [enqueue],
)

const setMagnifierShape = useCallback((shape: MagnifierShape) => {
  updateOptimistically((current) => ({
    ...current,
    magnifier: { ...current.magnifier, shape },
  }))
}, [updateOptimistically])
```

Apply the same pattern to density, magnification, and area. Synchronize `optimisticRef` when a load, confirmed save, or rollback is adopted. Ensure `enqueue` does not call `setSettingsState`; keep initial-load suppression keyed by the first queued user sequence.

- [x] **Step 5: Run provider tests and confirm green**

```bash
pnpm --dir ui exec vitest run src/settings/viewerSettings.test.ts src/settings/ViewerSettingsProvider.test.tsx
```

- [x] **Step 6: Commit the complete optimistic provider**

```bash
git add ui/src/settings/viewerSettings.ts ui/src/settings/viewerSettings.test.ts ui/src/settings/ViewerSettingsProvider.tsx ui/src/settings/ViewerSettingsProvider.test.tsx
git commit -m "feat: expose magnifier settings"
```

---

### Task 4: Add bounded magnifier controls to the Settings dialog

**Files:**
- Modify: `ui/src/components/ui/ViewerChoiceChip.tsx`
- Create: `ui/src/components/ui/ViewerChoiceChip.test.tsx`
- Modify: `ui/src/components/SettingsDialog.tsx`
- Modify: `ui/src/components/SettingsDialog.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/App.tsx`

**Interfaces:**
- `ViewerChoiceChip` accepts `type="checkbox" | "radio"`, defaulting to `checkbox`.
- `SettingsDialog` receives `magnifier`, `onMagnifierShapeChange`, `onMagnifierMagnificationChange`, and `onMagnifierAreaChange` in addition to its existing props.

- [x] **Step 1: Add failing radio-chip semantics tests**

Render two `ViewerChoiceChip` controls with the same name and `type="radio"`. Assert `getAllByRole('radio')` returns two controls, the selected label has `data-checked="true"`, and changing the second control calls `onCheckedChange(true)`. Retain a checkbox test proving the default type does not regress.

- [x] **Step 2: Add failing Settings dialog choice tests**

Update every dialog render with default magnifier preferences. Assert the new `图片预览` section contains three named groups:

```ts
expect(within(dialog).getByRole('group', { name: '放大镜形状' })).toBeVisible()
expect(within(dialog).getByRole('group', { name: '放大倍数' })).toBeVisible()
expect(within(dialog).getByRole('group', { name: '显示面积' })).toBeVisible()
```

Within those groups assert radio labels `圆形`, `圆角矩形`, `3 倍`, `4 倍`, `5 倍`, `6 倍`, `小`, `中`, and `大`; defaults are circle, 4, and small. Click one option in each group and assert the three typed callbacks receive `rounded_rectangle`, `6`, and `large`. Keep the existing single-category, slider, focus-trap, footer, 1024×720, and 720×450 checks.

- [x] **Step 3: Add failing style contracts for small-window reachability**

In `ui/src/styles/app.test.ts`, assert `.settings-dialog-content` remains vertically scrollable, `.magnifier-setting-options` wraps, radio chips have visible focus, and the 720px breakpoint does not hide any of the new fieldsets. Add forced-colors assertions for checked state and focus outline.

- [x] **Step 4: Run focused UI tests and observe failure**

```bash
pnpm --dir ui exec vitest run src/components/ui/ViewerChoiceChip.test.tsx src/components/SettingsDialog.test.tsx src/styles/app.test.ts
```

- [x] **Step 5: Generalize `ViewerChoiceChip` without changing its default**

Change its omitted input props so `type` is accepted, destructure `type = 'checkbox'`, and render `<input type={type}>`. Preserve checked, disabled, class, and `onCheckedChange` behavior.

- [x] **Step 6: Render the three bounded radio groups**

Keep the single `显示与外观` navigation category. Below the thumbnail slider add a `图片预览` heading and three `fieldset` elements. Map the typed lists from `settings/viewerSettings.ts` to radio chips with stable names. The magnification value conversion must use the typed list lookup rather than an unchecked numeric cast.

Use these visible labels:

```ts
const SHAPE_LABEL = { circle: '圆形', rounded_rectangle: '圆角矩形' } as const
const AREA_LABEL = { small: '小', medium: '中', large: '大' } as const
```

Keep the existing save-error feedback once at the end of the content column.

- [x] **Step 7: Wire provider values through `App`**

Destructure all magnifier values/setters from `useViewerSettings()` and pass them only to the mounted `SettingsDialog` and later to `ImagePreview`. At this task, pass settings to the dialog and leave the preview wiring for Task 10.

- [x] **Step 8: Add responsive, focus, and forced-colors styles**

Use the existing Viewer chip/radius/token language. Make option rows wrap with a minimum 8px gap, keep legends visible, and avoid fixed dialog-content heights. At 720×450 the sheet body must scroll while the footer remains reachable.

- [x] **Step 9: Run focused tests and confirm green**

```bash
pnpm --dir ui exec vitest run src/components/ui/ViewerChoiceChip.test.tsx src/components/SettingsDialog.test.tsx src/styles/app.test.ts
```

- [x] **Step 10: Commit the real settings UI**

```bash
git add ui/src/components/ui/ViewerChoiceChip.tsx ui/src/components/ui/ViewerChoiceChip.test.tsx ui/src/components/SettingsDialog.tsx ui/src/components/SettingsDialog.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts ui/src/App.tsx
git commit -m "feat: configure magnifier preferences"
```

---

### Task 5: Build and prove the pure viewport and lens geometry

**Files:**
- Create: `ui/src/components/imagePreview/imageGeometry.ts`
- Create: `ui/src/components/imagePreview/imageGeometry.test.ts`
- Create: `ui/src/components/imagePreview/magnifierGeometry.ts`
- Create: `ui/src/components/imagePreview/magnifierGeometry.test.ts`

**Interfaces:**
- Produces pure stage/source mapping, rotation, pan clamping, and anchor-preserving zoom functions.
- Produces exact lens dimensions and CSS source placement.

- [x] **Step 1: Define the viewport geometry input and failing boundary tests**

Use these public types:

```ts
export type PreviewMode = 'fit' | 'original' | 'free'
export type PreviewRotation = 0 | 90 | 180 | 270
export interface Point { x: number; y: number }
export interface Size { width: number; height: number }
export interface ImageViewportState {
  mode: PreviewMode
  zoom: number
  rotation: PreviewRotation
  offset: Point
}
export interface ImageViewportGeometry {
  stage: Size
  source: Size
  fitInset: number
}
```

Write table tests for contain scale, rotated extents, and offsets at 0/90/180/270 degrees. Assert panning clamps to zero when the transformed image is smaller than the stage and to exact half-overflow when larger.

- [x] **Step 2: Add failing anchor-preserving zoom tests**

For a 1000×800 source in a 500×400 stage, prove that zooming around stage center leaves offset unchanged and zooming around `{ x: 400, y: 300 }` maps that stage point to the same source point before and after zoom. Add lower/upper clamp tests at 0.1 and 8, including a rotated case.

- [x] **Step 3: Add failing inverse mapping and hit-test tables**

For every rotation, map source center, all four source corners, one inside point, and one gray-stage point through source-to-stage and stage-to-source. Assert round-trip tolerance below `0.001`. Expose an unbounded `stagePointToSourcePoint` for zoom anchoring and a clipped `sourcePointAtStagePoint` for actual-pixel hit testing; the clipped function returns `null` outside actual image pixels, including rotated bounding-box corners.

Add a representation-remap test proving that `{ x: 600, y: 400 }` in a 1200×800 fit representation becomes `{ x: 3000, y: 2000 }` in a 6000×4000 original. This normalized mapping is the lens sample whenever the main view is showing a downsampled fit representation.

- [x] **Step 4: Add failing exact lens geometry tests**

Assert all six approved size/shape combinations exactly:

```ts
expect(lensDimensions('circle', 'small')).toEqual({ width: 160, height: 160 })
expect(lensDimensions('circle', 'medium')).toEqual({ width: 220, height: 220 })
expect(lensDimensions('circle', 'large')).toEqual({ width: 300, height: 300 })
expect(lensDimensions('rounded_rectangle', 'small')).toEqual({ width: 180, height: 120 })
expect(lensDimensions('rounded_rectangle', 'medium')).toEqual({ width: 240, height: 160 })
expect(lensDimensions('rounded_rectangle', 'large')).toEqual({ width: 330, height: 220 })
```

Test source placement so a sampled source coordinate remains at lens center for factors 3, 4, 5, and 6 under all rotations.

- [x] **Step 5: Run geometry tests and observe failure**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/magnifierGeometry.test.ts
```

- [x] **Step 6: Implement the pure matrix math**

Model the stage origin at its center. Compute fit scale as `min(1, stage.width * fitInset / source.width, stage.height * fitInset / source.height)`. Original mode uses scale 1; fit uses contain scale; free uses contain scale times `zoom`. Rotate the centered source vector, then add stage center and translation. Inverse mapping subtracts stage center/translation, applies inverse rotation, divides by scale, and adds source center. `stagePointToSourcePoint` returns that mathematical point even outside the image; `sourcePointAtStagePoint` bounds-checks it; `remapSourcePoint` converts by the fit/original width and height ratios.

`zoomAtAnchor` must first sample the unbounded source point under the anchor using the old state, switch to free mode with clamped zoom, calculate the new translation that puts that source point back at the anchor, then clamp translation. Keep functions total for zero-sized stage/source inputs by returning safe zero bounds and `null` hit-test mappings.

- [x] **Step 7: Implement lens geometry as data, not CSS guesses**

Expose `lensDimensions(shape, area)` from a frozen typed record and `magnifierSourcePlacement(sourcePoint, dimensions, magnification)` that returns `left`, `top`, `transformOriginX`, and `transformOriginY`. The original image is positioned so the sampled point is lens center; CSS rotation/scale happens around that sampled point.

- [x] **Step 8: Run geometry tests and confirm green**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/magnifierGeometry.test.ts
```

- [x] **Step 9: Commit pure geometry**

```bash
git add ui/src/components/imagePreview/imageGeometry.ts ui/src/components/imagePreview/imageGeometry.test.ts ui/src/components/imagePreview/magnifierGeometry.ts ui/src/components/imagePreview/magnifierGeometry.test.ts
git commit -m "feat: model preview and magnifier geometry"
```

---

### Task 6: Unify viewport state and trackpad gesture input

**Files:**
- Create: `ui/src/components/imagePreview/useImageViewport.ts`
- Create: `ui/src/components/imagePreview/useImageViewport.test.tsx`
- Create: `ui/src/components/imagePreview/usePreviewGestures.ts`
- Create: `ui/src/components/imagePreview/usePreviewGestures.test.tsx`

**Interfaces:**
- `useImageViewport` owns `mode`, `zoom`, `rotation`, `offset`, current measurements, and all viewport actions.
- `usePreviewGestures` attaches one non-passive wheel listener and maps pinch, two-axis pan, and pointer drag into viewport actions.

- [x] **Step 1: Add failing hook tests for one viewport model**

Use a small test harness with measured stage/source sizes. Prove:

- `setFit()` returns mode fit, zoom 1, and zero/clamped offset.
- `setOriginal()` uses original scale and clamps offset.
- `zoomBy(factor, anchor)` switches to free and preserves the anchor.
- `panBy(delta)` and pointer drag use the same bounded offset action.
- `rotateClockwise()` follows 0→90→180→270→0 and reclamps.
- `setMeasurements()` reclamps after resize or representation change.
- `resetForEntity()` resets fit/zoom/rotation/offset but does not own magnifier enabled state.

- [x] **Step 2: Add failing wheel normalization tests**

Export and test `normalizeWheelDelta(event, pageSize)` for pixel, line, and page delta modes. In a DOM harness, dispatch cancelable wheel events and assert:

```ts
stage.dispatchEvent(new WheelEvent('wheel', {
  bubbles: true,
  cancelable: true,
  ctrlKey: true,
  clientX: 320,
  clientY: 180,
  deltaY: -20,
  deltaMode: WheelEvent.DOM_DELTA_PIXEL,
}))
```

The pinch event calls `zoomBy` with a factor derived from `Math.exp(-deltaY * 0.002)` and a stage-local anchor. An ordinary event with `deltaX: 12, deltaY: -18` calls `panBy({ x: -12, y: 18 })`. Both owned events are default-prevented; events on toolbar/outside stage are untouched.

- [x] **Step 3: Add failing animation-frame coalescing tests**

Stub `requestAnimationFrame`. Dispatch several wheel events before flushing one frame and assert only one viewport update occurs with accumulated pan deltas or the latest composed pinch intent. Unmount before flush and assert the frame is canceled and the listener removed.

- [x] **Step 4: Run focused hook tests and observe failure**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/useImageViewport.test.tsx src/components/imagePreview/usePreviewGestures.test.tsx
```

- [x] **Step 5: Implement `useImageViewport` over pure geometry**

Use functional state updates so wheel, pointer, and toolbar calls cannot overwrite each other. Return stable callbacks for `setFit`, `setOriginal`, `zoomBy`, `panBy`, `rotateClockwise`, `setMeasurements`, and `resetForEntity`, plus a CSS transform derived from the state. Do not retain DOM nodes in this hook.

- [x] **Step 6: Implement non-passive trackpad ownership**

Use `stage.addEventListener('wheel', onWheel, { passive: false })` in an effect. Ignore non-cancelable events and disabled stages. For owned events call `preventDefault`, normalize delta mode, accumulate work, and flush at most once per animation frame. Clamp in the viewport hook, never in the event adapter. Remove the exact listener options and cancel pending frames on cleanup.

For pointer drag, ignore non-primary buttons, set pointer capture only when pan bounds are non-zero, and release/cancel drag on pointer up, pointer cancel, or lost capture.

- [x] **Step 7: Run focused hook tests and confirm green**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/useImageViewport.test.tsx src/components/imagePreview/usePreviewGestures.test.tsx
```

- [x] **Step 8: Commit viewport and gesture hooks**

```bash
git add ui/src/components/imagePreview/useImageViewport.ts ui/src/components/imagePreview/useImageViewport.test.tsx ui/src/components/imagePreview/usePreviewGestures.ts ui/src/components/imagePreview/usePreviewGestures.test.tsx
git commit -m "feat: add trackpad preview gestures"
```

---

### Task 7: Own one abortable current-original representation

**Files:**
- Create: `ui/src/components/imagePreview/useCurrentOriginal.ts`
- Create: `ui/src/components/imagePreview/useCurrentOriginal.test.tsx`
- Reference only: `ui/src/components/ComparePane.tsx`
- Reference only: `ui/src/api/viewer.ts`

**Interfaces:**
- Produces `CurrentOriginalState` with status `idle | loading | ready | budget_error | error`, representation, and entity identity.
- Accepts the existing signal-aware image request callback and `needed` boolean.

- [x] **Step 1: Add failing lifecycle tests**

Use deferred requests to prove:

1. `needed=false` sends no request.
2. The first `needed=true` sends one `original100_percent` request with an `AbortSignal`.
3. Re-rendering the same entity while loading or ready does not duplicate the request.
4. Changing entity aborts the old signal, clears its decoded representation, and requests the new entity once.
5. A stale old completion cannot replace the new entity.
6. Unmount and `available=false` abort the current request.
7. Error code `image_budget_exceeded` maps to `budget_error`; other errors map to `error`; AbortError is silent.
8. A new entity clears the previous failure and retries normally.

- [x] **Step 2: Run the hook test and observe failure**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/useCurrentOriginal.test.tsx
```

- [x] **Step 3: Implement one request identity and abort controller**

Mirror the proven cancellation pattern in `ComparePane`, but retain only one current value. The effect dependencies are current `entityId`, `needed`, `available`, and the stable request callback. Create a new `AbortController`, set loading synchronously for the current identity, pass `controller.signal`, reject stale completions by identity, and abort on cleanup.

Do not cache decoded originals in a frontend map. Backend artifact reuse remains available through the existing image cache.

- [x] **Step 4: Run the hook test and confirm green**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/useCurrentOriginal.test.tsx
```

- [x] **Step 5: Commit current-original ownership**

```bash
git add ui/src/components/imagePreview/useCurrentOriginal.ts ui/src/components/imagePreview/useCurrentOriginal.test.tsx
git commit -m "feat: own current magnifier original"
```

---

### Task 8: Render the original-detail lens without pointer-driven React renders

**Files:**
- Create: `ui/src/components/imagePreview/ImageMagnifier.tsx`
- Create: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- `ImageMagnifier` is a `forwardRef` component exposing `place(sample)` and `hide()`.
- Pointer placement writes CSS variables through one queued animation frame; component props change only for settings, rotation, load status, and source identity.

- [x] **Step 1: Add failing semantic and geometry component tests**

Render the component for every shape/area combination and assert `data-shape`, width, and height match the approved table. For ready state, assert the lens contains a duplicate `<img>` using the original URL and the supplied filename in a non-announced decorative alt strategy. For loading, budget, and generic error, assert the compact text is respectively `正在载入原图`, `原图超出安全预览限制`, and `无法载入原图`.

- [x] **Step 2: Add failing imperative-frame tests**

Call `handle.place` several times before one frame flush. Assert only the latest sample is applied and the lens CSS variables become:

```ts
expect(lens).toHaveStyle({
  '--magnifier-x': '240px',
  '--magnifier-y': '180px',
  '--magnifier-source-left': '-1520px',
  '--magnifier-source-top': '-1060px',
  '--magnifier-scale': '4',
  '--magnifier-rotation': '90deg',
})
```

Use sample data that mathematically yields those values. Assert `hide()` clears the visible data attribute immediately and cancels pending placement; unmount cancels a queued frame.

- [x] **Step 3: Add failing style contracts**

Assert the lens is absolutely positioned, centered on its CSS x/y variables, clipped, pointer-events none, and bounded by the stage overflow. Circle uses 50% radius; rounded rectangle uses the existing Viewer large radius token. Assert the source image uses max-width/max-height none and transform origin variables. Add forced-colors boundary/text rules and reduced-motion rules with no decorative transition.

- [x] **Step 4: Run focused magnifier tests and observe failure**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/ImageMagnifier.test.tsx src/styles/app.test.ts
```

- [x] **Step 5: Implement the imperative lens component**

Define:

```ts
export interface MagnifierPlacement {
  stagePoint: Point
  sourcePoint: Point
}

export interface ImageMagnifierHandle {
  place(placement: MagnifierPlacement): void
  hide(): void
}
```

Use `useImperativeHandle`, refs for the lens/source nodes, a latest-placement ref, and one `requestAnimationFrame`. Set CSS custom properties directly from `magnifierSourcePlacement`. Render a source `<img draggable={false}>` only in ready state. Keep the lens shell mounted while enabled so loading/failure follows the pointer; use `aria-hidden="true"` on the moving visual and let the preview own live announcements/contained feedback.

- [x] **Step 6: Implement visual styles and accessibility media rules**

Use a clear border, existing surface/shadow tokens, clipped overflow, and compact centered status text. Set `.image-preview-stage { overflow: hidden; }` only after confirming existing transformed image behavior remains correct. Hide the cursor through a separate `.image-preview-stage[data-magnifier-over-image="true"]` rule so leaving actual pixels restores it immediately.

- [x] **Step 7: Run focused magnifier tests and confirm green**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/ImageMagnifier.test.tsx src/styles/app.test.ts
```

- [x] **Step 8: Commit the lens renderer**

```bash
git add ui/src/components/imagePreview/ImageMagnifier.tsx ui/src/components/imagePreview/ImageMagnifier.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: render original detail magnifier"
```

---

### Task 9: Vendor and register the real toolbar icon

**Files:**
- Modify: `scripts/vendor-viewer-icons.mjs`
- Create: `ui/src/assets/icons/lucide/zoom-in.svg` via the vendor script
- Modify: `ui/src/components/ui/ViewerIcon.tsx`
- Modify: `ui/src/components/ui/ViewerIcon.test.tsx`

**Interfaces:**
- Adds Viewer icon name `zoom-in` from the already pinned Lucide 1.27.0 source.

- [x] **Step 1: Add the failing icon registry assertion**

Extend the icon test so `VIEWER_ICON_NAMES` contains `zoom-in` exactly once and `<ViewerIcon name="zoom-in" />` resolves a no-inline SVG asset with empty alt and `aria-hidden="true"`.

- [x] **Step 2: Run the icon test and observe failure**

```bash
pnpm --dir ui exec vitest run src/components/ui/ViewerIcon.test.tsx
```

- [x] **Step 3: Vendor the pinned asset and register its typed name**

Add `'zoom-in'` to `sources` in `scripts/vendor-viewer-icons.mjs`, run:

```bash
node scripts/vendor-viewer-icons.mjs
```

Confirm only the expected pinned SVG set and existing `LICENSE.txt` content changed. Add `'zoom-in'` to `VIEWER_ICON_NAMES` in alphabetical position.

- [x] **Step 4: Run the icon test and confirm green**

```bash
pnpm --dir ui exec vitest run src/components/ui/ViewerIcon.test.tsx
```

- [ ] **Step 5: Commit asset provenance and registration**

```bash
git add scripts/vendor-viewer-icons.mjs ui/src/assets/icons/lucide/zoom-in.svg ui/src/assets/icons/lucide/LICENSE.txt ui/src/components/ui/ViewerIcon.tsx ui/src/components/ui/ViewerIcon.test.tsx
git commit -m "assets: add magnifier toolbar icon"
```

---

### Task 10: Integrate gestures, original ownership, Q, and the toolbar button in `ImagePreview`

**Files:**
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Extends `ImagePreviewProps.requestImage` with optional `AbortSignal`.
- Adds required prop `magnifier: MagnifierPreferences`.
- Preserves every existing preview prop and formal behavior.

- [ ] **Step 1: Update existing fixtures and add failing button/Q state tests**

Pass default magnifier preferences to every `ImagePreview` test render. Assert the toolbar contains button `放大镜` beside the zoom controls with `aria-pressed="false"`, `aria-keyshortcuts="Q"`, and tooltip containing Q. Test click and unmodified Q toggle the same flag and live announcements `放大镜已开启` / `放大镜已关闭`.

Add a table proving Q is ignored for `metaKey`, `ctrlKey`, `altKey`, `shiftKey`, `repeat`, `isComposing`, and `defaultPrevented`. Assert unsupported/unavailable images disable the button and ignore Q. Retain Escape and arrow navigation tests.

- [ ] **Step 2: Add failing pointer entry/exit and navigation lifetime tests**

Mock stage measurements and an image representation. Enable the magnifier, move over a mapped image pixel, and assert stage data says the pointer is over image, cursor-hide styling applies, and lens is visible. Move to gray stage, fire `pointerleave`, and move over toolbar; each hides the lens/restores cursor while the button stays pressed. Re-enter actual pixels and assert the lens returns.

Rerender with the next file and assert `aria-pressed` remains true, the old original signal is aborted, and a new original is requested. Unmount and render a new `ImagePreview`; assert the new session starts false.

- [ ] **Step 3: Add failing original loading/failure/recovery tests**

Prove enabling sends exactly one original request and keeps the fit image visible. While pending, the lens follows the pointer with `正在载入原图`. For `image_budget_exceeded`, assert the main fit remains, the lens says `原图超出安全预览限制`, and one contained local stage explanation is rendered. For generic failure, use `无法载入原图`. Navigate next and resolve successfully; assert failure clears and the new original appears. Clicking 100% while magnifier original is ready must reuse that same representation without a second request.

- [ ] **Step 4: Add failing unified viewport integration tests**

Assert toolbar plus/minus, pointer drag, pinch wheel, and ordinary two-axis wheel change the one image transform. Check center anchoring and boundary clamps through the rendered style/data attributes. Assert a two-finger pan never calls `onNavigate`, while ArrowLeft/ArrowRight still do.

- [ ] **Step 5: Run focused preview tests and observe failure**

```bash
pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx src/styles/app.test.ts
```

- [ ] **Step 6: Replace local transform state with `useImageViewport` and `usePreviewGestures`**

Remove `panBounds`, `clamp`, `dragStart`, and the separate mode/zoom/rotation/offset state from `ImagePreview.tsx`. Measure the stage with `ResizeObserver` and the current display-representation dimensions, pass measurements to the viewport hook, and render its transform. Give the main image explicit natural representation width/height, remove CSS max-size scaling from that element, and let the geometry-derived transform provide the single fit/original/free scale so CSS does not apply contain scaling twice. Connect stage ref/pointer handlers/non-passive wheel behavior through `usePreviewGestures`.

Free mode continues to render the fit representation. Original mode renders the current original. Fit stays visible while the original is loading.

- [ ] **Step 7: Add session magnifier state and strict keyboard ownership**

Add one `useState(false)` at `ImagePreview` component lifetime and do not reset it in the entity-change effect. Toggle with:

```ts
const ownsMagnifierShortcut =
  event.key.toLowerCase() === 'q' &&
  !event.metaKey &&
  !event.ctrlKey &&
  !event.altKey &&
  !event.shiftKey &&
  !event.repeat &&
  !event.isComposing &&
  !event.defaultPrevented &&
  !transformsDisabled
```

When owned, prevent default, toggle once, and announce the new state through a polite live region. Render a `ViewerIconButton` with `icon="zoom-in"`, stable `label="放大镜"`, `active={magnifierEnabled}`, `aria-keyshortcuts="Q"`, and `title="放大镜（Q）"` beside zoom controls.

- [ ] **Step 8: Share the one current original between 100% and magnifier**

Set `needed = mode === 'original' || magnifierEnabled` and call `useCurrentOriginal`. If original mode fails, return the main view to fit and retain the existing explanatory suffix. If only the magnifier needs it, do not change main mode. Pass loading/error/ready state into `ImageMagnifier`.

- [ ] **Step 9: Map pointer samples and update the lens imperatively**

On stage pointer move, convert client coordinates to stage coordinates and call `sourcePointAtStagePoint` against the current display representation. If the main view is fit/free, remap that coordinate by the display/original dimension ratio before placing the lens; original mode uses the coordinate directly. Place only if enabled, available, and the mapping is non-null. Update a stage data attribute for cursor hiding. When mapping returns null or pointer leaves/cancels, call `hide()` and clear only pointer-over-image state. Rotation is passed to the lens so the duplicate original matches the main orientation.

- [ ] **Step 10: Wire preferences and signal-aware request from `App`**

Pass the provider's `magnifier` to both mounted `ImagePreview` locations in `App.tsx`. The existing `requestPreviewImage(file, representation, signal?)` already delegates the abort signal to the bridge; update only the `ImagePreviewProps` type and tests, not the bridge cancellation protocol.

- [ ] **Step 11: Finish responsive stage/toolbar behavior**

Keep the magnifier button in the central display-control group without hiding the navigation. At 720×450 and 200% zoom, controls may wrap/compact according to existing toolbar rules but remain keyboard reachable. Ensure lens overflow cannot create page scrollbars and the cursor-hide selector applies only to the stage's actual-image state.

- [ ] **Step 12: Run focused preview tests and confirm green**

```bash
pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx src/components/imagePreview src/styles/app.test.ts
```

- [ ] **Step 13: Commit the integrated preview feature**

```bash
git add ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx ui/src/App.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: add trackpad zoom and preview magnifier"
```

---

### Task 11: Formalize product and automated acceptance coverage

**Files:**
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx`
- Modify: `ui/src/acceptance/scenes/dialogScenes.tsx`
- Modify: `ui/src/acceptance/scenes/dialogScenes.test.tsx`
- Modify: `scripts/viewer-native-acceptance.mjs`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Reference only: `scripts/viewer-native-acceptance.swift`

**Interfaces:**
- Adds formal visual state `PRE-08` named `preview-magnifier`.
- Extends `DIA-01` to the real settings-v2 controls.
- Keeps A11Y-05 at 200% page zoom and includes the new UI automatically.

- [ ] **Step 1: Add the approved behavior to the product specification**

Under `[REQ-IA-IMAGE-PREVIEW]`, add bullets for Q/button toggle parity, actual-image entry/exit behavior, original-detail lens semantics, session lifetime, and bounded trackpad pan. Add a settings bullet with the exact shape, magnification, area values and defaults. Under the safety section, state that magnifier original loading uses the same 700 MB / 100 MP bound and never substitutes a fit proxy after budget failure.

- [ ] **Step 2: Add failing scene-ledger and scene tests**

Insert after PRE-07:

```json
{ "id": "PRE-08", "wave": 2, "referenceState": "preview-magnifier", "sceneGroup": "viewing", "components": ["ImagePreview", "ImageMagnifier"] }
```

Extend `PreviewState` with `magnifier`, activate it by clicking the real `放大镜` button, move a synthetic pointer to a deterministic actual-image point after the fit and original images are ready, and pass default preferences. The scene-ready predicate must require a ready lens with `data-shape="circle"`, 160×160 geometry, factor 4, and the real original URL.

Update the viewing-scene test to assert PRE-08 renders the pressed real toolbar button and loaded lens. Update DIA-01 scene/test with all magnifier settings props and assert the default selected radio values.

- [ ] **Step 3: Run scene tests and observe failure**

```bash
pnpm --dir ui exec vitest run src/acceptance/scenes/viewingScenes.test.tsx src/acceptance/scenes/dialogScenes.test.tsx
```

- [ ] **Step 4: Implement the deterministic PRE-08 scene and DIA-01 extension**

Use `requestAcceptanceImage` for both fit and original. Do not special-case production code for acceptance. The scene may call real buttons and dispatch pointer movement in its existing one-time effect after dimensions are measurable. Preserve catalog ordering so `Object.keys(VIEWING_SCENES)` exactly matches the ledger.

- [ ] **Step 5: Extend native acceptance planning for keyboard/button parity**

Change the native audit list from `numberedIds('PRE', 7)` to `numberedIds('PRE', 8)`. Add a PRE-08 native plan that opens a real image, presses Q, asserts an `AXStaticText` named `放大镜已开启`, clicks the stable AX button `放大镜`, and asserts an `AXStaticText` named `放大镜已关闭`. This proves Q/button parity without depending on WebKit exposing `aria-pressed` as an AX value.

Update `scripts/viewer-native-acceptance.test.mjs` to freeze the exact new step sequence. Do not claim that this keyboard automation verifies physical pinch behavior.

- [ ] **Step 6: Run automated acceptance contracts**

```bash
pnpm --dir ui exec vitest run src/acceptance/scenes/viewingScenes.test.tsx src/acceptance/scenes/dialogScenes.test.tsx
pnpm test:native-acceptance
pnpm test:visual-acceptance
pnpm build:visual-acceptance
```

- [ ] **Step 7: Capture the required visual states**

Run the formal visual acceptance for PRE-08, DIA-01, and A11Y-05 at the catalog's required viewports. Inspect the lossless product images for default circle/4x/small geometry, toolbar fit, settings reachability, lens clipping, and 200% zoom. Store evidence only through the existing acceptance harness; do not commit generated `target` output.

```bash
pnpm accept:visual -- --id PRE-08 --id DIA-01 --id A11Y-05
```

- [ ] **Step 8: Run native PRE-08 acceptance in the packaged development build**

```bash
pnpm accept:native -- --id PRE-08 --viewport 1024x720
pnpm accept:native -- --id PRE-08 --viewport 1440x900
```

Confirm Q and the toolbar button operate the same pressed state and the main preview remains usable.

- [ ] **Step 9: Commit formal product and automated acceptance coverage**

```bash
git add docs/PRODUCT_SPEC.md ui/src/acceptance/acceptanceStateCatalog.json ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx ui/src/acceptance/scenes/dialogScenes.tsx ui/src/acceptance/scenes/dialogScenes.test.tsx scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs
git commit -m "test: cover preview magnifier acceptance"
```

---

### Task 12: Pass the physical MacBook trackpad release gate

**Files:**
- Create after actual execution: `docs/reviews/2026-08-06-viewer-trackpad-magnifier-acceptance.md`
- Conditional only after documented frontend failure: a separate approved AppKit fallback amendment and implementation plan

**Interfaces:**
- Produces human-reviewed device evidence; it does not add an automated claim that synthetic wheel events equal a physical trackpad.

- [ ] **Step 1: Launch the latest development build with one instance**

Use the repository launcher so the tested app is the current source build:

```bash
pnpm start:viewer
```

Open a project containing a normal image and a source that triggers the original budget error. Record macOS version, Mac model, Viewer commit hash, display scale, and whether an external pointing device was connected.

- [ ] **Step 2: Execute the eight approved hardware checks**

On the built-in MacBook trackpad verify and record pass/fail evidence for:

1. Pinch in/out preserves the point between the fingers.
2. Horizontal, vertical, and diagonal two-finger pan all move the image naturally.
3. Every edge stops hard and no gesture changes the current image.
4. Q and the toolbar button toggle the same state.
5. Actual-image exit restores the pointer; re-entry restores the still-enabled lens.
6. Previous/next navigation preserves enabled state and shows only the new image detail.
7. Circle and rounded rectangle render at small/medium/large and 3x/4x/5x/6x.
8. Fast pointer movement plus rapid navigation shows no stale original, cursor desynchronization, or unbounded lag.

Repeat the default state at 720×450 and repeat keyboard reachability at 200% page zoom. Verify budget failure shows the approved compact lens state and leaves the fit preview functional.

- [ ] **Step 3: Make the native-fallback decision explicit**

If all physical gesture checks pass, record `Frontend WebKit wheel path accepted; AppKit fallback not triggered.` in the review file.

If pinch does not arrive as owned non-passive `wheel + ctrlKey`, anchor coordinates are unreliable, or diagonal pan is lost in the packaged WKWebView, stop completion. Record the exact event evidence and failed check. Do not improvise native code in this task. Write and obtain approval for a focused fallback amendment that normalizes AppKit magnification into the existing `useImageViewport` actions without enabling whole-page WebView magnification or adding a second state model.

- [ ] **Step 4: Commit the completed hardware review**

Only after every required check passes:

```bash
git add docs/reviews/2026-08-06-viewer-trackpad-magnifier-acceptance.md
git commit -m "docs: record trackpad magnifier acceptance"
```

---

### Task 13: Run the complete regression, quality, security, and cleanliness gates

**Files:**
- Modify only if measurements legitimately change: `docs/quality/architecture-health-baseline.json`
- Modify only if deliberate tested coverage changes require it: `docs/quality/ui-coverage-baseline.json`
- Modify only if deliberate tested coverage changes require it: `docs/quality/rust-coverage-baseline.json`

- [ ] **Step 1: Run formatting without touching unrelated files**

```bash
pnpm --dir ui exec biome format --write src/api/types.ts src/api/viewer.ts src/settings src/components/ImagePreview.tsx src/components/ImagePreview.test.tsx src/components/imagePreview src/components/SettingsDialog.tsx src/components/SettingsDialog.test.tsx src/components/ui/ViewerChoiceChip.tsx src/components/ui/ViewerChoiceChip.test.tsx src/components/ui/ViewerIcon.tsx src/components/ui/ViewerIcon.test.tsx src/acceptance/scenes/viewingScenes.tsx src/acceptance/scenes/viewingScenes.test.tsx src/acceptance/scenes/dialogScenes.tsx src/acceptance/scenes/dialogScenes.test.tsx src/App.tsx src/styles/app.test.ts
cargo fmt --all
```

- [ ] **Step 2: Run all UI and Rust tests plus builds**

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

- [ ] **Step 3: Run repository, architecture, and security gates**

```bash
pnpm test:policy
pnpm architecture:health
pnpm dependencies:exceptions
pnpm security
```

If architecture health changes solely because the feature adds well-tested production/test lines, inspect the generated report and update the baseline with the script's supported write mode. Never relax file-size or decision-score outlier rules to make the gate pass.

The exact architecture update command, when justified, is:

```bash
node scripts/architecture-health.mjs --update docs/quality/architecture-health-baseline.json
```

- [ ] **Step 4: Run coverage and formal acceptance regression**

```bash
pnpm coverage:ui
pnpm coverage:rust
pnpm test:visual-acceptance
pnpm test:native-acceptance
pnpm build:visual-acceptance
```

Regenerate a coverage baseline only when the measured result is understood and at least as strong as repository policy permits. Do not hide newly uncovered gesture, settings migration, request cancellation, or failure branches.

The exact update commands, when justified, are:

```bash
node scripts/coverage-baseline.mjs --update target/coverage/ui/coverage-summary.json docs/quality/ui-coverage-baseline.json
node scripts/rust-coverage-baseline.mjs --update target/coverage/rust.json docs/quality/rust-coverage-baseline.json
```

- [ ] **Step 5: Inspect final scope and cleanliness**

```bash
git status --short
git diff --check
git log --oneline --decorate -12
```

Confirm there are no generated acceptance images, `.superpowers` artifacts, temporary settings files, compiled binaries, unrelated user changes, or uncommitted source edits. If the worktree contained user changes before execution, preserve and report them rather than cleaning them destructively.

- [ ] **Step 6: Run the repository's complete verification command**

```bash
pnpm verify
```

Expected: policy, clean-check prerequisites, UI checks/tests/build, Rust format/clippy/tests, Tauri security, dependency policy, and license checks all pass.

- [ ] **Step 7: Commit legitimate final gate metadata only if needed**

```bash
git add docs/quality/architecture-health-baseline.json docs/quality/ui-coverage-baseline.json docs/quality/rust-coverage-baseline.json
git commit -m "chore: refresh verified quality baselines"
```

Skip this commit when no baseline file legitimately changed. End with a clean worktree and report the physical trackpad result separately from synthetic automated coverage.
