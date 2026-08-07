# Viewer Stable Progressive Preview and Larger Magnifier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the small-proxy-to-large-original preview jump, replace magnifier choices with 2×/3×/4× at a 2× default, and enlarge all six fixed lens geometries.

**Architecture:** Keep the existing one-viewport progressive pipeline, but allow fitted representations to upscale and make the image element consume one committed source-geometry/transform pair. Upgrade viewer settings to schema 4 across Rust, Tauri, and React, migrating schema-3 magnification while preserving other preferences. Keep lens sizing in the existing pure geometry module and update acceptance fixtures to the same public values.

**Tech Stack:** React 19, TypeScript 6, Vitest 4, Testing Library, Tauri 2, Rust, serde/serde_json, Playwright visual acceptance, macOS development launcher.

## Global Constraints

- “适应窗口” remains the only `100%` baseline; percentage never means one source pixel per CSS pixel.
- A fit proxy may upscale to the approved fit inset; the original may downscale. Representation replacement must not visibly change bounds when aspect ratio is unchanged.
- The current original still replaces the proxy without crossfade, second full-resolution request, or an additional decoded copy.
- Original decoding remains behind the existing `100,000,000` pixel and `700 MB` safety budgets.
- Public magnifier choices are exactly `2`, `3`, and `4`; default magnification is `2`.
- Settings schema version is `4`; schema-3 `1.5` migrates to `2`, while schema-3 `2` and `3` remain unchanged. Thumbnail density, shape, and area are preserved.
- Lens sizes are exactly circle `200/280/380` CSS px and rounded rectangle `230×150/300×200/420×280` CSS px for small/medium/large.
- Q/button parity, visible pointer, pointer-adjacent placement, edge flipping, enabled-state memory, animation timing, and trackpad sensitivity do not change.
- The stage retains overflow clipping; larger lenses must not create page scrolling or cover the toolbar.
- Do not add dependencies or bypass the image protocol, CSP, cache, request cancellation, or path-isolation boundaries.

---

## File Responsibility Map

- `ui/src/components/imagePreview/imageGeometry.ts`: pure fit/free source-to-stage scale; fitting may upscale.
- `ui/src/components/imagePreview/useImageViewport.ts`: owns the one committed source/stage geometry and transform.
- `ui/src/components/ImagePreview.tsx`: chooses fit/original URLs and renders width, height, and transform from the same committed viewport geometry.
- `crates/viewer-application/src/settings.rs`: schema-4 constant, bounded 2/3/4 domain enum, and 2× default.
- `crates/viewer-infrastructure/src/settings.rs`: exact schema-4 persistence and schema-3 migration.
- `src-tauri/src/dto/settings.rs`: public camelCase schema-4 DTO and input validation.
- `ui/src/api/types.ts` and `ui/src/settings/viewerSettings.ts`: frontend public type, choices, and defaults.
- `ui/src/settings/ViewerSettingsProvider.tsx` and `ui/src/components/SettingsDialog.tsx`: unchanged persistence orchestration consuming the revised public values.
- `ui/src/components/imagePreview/magnifierGeometry.ts`: approved six fixed lens dimensions and existing placement math.
- `ui/src/acceptance/scenes/*.tsx`: deterministic settings and preview scenes aligned with schema 4 and 2× default.

---

### Task 1: Make Progressive Preview Geometry Stable

**Files:**
- Modify: `ui/src/components/imagePreview/imageGeometry.ts`
- Modify: `ui/src/components/imagePreview/imageGeometry.test.ts`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`

**Interfaces:**
- Preserves: `displayScale(state: ImageViewportState, geometry: ImageViewportGeometry): number`.
- Preserves: `ImageViewport.setMeasurements(stage: Size, source: Size): void`.
- Produces: fitted scale `min(stage.width * fitInset / rotatedSourceWidth, stage.height * fitInset / rotatedSourceHeight)` without a `1` cap.
- Produces: `<img width height style.transform>` from `viewport.geometry.source` and `viewport.transform` as one committed pair.

- [ ] **Step 1: Add a failing pure test for upscaled fit proxies**

Add this case to `imageGeometry.test.ts`:

```ts
it('fills the fit inset even when the current representation is smaller than the stage', () => {
  const proxyGeometry: ImageViewportGeometry = {
    stage: { width: 1920, height: 1000 },
    source: { width: 560, height: 373 },
    fitInset: 0.9,
  }

  const scale = displayScale(state(), proxyGeometry)

  expect(scale).toBeCloseTo(900 / 373)
  expect(proxyGeometry.source.height * scale).toBeCloseTo(900)
  expect(proxyGeometry.source.width * scale).toBeGreaterThan(1300)
})
```

- [ ] **Step 2: Run the pure test and verify the root-cause failure**

Run:

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/imageGeometry.test.ts
```

Expected: FAIL because `displayScale` returns `1`, rendering the proxy at `560 × 373` instead of the fitted inset.

- [ ] **Step 3: Remove only the one-to-one upscaling cap**

Change the fit calculation in `imageGeometry.ts` to:

```ts
const fitScale = Math.min(
  (geometry.stage.width * geometry.fitInset) / sourceWidth,
  (geometry.stage.height * geometry.fitInset) / sourceHeight,
)
```

Do not change `MIN_PREVIEW_ZOOM`, `MAX_PREVIEW_ZOOM`, rotation, anchor, or pan calculations.

- [ ] **Step 4: Run pure geometry tests green**

Run:

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/useImageViewport.test.tsx
```

Expected: PASS; existing large-source fit, rotations, anchors, and boundaries remain green.

- [ ] **Step 5: Add a failing component regression for proxy-to-original bounds**

In `ImagePreview.test.tsx`, add a helper that derives visible dimensions from the committed attributes and transform:

```ts
function visibleImageSize(image: HTMLElement) {
  const scale = Number(image.getAttribute('style')?.match(/scale\(([^)]+)\)/)?.[1])
  return {
    width: Number(image.getAttribute('width')) * scale,
    height: Number(image.getAttribute('height')) * scale,
  }
}
```

Add a test with `imageMetadata: null`, a `300 × 200` deferred fit representation, and a `6000 × 4000` deferred original. Resolve fit first, record `visibleImageSize`, resolve original, and assert:

```ts
expect(preview).toHaveAttribute('data-representation', 'fit')
const fitSize = visibleImageSize(preview)

await act(async () => original.resolve(loaded('original', 6000, 4000)))
await waitFor(() => expect(preview).toHaveAttribute('data-representation', 'original'))

expect(visibleImageSize(preview).width).toBeCloseTo(fitSize.width, 6)
expect(visibleImageSize(preview).height).toBeCloseTo(fitSize.height, 6)
```

Also click `放大` before resolving the original in a second assertion and prove the visible `125%` state and transform remain continuous.

- [ ] **Step 6: Run the component test and verify it fails for mixed geometry**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx
```

Expected: FAIL because the image currently reads raw `sourceDimensions` while its transform is calculated from the previously committed viewport geometry.

- [ ] **Step 7: Commit source measurement before paint and render the committed pair**

In `ImagePreview.tsx`:

1. import `useLayoutEffect`;
2. replace the `viewport.setMeasurements(stageSize, sourceDimensions)` passive effect with `useLayoutEffect`;
3. define `const renderedSource = viewport.geometry.source`;
4. render the main image only when both rendered-source dimensions are positive;
5. set `width={renderedSource.width}` and `height={renderedSource.height}` while keeping `style={{ transform: viewport.transform }}`.

Use this render guard:

```tsx
const sourceGeometryReady = renderedSource.width > 0 && renderedSource.height > 0

{representation && sourceGeometryReady ? (
  <img
    className="image-preview-image"
    src={representation.url}
    alt={file.name}
    width={renderedSource.width}
    height={renderedSource.height}
    draggable={false}
    data-mode={viewport.state.mode}
    data-representation={originalRepresentation === null ? 'fit' : 'original'}
    style={{ transform: viewport.transform }}
  />
) : null}
```

Keep the existing representation loading/failure decisions; the layout effect prevents the transient geometry-preparation commit from painting.

- [ ] **Step 8: Run the focused preview suite green**

Run:

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/useImageViewport.test.tsx src/components/ImagePreview.test.tsx src/acceptance/scenes/viewingScenes.test.tsx
pnpm --dir ui check
```

Expected: PASS with the regression showing identical fit/original visible bounds.

- [ ] **Step 9: Commit stable progressive geometry**

```bash
git add ui/src/components/imagePreview/imageGeometry.ts ui/src/components/imagePreview/imageGeometry.test.ts ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx
git commit -m "fix: stabilize progressive preview geometry"
```

---

### Task 2: Upgrade the Rust and Tauri Settings Contract to Schema 4

**Files:**
- Modify: `crates/viewer-application/src/settings.rs`
- Modify: `crates/viewer-infrastructure/src/settings.rs`
- Modify: `src-tauri/src/dto/settings.rs`

**Interfaces:**
- Produces: `VIEWER_SETTINGS_SCHEMA_VERSION: u32 = 4`.
- Produces: `MagnifierMagnification::{Two, Three, Four}` with `Two` as `Default`.
- Preserves: `TryFrom<f64>` and `From<MagnifierMagnification> for f64`.
- Produces: schema-3 migration `1.5 → Two`, `2 → Two`, `3 → Three` while preserving density, shape, and area.

- [ ] **Step 1: Write failing domain tests for schema 4 and 2/3/4**

Replace the domain expectations with:

```rust
#[test]
fn default_settings_use_schema_four_and_small_circle_two_x_magnifier() {
    assert_eq!(VIEWER_SETTINGS_SCHEMA_VERSION, 4);
    assert_eq!(
        ViewerSettings::default().magnifier.magnification,
        MagnifierMagnification::Two
    );
}

#[test]
fn magnification_accepts_only_the_three_public_values() {
    for (public_value, expected) in [
        (2.0, MagnifierMagnification::Two),
        (3.0, MagnifierMagnification::Three),
        (4.0, MagnifierMagnification::Four),
    ] {
        let parsed = MagnifierMagnification::try_from(public_value).expect("public value");
        assert_eq!(parsed, expected);
        assert_eq!(f64::from(parsed), public_value);
    }
    for rejected in [0.0, 1.5, 2.5, 5.0, f64::INFINITY, f64::NAN] {
        assert!(MagnifierMagnification::try_from(rejected).is_err());
    }
}
```

- [ ] **Step 2: Run domain tests red**

```bash
cargo test -p viewer-application settings::tests
```

Expected: FAIL on schema `3`, missing `Four`, and `1.5` still being accepted.

- [ ] **Step 3: Implement the bounded domain values**

Set the schema constant to `4`, delete `OnePointFive`, add `Four`, put `#[default]` on `Two`, and implement exact numeric conversions:

```rust
match value {
    2.0 => Ok(Self::Two),
    3.0 => Ok(Self::Three),
    4.0 => Ok(Self::Four),
    _ => Err(()),
}
```

Update domain test fixtures that previously constructed `OnePointFive` to `Two` or use `Four` where the test needs the highest public value.

- [ ] **Step 4: Run domain tests green**

```bash
cargo test -p viewer-application settings::tests
```

Expected: PASS.

- [ ] **Step 5: Write failing infrastructure migration and round-trip tests**

Change the round-trip expectation to schema 4 and add a schema-3 migration table:

```rust
#[test]
fn version_three_preserves_preferences_and_migrates_magnification() {
    for (stored_value, expected) in [
        (1.5, MagnifierMagnification::Two),
        (2.0, MagnifierMagnification::Two),
        (3.0, MagnifierMagnification::Three),
    ] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("settings.json"),
            serde_json::json!({
                "schemaVersion": 3,
                "thumbnailDensity": "maximum",
                "magnifier": {
                    "shape": "rounded_rectangle",
                    "magnification": stored_value,
                    "area": "large"
                }
            }).to_string(),
        ).unwrap();

        let loaded = JsonViewerSettingsStore::new(directory.path().to_path_buf()).load();
        assert_eq!(loaded.thumbnail_density, ThumbnailDensity::Maximum);
        assert_eq!(loaded.magnifier.shape, MagnifierShape::RoundedRectangle);
        assert_eq!(loaded.magnifier.magnification, expected);
        assert_eq!(loaded.magnifier.area, MagnifierArea::Large);
    }
}
```

Add a schema-4 test covering `2.0`, `3.0`, and `4.0`, and update the unsupported-schema test to use `5` rather than `4`.

- [ ] **Step 6: Run infrastructure settings tests red**

```bash
cargo test -p viewer-infrastructure settings::tests
```

Expected: FAIL because save/parse still targets schema 3 and no schema-4 stored type exists.

- [ ] **Step 7: Implement schema-4 storage and schema-3 migration**

Keep `StoredViewerSettingsV3` for migration and add `StoredViewerSettingsV4` with the same strict camelCase shape. Save `StoredViewerSettingsV4`, route `schemaVersion: 4` to `parse_v4`, and replace the old schema-3 direct parse with:

```rust
fn parse_v3(value: serde_json::Value) -> Option<ViewerSettings> {
    let stored = serde_json::from_value::<StoredViewerSettingsV3>(value).ok()?;
    let magnification = match stored.magnifier.magnification {
        1.5 | 2.0 => MagnifierMagnification::Two,
        3.0 => MagnifierMagnification::Three,
        _ => return None,
    };
    (stored.schema_version == 3).then_some(ViewerSettings {
        thumbnail_density: stored.thumbnail_density,
        magnifier: MagnifierPreferences {
            shape: stored.magnifier.shape,
            magnification,
            area: stored.magnifier.area,
        },
    })
}
```

`parse_v4` must use `MagnifierMagnification::try_from` so only 2/3/4 are accepted. Leave schema-1 and schema-2 branches unchanged apart from inheriting the new default.

- [ ] **Step 8: Update Tauri DTO tests before DTO production code**

In `src-tauri/src/dto/settings.rs`, change the serialized fixture to `MagnifierMagnification::Four`, expect `schemaVersion: 4` and `magnification: 4.0`, then reject `1.5` while accepting `4.0` in the input conversion test.

Run:

```bash
cargo test -p viewer-desktop dto::settings::tests
```

Expected before all production updates settle: FAIL on the old schema/value contract. After the domain and storage changes, update only stale test fixtures and DTO expectations; the DTO conversion implementation continues using the bounded domain `TryFrom<f64>`.

- [ ] **Step 9: Run all settings layers green**

```bash
cargo test -p viewer-application settings::tests
cargo test -p viewer-infrastructure settings::tests
cargo test -p viewer-desktop dto::settings::tests
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Expected: PASS; no Rust layer accepts `1.5`, and schema-3 migration preserves the other preferences.

- [ ] **Step 10: Commit schema-4 persistence**

```bash
git add crates/viewer-application/src/settings.rs crates/viewer-infrastructure/src/settings.rs src-tauri/src/dto/settings.rs
git commit -m "feat: migrate magnifier settings to schema four"
```

---

### Task 3: Publish 2×/3×/4× in React Settings and Fixtures

**Files:**
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/settings/viewerSettings.ts`
- Modify: `ui/src/settings/viewerSettings.test.ts`
- Modify: `ui/src/settings/ViewerSettingsProvider.test.tsx`
- Modify: `ui/src/components/SettingsDialog.test.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/EmptyProject.test.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`
- Modify: `ui/src/state/useViewerController.test.tsx`
- Modify: `ui/src/acceptance/acceptanceBridge.ts`
- Modify: `ui/src/acceptance/acceptanceBridge.test.ts`
- Modify: `ui/src/acceptance/scenes/dialogScenes.tsx`
- Modify: `ui/src/acceptance/scenes/dialogScenes.test.tsx`
- Modify: `ui/src/acceptance/scenes/feedbackScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx`
- Modify: `ui/src/acceptance/scenes/workspaceScenes.tsx`

**Interfaces:**
- Produces: `MagnifierMagnification = 2 | 3 | 4`.
- Produces: `MAGNIFIER_MAGNIFICATIONS = [2, 3, 4]`.
- Produces: `DEFAULT_VIEWER_SETTINGS_UPDATE.magnifier.magnification = 2`.
- Produces: frontend `ViewerSettings.schemaVersion = 4` fixtures from the bridge.

- [ ] **Step 1: Change public frontend expectations first**

In `viewerSettings.test.ts`, require:

```ts
expect(DEFAULT_VIEWER_SETTINGS_UPDATE).toEqual({
  thumbnailDensity: 'standard',
  magnifier: { shape: 'circle', magnification: 2, area: 'small' },
})
expect(MAGNIFIER_MAGNIFICATIONS).toEqual([2, 3, 4])
```

In `SettingsDialog.test.tsx`, set the fixture to 2×, assert the labels `2 倍`, `3 倍`, `4 倍`, and click `4 倍`, expecting `onMagnifierMagnificationChange(4)`.

- [ ] **Step 2: Run the two frontend tests red**

```bash
pnpm --dir ui exec vitest run src/settings/viewerSettings.test.ts src/components/SettingsDialog.test.tsx
```

Expected: FAIL because the public type/list/default still expose 1.5/2/3.

- [ ] **Step 3: Implement the frontend public contract**

Change:

```ts
export type MagnifierMagnification = 2 | 3 | 4
export const MAGNIFIER_MAGNIFICATIONS: readonly MagnifierMagnification[] = [2, 3, 4]
```

Set `DEFAULT_VIEWER_SETTINGS_UPDATE.magnifier.magnification` to `2`. `SettingsDialog.tsx` already maps the typed list, so do not add separate controls or unchecked numeric casts.

- [ ] **Step 4: Run the focused settings tests green**

```bash
pnpm --dir ui exec vitest run src/settings/viewerSettings.test.ts src/components/SettingsDialog.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Update provider behavior fixtures to schema 4**

In `ViewerSettingsProvider.test.tsx`:

- change `DEFAULT_MAGNIFIER.magnification` to `2`;
- return `{ schemaVersion: 4, ... }` from `settings()`;
- change the test button to `setMagnifierMagnification(4)` and label it `four`;
- update optimistic/save/rollback payloads from `1.5` to `2` and from the selected `3` to `4` where the test is exercising a user change.

Run:

```bash
pnpm --dir ui exec vitest run src/settings/ViewerSettingsProvider.test.tsx
```

Expected: PASS and every serialized optimistic update contains a valid schema-4 magnification.

- [ ] **Step 6: Update all product and acceptance fixtures deliberately**

Replace magnifier default fixtures with `magnification: 2` in the listed App, EmptyProject, ImagePreview, controller, bridge, and acceptance scene files. Change acceptance dialog readiness from:

```ts
'1.5 倍|2 倍|3 倍'
```

to:

```ts
'2 倍|3 倍|4 倍'
```

Change PRE-08 readiness and its test from `--magnifier-scale: 1.5` to `2`. Change bridge/settings response fixtures from `schemaVersion: 3` to `schemaVersion: 4` only where the object is a Viewer settings object; do not alter unrelated portable-metadata schema version 3 files.

- [ ] **Step 7: Prove no product setting fixture still exposes 1.5**

Run:

```bash
rg -n "magnification: 1\.5|1\.5 倍|schemaVersion: 3" ui/src
```

Expected: no magnifier/settings result. Any remaining `1.5` must be an unrelated aspect ratio, line height, or preview-relative zoom test and must not be mechanically changed.

- [ ] **Step 8: Run the complete UI suite and type check**

```bash
pnpm --dir ui test
pnpm --dir ui check
```

Expected: PASS; settings controls, bridge data, preview, feedback, and acceptance scenes agree on 2/3/4 and schema 4.

- [ ] **Step 9: Commit the frontend setting contract**

```bash
git add ui/src/api/types.ts ui/src/settings ui/src/components/SettingsDialog.test.tsx ui/src/App.test.tsx ui/src/components/EmptyProject.test.tsx ui/src/components/ImagePreview.test.tsx ui/src/components/imagePreview/ImageMagnifier.test.tsx ui/src/state/useViewerController.test.tsx ui/src/acceptance
git commit -m "feat: expose two three and four times magnification"
```

---

### Task 4: Enlarge All Six Lens Areas

**Files:**
- Modify: `ui/src/components/imagePreview/magnifierGeometry.ts`
- Modify: `ui/src/components/imagePreview/magnifierGeometry.test.ts`
- Modify: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Preserves: `lensDimensions(shape: MagnifierShape, area: MagnifierArea): Size`.
- Preserves: `magnifierShellPlacement(pointer, stage, lens): MagnifierShellPlacement`.
- Produces: the six exact approved dimensions from Global Constraints.

- [ ] **Step 1: Replace dimension expectations before production constants**

In `magnifierGeometry.test.ts` require:

```ts
expect(lensDimensions('circle', 'small')).toEqual({ width: 200, height: 200 })
expect(lensDimensions('circle', 'medium')).toEqual({ width: 280, height: 280 })
expect(lensDimensions('circle', 'large')).toEqual({ width: 380, height: 380 })
expect(lensDimensions('rounded_rectangle', 'small')).toEqual({ width: 230, height: 150 })
expect(lensDimensions('rounded_rectangle', 'medium')).toEqual({ width: 300, height: 200 })
expect(lensDimensions('rounded_rectangle', 'large')).toEqual({ width: 420, height: 280 })
```

Update the small-circle source-center case to expect `left: -1500`, `top: -1100`, with the sampled point landing at `100, 100`.

- [ ] **Step 2: Run geometry tests red**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/magnifierGeometry.test.ts
```

Expected: FAIL on all six old dimensions and the old 80 px center.

- [ ] **Step 3: Change only the typed lens-dimension table**

Set `LENS_DIMENSIONS` to:

```ts
const LENS_DIMENSIONS: Record<MagnifierArea, Record<MagnifierShape, Size>> = {
  small: {
    circle: { width: 200, height: 200 },
    rounded_rectangle: { width: 230, height: 150 },
  },
  medium: {
    circle: { width: 280, height: 280 },
    rounded_rectangle: { width: 300, height: 200 },
  },
  large: {
    circle: { width: 380, height: 380 },
    rounded_rectangle: { width: 420, height: 280 },
  },
}
```

Do not change the 18 px pointer gap or placement algorithm.

- [ ] **Step 4: Update placement expectations using the new small circle**

For a 200 px small circle in the existing `640 × 480` cases, require lower-right center `318, 268`, flipped center `482, 322`, and shell origin `200, 200`. In `ImageMagnifier.test.tsx`, update the six render rows, source offsets, and imperative placement values; with 2× at source scale `0.25`, expect `--magnifier-scale: 0.5`.

- [ ] **Step 5: Add an overflow-safety assertion**

In `ImagePreview.test.tsx`, render a large rounded rectangle in the 720×450 contract case and assert the lens remains a child of `.image-preview-stage`. In `ui/src/styles/app.test.ts`, add the exact assertion:

```ts
expect(declaration('.image-preview-stage', 'overflow')).toBe('hidden')
```

The test must not introduce adaptive lens dimensions.

- [ ] **Step 6: Run magnifier and style suites green**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/magnifierGeometry.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/ImagePreview.test.tsx src/styles/app.test.ts src/styles/visualAccessibility.test.ts
pnpm --dir ui check
```

Expected: PASS with all six exact sizes and unchanged placement/animation semantics.

- [ ] **Step 7: Commit the larger lens areas**

```bash
git add ui/src/components/imagePreview/magnifierGeometry.ts ui/src/components/imagePreview/magnifierGeometry.test.ts ui/src/components/imagePreview/ImageMagnifier.test.tsx ui/src/components/ImagePreview.test.tsx
git commit -m "feat: enlarge magnifier viewing areas"
```

---

### Task 5: Run Formal Acceptance and Launch the Development Build

**Files:**
- Modify only if deterministic readiness requires alignment: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify only if deterministic readiness requires alignment: `ui/src/acceptance/scenes/viewingScenes.test.tsx`
- Create after measured evidence exists: `docs/reviews/2026-08-07-viewer-stable-progressive-preview-acceptance.md`
- Modify after evidence exists: `docs/README.md`

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces: exact automated, visual, resource, and physical MacBook verdicts tied to one implementation commit.
- Produces: one running latest development Viewer instance.

- [ ] **Step 1: Run focused feature tests**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/useImageViewport.test.tsx src/components/imagePreview/magnifierGeometry.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/ImagePreview.test.tsx src/settings/viewerSettings.test.ts src/settings/ViewerSettingsProvider.test.tsx src/components/SettingsDialog.test.tsx src/acceptance/scenes/dialogScenes.test.tsx src/acceptance/scenes/viewingScenes.test.tsx
cargo test -p viewer-application settings::tests
cargo test -p viewer-infrastructure settings::tests
cargo test -p viewer-desktop dto::settings::tests
```

Expected: every selected feature test passes without a skipped feature case.

- [ ] **Step 2: Run the complete repository gate**

```bash
pnpm verify
pnpm quality:report
```

Expected: both commands exit 0; UI/Rust coverage and architecture baselines do not regress. Known advisory duplicate-dependency warnings may print but the configured security gate must finish with `bans ok, licenses ok, sources ok`.

- [ ] **Step 3: Generate the approved preview visual scenes**

```bash
pnpm accept:visual -- --id PRE-01 --id PRE-02 --id PRE-03 --id PRE-05 --id PRE-06 --id PRE-08
```

Inspect both 1024×720 and 1440×900 product captures. Confirm PRE-01/PRE-02 are fitted 100%, PRE-03 is 156%, failure states retain a fitted proxy, and PRE-08 uses the larger 200 px small circle at 2×. If the historical reference fixture remains absent, report the exact missing path and distinguish successful product capture from unavailable pixel comparison.

- [ ] **Step 4: Restart one latest development instance**

```bash
pnpm start:viewer
```

Record the launcher-reported PID, branch, commit, and log path. Confirm exactly one development binary:

```bash
VIEWER_PROCESS_ID="$(pgrep -n -f '/target/debug/viewer-desktop$')"
test -n "$VIEWER_PROCESS_ID"
test "$(pgrep -f '/target/debug/viewer-desktop$' | wc -l | tr -d ' ')" = "1"
ps -o pid,%cpu,rss,etime,command -p "$VIEWER_PROCESS_ID"
```

- [ ] **Step 5: Ask for the physical MacBook verdict**

Against the running build, ask the user to verify:

1. opening several uncached images shows a fitted proxy that sharpens without changing bounds;
2. settings show only 2×/3×/4× and restart at the saved value;
3. circle and rounded-rectangle small/medium/large are visibly larger and remain usable near all four edges;
4. Q/button, pointer visibility, animation, trackpad zoom, navigation, and settled machine temperature have no regression.

Do not mark physical acceptance pass until the user explicitly reports it.

- [ ] **Step 6: Write measured acceptance evidence**

Create `docs/reviews/2026-08-07-viewer-stable-progressive-preview-acceptance.md` only after the commands and physical check. Record:

- implementation commit;
- exact UI and Rust totals;
- coverage/architecture verdicts;
- visual product output directories and reference-comparison status;
- Viewer PID, CPU, RSS, elapsed time, and test image state;
- pass/fail for each physical item;
- any historical fixture limitation without presenting it as a product failure.

Add the review to `docs/README.md` as Historical evidence.

- [ ] **Step 7: Verify evidence edits and commit**

```bash
pnpm test:policy
git diff --check
git status --short
git add docs/README.md docs/reviews/2026-08-07-viewer-stable-progressive-preview-acceptance.md ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx
git commit -m "docs: verify stable progressive preview revision"
```

Stage the two acceptance scene files only if Step 3 required a deterministic readiness correction.

---

## Final Completion Check

Before claiming completion, invoke `superpowers:verification-before-completion` and inspect fresh final-tree output. Report automated gates, visual capture/comparison, resource sample, and physical MacBook verdict separately. A synthetic geometry or wheel test does not prove the user's observed first-paint quality or hardware experience.
