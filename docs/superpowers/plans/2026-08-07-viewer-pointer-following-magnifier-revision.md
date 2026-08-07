# Viewer Pointer-Following Magnifier Revision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the image magnifier appear immediately when Q is pressed over a stationary image, keep the normal pointer visible while a bounded lens follows beside it, add reversible pop/retract motion, and replace magnification choices with 1.5x/2x/3x defaulting to 1.5x.

**Architecture:** A Viewer-lifetime pointer tracker supplies one mutable client-coordinate ref to `ImagePreview`; the preview continues to own enabled state and source hit-testing. Pure magnifier geometry separates the pixel sampled under the arrow from the bounded lens-shell position beside it, while `ImageMagnifier` keeps animation-frame-coalesced CSS updates. Settings advance to schema v3 with an exact Rust enum and explicit v1/v2 density-only migration.

**Tech Stack:** Rust 2024, serde/serde_json, Tauri 2, React 19, TypeScript 5, CSS custom properties/transitions, Vitest/Testing Library, pnpm, Cargo.

## Global Constraints

- Unmodified Q and the toolbar button control one session-only enabled state; modified, repeated, composing, and already-prevented Q remain unowned.
- The ordinary pointer is always visible; the lens is always `pointer-events: none`.
- The preferred shell begins 18 CSS px right and below the pointer, flips each axis independently, and remains bounded inside the stage.
- The pointer remains the exact original-detail sample point even though the lens shell is offset.
- Enter motion is 140 ms ease-out from 85% scale/zero opacity; exit is 110 ms ease-in to 85%/zero opacity; reduced motion is immediate.
- Magnification choices are exactly numeric `1.5 | 2 | 3`, default `1.5`.
- Schema v1/v2 preserve only valid thumbnail density and reset the magnifier to circle/1.5x/small; malformed and unknown settings use all defaults.
- Existing circle/rounded-rectangle sizes, 100 MP/700 MB original budget, fit-proxy prohibition, current-original ownership, navigation lifetime, trackpad gestures, and viewport transform behavior do not change.
- Do not add AppKit cursor hooks, dependencies, a second transform model, canvas pixel copying, or per-pointer React state updates.
- Preserve unrelated user changes and keep the latest development Viewer running except while the repository launcher replaces it with a newer build.

---

### Task 1: Define exact schema-v3 magnification values in the application domain

**Files:**
- Modify: `crates/viewer-application/src/settings.rs`
- Verify: `crates/viewer-application/src/lib.rs`

**Interfaces:**
- Produces: `VIEWER_SETTINGS_SCHEMA_VERSION: u32 = 3`.
- Produces: `MagnifierMagnification::{OnePointFive, Two, Three}`.
- Produces: `TryFrom<f64> for MagnifierMagnification` and `From<MagnifierMagnification> for f64`.
- Preserves: `MagnifierPreferences`, `ViewerSettings`, `ViewerSettingsPort`, and `ViewerSettingsService` public shapes.

- [ ] **Step 1: Replace the domain expectations with failing schema-v3 tests**

Update the two magnification tests in `settings.rs` to assert the exact new default and public values:

```rust
#[test]
fn default_settings_use_schema_three_and_small_circle_one_point_five_x_magnifier() {
    assert_eq!(VIEWER_SETTINGS_SCHEMA_VERSION, 3);
    assert_eq!(
        ViewerSettings::default(),
        ViewerSettings {
            thumbnail_density: ThumbnailDensity::Standard,
            magnifier: MagnifierPreferences {
                shape: MagnifierShape::Circle,
                magnification: MagnifierMagnification::OnePointFive,
                area: MagnifierArea::Small,
            },
        }
    );
}

#[test]
fn magnification_accepts_only_the_three_public_values() {
    for (public_value, expected) in [
        (1.5, MagnifierMagnification::OnePointFive),
        (2.0, MagnifierMagnification::Two),
        (3.0, MagnifierMagnification::Three),
    ] {
        let parsed = MagnifierMagnification::try_from(public_value).expect("public value");
        assert_eq!(parsed, expected);
        assert_eq!(f64::from(parsed), public_value);
    }
    for rejected in [0.0, 1.0, 1.4, 2.5, 4.0, f64::INFINITY, f64::NAN] {
        assert!(MagnifierMagnification::try_from(rejected).is_err());
    }
}
```

Change the service update fixture from `Six` to `Three`.

- [ ] **Step 2: Run the focused test and verify the intended compile failure**

```bash
cargo test -p viewer-application settings
```

Expected: FAIL because schema 2 and the old `Four/Five/Six` variants still exist while `OnePointFive/Two` and `TryFrom<f64>` do not.

- [ ] **Step 3: Implement the minimal exact domain enum**

Replace the old enum and integer conversions with:

```rust
pub const VIEWER_SETTINGS_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MagnifierMagnification {
    #[default]
    OnePointFive,
    Two,
    Three,
}

impl TryFrom<f64> for MagnifierMagnification {
    type Error = ();

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        match value {
            1.5 => Ok(Self::OnePointFive),
            2.0 => Ok(Self::Two),
            3.0 => Ok(Self::Three),
            _ => Err(()),
        }
    }
}

impl From<MagnifierMagnification> for f64 {
    fn from(value: MagnifierMagnification) -> Self {
        match value {
            MagnifierMagnification::OnePointFive => 1.5,
            MagnifierMagnification::Two => 2.0,
            MagnifierMagnification::Three => 3.0,
        }
    }
}
```

- [ ] **Step 4: Run the focused domain tests and formatting**

```bash
cargo fmt --all
cargo test -p viewer-application settings
```

Expected: all application settings tests pass.

- [ ] **Step 5: Commit the domain change**

```bash
git add crates/viewer-application/src/settings.rs
git commit -m "feat: define schema three magnification values"
```

---

### Task 2: Persist schema v3 and reset old magnifier preferences

**Files:**
- Modify: `crates/viewer-infrastructure/src/settings.rs`

**Interfaces:**
- Consumes: `MagnifierMagnification::try_from(f64)` and `f64::from(MagnifierMagnification)` from Task 1.
- Produces: strict `StoredViewerSettingsV3` persisted with `schemaVersion: 3` and numeric magnification.
- Produces: v1/v2 migration that preserves valid `thumbnail_density` and uses `MagnifierPreferences::default()`.

- [ ] **Step 1: Write failing round-trip and old-version migration tests**

Change the round-trip fixture to `MagnifierMagnification::Two` and freeze this JSON:

```rust
serde_json::json!({
    "schemaVersion": 3,
    "thumbnailDensity": "large",
    "magnifier": {
        "shape": "rounded_rectangle",
        "magnification": 2.0,
        "area": "medium"
    }
})
```

Replace `version_two_loads_every_bounded_magnifier_value` with:

```rust
#[test]
fn version_two_preserves_density_and_resets_magnifier_preferences() {
    for old_magnification in [3, 4, 5, 6] {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("settings.json"),
            serde_json::json!({
                "schemaVersion": 2,
                "thumbnailDensity": "maximum",
                "magnifier": {
                    "shape": "rounded_rectangle",
                    "magnification": old_magnification,
                    "area": "large"
                }
            })
            .to_string(),
        )
        .unwrap();
        let store = JsonViewerSettingsStore::new(directory.path().to_path_buf());

        assert_eq!(
            store.load(),
            ViewerSettings {
                thumbnail_density: ThumbnailDensity::Maximum,
                magnifier: MagnifierPreferences::default(),
            }
        );
    }
}
```

Add a strict v3 test that accepts 1.5/2/3 and an invalid-v3 table that rejects 1, 2.5, 4, strings, unknown shape/area, and unknown fields by returning all defaults.

- [ ] **Step 2: Run the infrastructure settings tests and verify failure**

```bash
cargo test -p viewer-infrastructure settings
```

Expected: FAIL because saving still emits schema 2, v2 still preserves its magnifier, and schema 3 is unsupported.

- [ ] **Step 3: Add version-specific stored structs and parsing**

Keep the existing v1 and old v2 structs. Add:

```rust
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredViewerSettingsV3 {
    schema_version: u32,
    thumbnail_density: ThumbnailDensity,
    magnifier: StoredMagnifierPreferencesV3,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredMagnifierPreferencesV3 {
    shape: MagnifierShape,
    magnification: f64,
    area: MagnifierArea,
}
```

Serialize only `StoredViewerSettingsV3`. Dispatch parsing as follows:

```rust
match value.get("schemaVersion")?.as_u64()? {
    1 => parse_v1(value),
    2 => parse_v2_density_only(value),
    3 => parse_v3(value),
    _ => None,
}
```

`parse_v2_density_only` must strictly deserialize the complete v2 shape and additionally require `matches!(stored.magnifier.magnification, 3 | 4 | 5 | 6)` before preserving density. `parse_v3` converts its `f64` through the exact domain `TryFrom` and requires `stored.schema_version == VIEWER_SETTINGS_SCHEMA_VERSION`.

- [ ] **Step 4: Run infrastructure and application settings tests**

```bash
cargo fmt --all
cargo test -p viewer-application settings
cargo test -p viewer-infrastructure settings
```

Expected: both settings suites pass, including strict invalid-v3 fallback and density-only old-version migration.

- [ ] **Step 5: Commit persistence migration**

```bash
git add crates/viewer-infrastructure/src/settings.rs
git commit -m "feat: migrate viewer settings to schema three"
```

---

### Task 3: Update the Tauri and TypeScript settings contracts

**Files:**
- Modify: `src-tauri/src/dto/settings.rs`
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/settings/viewerSettings.ts`
- Modify: `ui/src/settings/viewerSettings.test.ts`
- Modify: `ui/src/settings/ViewerSettingsProvider.test.tsx`
- Modify: `ui/src/components/SettingsDialog.test.tsx`
- Modify fixtures in: `ui/src/App.test.tsx`
- Modify fixtures in: `ui/src/acceptance/acceptanceBridge.ts`
- Modify fixtures in: `ui/src/acceptance/acceptanceBridge.test.ts`
- Modify fixtures in: `ui/src/acceptance/scenes/dialogScenes.tsx`
- Modify fixtures in: `ui/src/acceptance/scenes/feedbackScenes.tsx`
- Modify fixtures in: `ui/src/acceptance/scenes/workspaceScenes.tsx`
- Modify fixtures in: `ui/src/components/EmptyProject.test.tsx`
- Modify fixtures in: `ui/src/state/useViewerController.test.tsx`

**Interfaces:**
- Consumes: schema-v3 Rust enum and conversion from Task 1.
- Produces: Tauri JSON `schemaVersion: 3` and numeric `magnification: 1.5 | 2 | 3`.
- Produces: TypeScript `MagnifierMagnification = 1.5 | 2 | 3` and `ViewerSettings.schemaVersion = 3`.
- Produces: `MAGNIFIER_MAGNIFICATIONS = [1.5, 2, 3]` and default `1.5`.

- [ ] **Step 1: Write failing frozen DTO tests**

Update the successful output/input expectations to `schemaVersion: 3` and `magnification: 1.5` or `2`. Reject numeric values `[0, 1, 1.4, 2.5, 4, 6]` and continue rejecting unknown fields/strings.

The successful conversion expectation is:

```rust
ViewerSettings {
    thumbnail_density: ThumbnailDensity::Maximum,
    magnifier: MagnifierPreferences {
        shape: MagnifierShape::RoundedRectangle,
        magnification: MagnifierMagnification::Three,
        area: MagnifierArea::Large,
    },
}
```

with input JSON `"magnification": 3`.

- [ ] **Step 2: Write failing TypeScript default and bridge tests**

Change `viewerSettings.test.ts` to require:

```ts
expect(MAGNIFIER_MAGNIFICATIONS).toEqual([1.5, 2, 3])
expect(DEFAULT_VIEWER_SETTINGS_UPDATE).toEqual({
  thumbnailDensity: 'standard',
  magnifier: { shape: 'circle', magnification: 1.5, area: 'small' },
})
```

Change the bridge contract to submit and receive schema-v3 settings with a `2` or `3` magnification. Update the Settings dialog test to assert visible radio labels `1.5 倍`, `2 倍`, and `3 倍`, with `1.5 倍` checked by default.

- [ ] **Step 3: Run focused Rust and UI tests and verify failure**

```bash
cargo test -p viewer-desktop settings
pnpm --dir ui exec vitest run src/api/viewer.test.ts src/settings src/components/SettingsDialog.test.tsx
```

Expected: FAIL because the DTO and TypeScript contracts still expose schema 2 and 3/4/5/6.

- [ ] **Step 4: Implement the exact DTO and public types**

Change DTO magnification fields from `u8` to `f64`, convert output with `f64::from`, and convert input with `MagnifierMagnification::try_from`. The input DTO no longer derives `Eq` because it contains `f64`; it remains `Clone`, `Copy`, `Debug`, `Deserialize`, and `PartialEq`.

Update TypeScript declarations exactly:

```ts
export type MagnifierMagnification = 1.5 | 2 | 3

export interface ViewerSettings extends ViewerSettingsUpdate {
  schemaVersion: 3
}
```

Update the settings constants exactly:

```ts
export const MAGNIFIER_MAGNIFICATIONS: readonly MagnifierMagnification[] = [1.5, 2, 3]

export const DEFAULT_VIEWER_SETTINGS_UPDATE: ViewerSettingsUpdate = {
  thumbnailDensity: 'standard',
  magnifier: { shape: 'circle', magnification: 1.5, area: 'small' },
}
```

Mechanically update every listed fixture to schema 3 and a valid new magnification without changing unrelated test intent.

- [ ] **Step 5: Run the complete settings boundary suites**

```bash
cargo fmt --all
cargo test -p viewer-desktop settings
pnpm --dir ui exec vitest run src/api/viewer.test.ts src/settings src/components/SettingsDialog.test.tsx src/App.test.tsx src/acceptance/acceptanceBridge.test.ts src/components/EmptyProject.test.tsx src/state/useViewerController.test.tsx
```

Expected: all selected tests pass and TypeScript accepts no old magnification fixture.

- [ ] **Step 6: Commit the complete settings contract**

```bash
git add src-tauri/src/dto/settings.rs ui/src/api/types.ts ui/src/api/viewer.test.ts ui/src/settings ui/src/components/SettingsDialog.test.tsx ui/src/App.test.tsx ui/src/acceptance/acceptanceBridge.ts ui/src/acceptance/acceptanceBridge.test.ts ui/src/acceptance/scenes/dialogScenes.tsx ui/src/acceptance/scenes/feedbackScenes.tsx ui/src/acceptance/scenes/workspaceScenes.tsx ui/src/components/EmptyProject.test.tsx ui/src/state/useViewerController.test.tsx
git commit -m "feat: expose revised magnifier settings"
```

---

### Task 4: Track the latest pointer for the Viewer lifetime

**Files:**
- Create: `ui/src/components/imagePreview/useLatestPointerClientPoint.ts`
- Create: `ui/src/components/imagePreview/useLatestPointerClientPoint.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`

**Interfaces:**
- Produces: `useLatestPointerClientPoint(): MutableRefObject<Point | null>`.
- Produces: required `ImagePreviewProps.pointerClientPoint: MutableRefObject<Point | null>`.
- The hook records client coordinates on window-capture `pointermove` and `pointerdown` events and removes both listeners on unmount.

- [ ] **Step 1: Write a failing hook lifecycle test**

Capture the hook's returned ref from a harness and assert it changes without a React
rerender:

```tsx
let observed: MutableRefObject<Point | null> | null = null
function Harness() {
  observed = useLatestPointerClientPoint()
  return null
}

const view = render(<Harness />)
fireEvent.pointerMove(window, { clientX: 120, clientY: 80 })
expect(observed?.current).toEqual({ x: 120, y: 80 })

fireEvent.pointerDown(window, { clientX: 250, clientY: 160 })
expect(observed?.current).toEqual({ x: 250, y: 160 })

view.unmount()
window.dispatchEvent(new PointerEvent('pointermove', { clientX: 400, clientY: 300 }))
expect(observed?.current).toEqual({ x: 250, y: 160 })
```

- [ ] **Step 2: Run the new test and verify module-not-found failure**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/useLatestPointerClientPoint.test.tsx
```

Expected: FAIL because the hook module does not exist.

- [ ] **Step 3: Implement capture-phase, passive pointer tracking**

Create:

```ts
export function useLatestPointerClientPoint(): MutableRefObject<Point | null> {
  const latest = useRef<Point | null>(null)
  useEffect(() => {
    const record = (event: PointerEvent) => {
      latest.current = { x: event.clientX, y: event.clientY }
    }
    window.addEventListener('pointermove', record, { capture: true, passive: true })
    window.addEventListener('pointerdown', record, { capture: true, passive: true })
    return () => {
      window.removeEventListener('pointermove', record, true)
      window.removeEventListener('pointerdown', record, true)
    }
  }, [])
  return latest
}
```

Call the hook once in `App`, pass the returned ref to the production `ImagePreview`, and add the required prop to direct test/acceptance call sites. Use stable mutable refs in tests; do not create a new ref object per rerender.

- [ ] **Step 4: Run the hook, App, and preview compile-facing tests**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/useLatestPointerClientPoint.test.tsx src/components/ImagePreview.test.tsx src/App.test.tsx src/acceptance/scenes/viewingScenes.test.tsx
```

Expected: all selected tests pass; no magnifier behavior has changed yet.

- [ ] **Step 5: Commit the pointer source**

```bash
git add ui/src/components/imagePreview/useLatestPointerClientPoint.ts ui/src/components/imagePreview/useLatestPointerClientPoint.test.tsx ui/src/App.tsx ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx ui/src/acceptance/scenes/viewingScenes.tsx
git commit -m "feat: track latest viewer pointer position"
```

---

### Task 5: Compute bounded pointer-adjacent lens placement

**Files:**
- Modify: `ui/src/components/imagePreview/magnifierGeometry.ts`
- Modify: `ui/src/components/imagePreview/magnifierGeometry.test.ts`

**Interfaces:**
- Produces: `MAGNIFIER_POINTER_GAP = 18`.
- Produces: `MagnifierShellPlacement { center: Point; origin: Point; horizontal: 'left' | 'right'; vertical: 'above' | 'below' }`.
- Produces: `magnifierShellPlacement(pointer: Point, stage: Size, lens: Size): MagnifierShellPlacement`.
- Preserves: `lensDimensions` and `magnifierSourcePlacement` source-sampling behavior.

- [ ] **Step 1: Add failing geometry examples for all placement paths**

Add exact table tests using stage `640x480`, lens `160x160`, and gap 18:

```ts
expect(magnifierShellPlacement({ x: 200, y: 150 }, stage, lens)).toEqual({
  center: { x: 298, y: 248 },
  origin: { x: 0, y: 0 },
  horizontal: 'right',
  vertical: 'below',
})
expect(magnifierShellPlacement({ x: 600, y: 440 }, stage, lens)).toEqual({
  center: { x: 502, y: 342 },
  origin: { x: 160, y: 160 },
  horizontal: 'left',
  vertical: 'above',
})
```

Add independent right-only and bottom-only flip cases plus a constrained-stage case that asserts `center.x` and `center.y` remain within half-lens bounds. Retain a source-placement test proving `{x: 1600, y: 1200}` still maps to lens center independently of shell position.

- [ ] **Step 2: Run geometry tests and verify export failures**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/magnifierGeometry.test.ts
```

Expected: FAIL because `magnifierShellPlacement` and its result type do not exist.

- [ ] **Step 3: Implement independent axis choice and clamp**

Use one pure axis helper:

```ts
function placeAxis(
  pointer: number,
  stageLength: number,
  lensLength: number,
  positiveName: 'right' | 'below',
  negativeName: 'left' | 'above',
) {
  const positiveStart = pointer + MAGNIFIER_POINTER_GAP
  const negativeStart = pointer - MAGNIFIER_POINTER_GAP - lensLength
  const positiveFits = positiveStart + lensLength <= stageLength
  const negativeFits = negativeStart >= 0
  const usePositive = positiveFits || (!negativeFits && stageLength - pointer >= pointer)
  const start = clamp(usePositive ? positiveStart : negativeStart, 0, Math.max(0, stageLength - lensLength))
  return {
    center: start + lensLength / 2,
    side: usePositive ? positiveName : negativeName,
    origin: usePositive ? 0 : lensLength,
  }
}
```

Combine horizontal and vertical results without changing `magnifierSourcePlacement`.

- [ ] **Step 4: Run geometry tests and the image geometry regression suite**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/magnifierGeometry.test.ts src/components/imagePreview/imageGeometry.test.ts
```

Expected: all geometry tests pass.

- [ ] **Step 5: Commit pure geometry**

```bash
git add ui/src/components/imagePreview/magnifierGeometry.ts ui/src/components/imagePreview/magnifierGeometry.test.ts
git commit -m "feat: bound magnifier beside the pointer"
```

---

### Task 6: Render the offset lens with reversible enter/exit motion

**Files:**
- Modify: `ui/src/components/imagePreview/ImageMagnifier.tsx`
- Modify: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: `magnifierShellPlacement` from Task 5.
- Adds: `ImageMagnifierProps.stageSize: Size`.
- Preserves: `ImageMagnifierHandle.place({ stagePoint, sourcePoint })` and `hide()`.
- Produces CSS variables: `--magnifier-x`, `--magnifier-y`, `--magnifier-shell-origin-x`, and `--magnifier-shell-origin-y`.

- [ ] **Step 1: Write failing component placement tests**

Render a small circle with `stageSize={{ width: 640, height: 480 }}`, place it at
pointer `{x:200,y:150}` with source point `{x:1600,y:1140}`, flush one frame, and
assert:

```ts
expect(lens).toHaveStyle({
  '--magnifier-x': '298px',
  '--magnifier-y': '248px',
  '--magnifier-shell-origin-x': '0px',
  '--magnifier-shell-origin-y': '0px',
  '--magnifier-scale': '1.5',
})
expect(source).toHaveStyle({
  '--magnifier-source-left': '-1520px',
  '--magnifier-source-top': '-1060px',
})
```

Move the same pointer near the lower-right stage edge and assert the shell flips while the source offsets remain derived from the same source point. Verify `hide()` removes `data-visible` but leaves the element mounted for CSS exit motion.

- [ ] **Step 2: Write failing CSS contract tests**

Require these declarations:

```ts
expect(base.declarations['pointer-events']).toBe('none')
expect(base.declarations.opacity).toBe('0')
expect(base.declarations.transform).toContain('scale(0.85)')
expect(base.declarations.transition).toContain('110ms')
expect(visible.declarations.transform).toContain('scale(1)')
expect(visible.declarations.transition).toContain('140ms')
expect(reduced.declarations.transition).toBe('none')
```

Also assert there is no `.image-preview-stage[data-magnifier-over-image="true"] { cursor: none; }` rule.

- [ ] **Step 3: Run focused tests and verify failures**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/ImageMagnifier.test.tsx src/styles/app.test.ts
```

Expected: FAIL because the shell remains centered on the pointer, magnification fixtures are old, and CSS has no enter/exit transition.

- [ ] **Step 4: Implement shell placement in the imperative frame**

Include `stageSize` in `MagnifierConfiguration`. In `flush`, compute:

```ts
const shell = magnifierShellPlacement(
  placement.stagePoint,
  current.stageSize,
  currentDimensions,
)
setPixels(element, '--magnifier-x', shell.center.x)
setPixels(element, '--magnifier-y', shell.center.y)
setPixels(element, '--magnifier-shell-origin-x', shell.origin.x)
setPixels(element, '--magnifier-shell-origin-y', shell.origin.y)
```

Continue computing source offsets from `placement.sourcePoint`, not `shell.center`. Re-schedule a visible lens when shape, area, magnification, rotation, or stage size changes.

- [ ] **Step 5: Implement reversible CSS motion and preserve the arrow**

Use the shell transform independently of the source-image magnification:

```css
.image-magnifier {
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, -50%) scale(0.85);
  transform-origin: var(--magnifier-shell-origin-x) var(--magnifier-shell-origin-y);
  transition:
    opacity 110ms ease-in,
    transform 110ms ease-in,
    visibility 0s linear 110ms;
  visibility: hidden;
}

.image-magnifier[data-visible="true"] {
  opacity: 1;
  transform: translate(-50%, -50%) scale(1);
  transition:
    opacity 140ms ease-out,
    transform 140ms ease-out,
    visibility 0s linear 0s;
  visibility: visible;
}
```

Delete the cursor-hiding rule. Keep the existing reduced-motion rule with `transition: none` and forced-colors declarations.

- [ ] **Step 6: Run lens and style tests**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/ImageMagnifier.test.tsx src/components/imagePreview/magnifierGeometry.test.ts src/styles/app.test.ts
```

Expected: all selected tests pass.

- [ ] **Step 7: Commit lens rendering and motion**

```bash
git add ui/src/components/imagePreview/ImageMagnifier.tsx ui/src/components/imagePreview/ImageMagnifier.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: animate pointer-following magnifier"
```

---

### Task 7: Fix stationary Q activation in `ImagePreview`

**Files:**
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`

**Interfaces:**
- Consumes: `pointerClientPoint` from Task 4 and `ImageMagnifier.stageSize` from Task 6.
- Produces: immediate activation from the latest client coordinate without a post-Q pointer event.
- Preserves: transformed-image hit testing, Q/button parity, actual-image exit/re-entry, current-original ownership, navigation persistence, and preview-close reset.

- [ ] **Step 1: Add the failing physical-regression test**

Create a stable pointer ref before rendering:

```tsx
const pointerClientPoint = { current: { x: 320, y: 240 } }
const view = render(
  <ImagePreview
    file={target}
    files={[target]}
    magnifier={{ shape: 'circle', magnification: 1.5, area: 'small' }}
    pointerClientPoint={pointerClientPoint}
    requestImage={request}
    onNavigate={vi.fn()}
    onClose={vi.fn()}
  />,
)
```

Mock the stage bounds to `left:0, top:0, width:640, height:480`, wait for the fit image, press Q, flush animation frames, and assert `data-visible="true"`, shell position `418px,338px`, and no pointer event occurred after Q. Then set the ref to `{x: 2,y:2}`, press Q off/on, and assert the enabled button is true while the lens remains hidden outside actual image pixels.

- [ ] **Step 2: Run the single regression test and verify failure**

```bash
pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx -t "shows the pointer-adjacent lens immediately for stationary Q activation"
```

Expected: FAIL because `ImagePreview` only replays `lastStagePoint`, which is empty when no stage pointer event was observed after preview mount.

- [ ] **Step 3: Convert latest client coordinates through the stage on activation**

Add a focused callback:

```ts
const placeLatestMagnifier = useCallback(() => {
  const element = stage.current
  const clientPoint = pointerClientPoint.current
  if (element === null || clientPoint === null) {
    hideMagnifier(magnifierHandle, stage)
    return
  }
  const bounds = element.getBoundingClientRect()
  const stagePoint = {
    x: clientPoint.x - bounds.left,
    y: clientPoint.y - bounds.top,
  }
  lastStagePoint.current = stagePoint
  placeMagnifier(stagePoint)
}, [placeMagnifier, pointerClientPoint])
```

When magnifier enabled state, representation, viewport geometry, rotation, stage size, or magnifier preferences change, call `placeLatestMagnifier`. Stage pointer handlers also write event client coordinates into the same mutable ref before placing. Pass `stageSize` to `ImageMagnifier`.

Do not query native cursor position and do not use React state for pointer samples.

- [ ] **Step 4: Run the complete preview suites**

```bash
pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx src/components/imagePreview
```

Expected: all preview, original-ownership, geometry, gesture, magnifier, and stationary-Q tests pass.

- [ ] **Step 5: Commit the interaction fix**

```bash
git add ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx
git commit -m "fix: show magnifier immediately on stationary Q"
```

---

### Task 8: Update product, acceptance, and accessibility contracts

**Files:**
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx`
- Modify: `ui/src/acceptance/scenes/dialogScenes.tsx`
- Modify: `ui/src/acceptance/scenes/dialogScenes.test.tsx`
- Modify if exact copy is frozen: `scripts/viewer-native-acceptance.test.mjs`

**Interfaces:**
- Updates: PRE-08 to circle/small/1.5x, visible arrow, offset shell, and settled enter motion.
- Updates: DIA-01 to exact `1.5x/2x/3x` choices with 1.5x selected.
- Preserves: native Q/button parity plan; native automation does not claim pointer-placement or physical trackpad evidence.

- [ ] **Step 1: Write failing scene expectations**

Require the PRE-08 lens to have `--magnifier-scale: 1.5`, a shell center different from the pointer sample, `data-visible="true"`, and no stage cursor suppression. Require DIA-01 labels `1.5 倍`, `2 倍`, `3 倍`, with `1.5 倍` checked.

- [ ] **Step 2: Run the scene tests and verify old-default failures**

```bash
pnpm --dir ui exec vitest run src/acceptance/scenes/viewingScenes.test.tsx src/acceptance/scenes/dialogScenes.test.tsx
```

Expected: FAIL because acceptance fixtures still use and expect 4x.

- [ ] **Step 3: Update deterministic scene readiness without an observer feedback loop**

Pass `magnification: 1.5` and a stable pointer ref. Keep the real button and real pointer event. Once the lens/source readiness predicate first succeeds, wait one bounded 160 ms timer and two animation frames before setting `data-acceptance-scene-ready="true"`; cancel the timer/frames on cleanup. Do not schedule another pointer event in response to the lens's own style-attribute mutations.

The ready predicate must require:

```ts
lens.dataset.visible === 'true'
lens.style.getPropertyValue('--magnifier-scale') === '1.5'
lens.style.getPropertyValue('--magnifier-x') !== String(pointerX)
source.src.includes('representation=original100_percent')
```

- [ ] **Step 4: Update the product specification**

Replace old cursor-hidden/centered and 3/4/5/6 language in `docs/PRODUCT_SPEC.md` with the approved pointer-adjacent, 18 px flip/clamp, arrow-visible, 140/110 ms reduced-motion-aware behavior and schema-v3 1.5/2/3 default-1.5 settings contract.

- [ ] **Step 5: Run acceptance contracts and policy**

```bash
pnpm --dir ui exec vitest run src/acceptance/scenes/viewingScenes.test.tsx src/acceptance/scenes/dialogScenes.test.tsx
pnpm test:visual-acceptance
pnpm test:native-acceptance
pnpm build:visual-acceptance
pnpm test:policy
```

Expected: scene, visual-contract, native-plan, acceptance build, and policy tests pass. If the historical read-only native fixture remains absent, record that external fixture failure separately and require the focused PRE-08 native-plan tests to pass; do not alter the fixture contract.

- [ ] **Step 6: Commit product and acceptance updates**

```bash
git add docs/PRODUCT_SPEC.md ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx ui/src/acceptance/scenes/dialogScenes.tsx ui/src/acceptance/scenes/dialogScenes.test.tsx scripts/viewer-native-acceptance.test.mjs
git commit -m "test: revise magnifier product acceptance"
```

Omit `scripts/viewer-native-acceptance.test.mjs` from `git add` when it did not need a change.

---

### Task 9: Run complete quality gates and relaunch the latest development build

**Files:**
- Modify only when measured and justified: `docs/quality/ui-coverage-baseline.json`
- Modify only when measured and justified: `docs/quality/rust-coverage-baseline.json`

**Interfaces:**
- Produces: clean, verified feature branch and one latest-source Viewer process.
- Produces: explicit separation between automated evidence and physical pointer/trackpad retest evidence.

- [ ] **Step 1: Format only the changed source surfaces**

```bash
pnpm --dir ui exec biome format --write src/api/types.ts src/settings src/components/ImagePreview.tsx src/components/ImagePreview.test.tsx src/components/imagePreview src/components/SettingsDialog.test.tsx src/acceptance/scenes/viewingScenes.tsx src/acceptance/scenes/viewingScenes.test.tsx src/acceptance/scenes/dialogScenes.tsx src/acceptance/scenes/dialogScenes.test.tsx src/App.tsx src/styles/app.css src/styles/app.test.ts
cargo fmt --all
```

- [ ] **Step 2: Run complete UI and Rust verification**

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Expected: all commands exit 0. Check Activity Monitor/`ps` afterward and terminate only test workers created by these commands if any remain after their parent command exits.

- [ ] **Step 3: Run architecture, dependency, security, and coverage gates**

```bash
pnpm architecture:health
pnpm dependencies:exceptions
pnpm security
pnpm coverage:ui
pnpm coverage:rust
```

Expected: all gates pass. Update a baseline only with the repository's supported update script after inspecting the measured report and confirming no new settings, stationary-Q, placement, motion, cancellation, or failure branch is uncovered.

- [ ] **Step 4: Inspect scope and create any justified final metadata commit**

```bash
git status --short
git diff --check
git diff --stat 2e169e8...HEAD
git log --oneline --decorate -15
```

If and only if a measured baseline legitimately changed:

```bash
git add docs/quality/ui-coverage-baseline.json docs/quality/rust-coverage-baseline.json
git commit -m "chore: refresh verified magnifier baselines"
```

- [ ] **Step 5: Run the clean-tree repository gate**

```bash
pnpm verify
```

Expected: exit 0 on a clean worktree, including policy, UI checks/tests/build, Rust format/clippy/tests, security, dependency, and license checks.

- [ ] **Step 6: Start the newest development Viewer**

```bash
pnpm start:viewer
```

Confirm launcher output identifies branch `codex/viewer-trackpad-magnifier` and the final `HEAD`, only one Viewer development process is present, no test worker consumes sustained CPU, and the worktree remains clean.

- [ ] **Step 7: Hand off physical retesting without overstating evidence**

Ask the user to verify in the running app:

1. move onto the image, stop, then press Q; the lens appears immediately without another movement;
2. the arrow remains visible and precisely selects the sampled pixel;
3. the shell follows 18 px beside the pointer and flips near all four edges;
4. Q/button, image exit/re-entry, navigation persistence, and original failure remain correct;
5. circle/rounded rectangle, small/medium/large, and 1.5x/2x/3x work with 1.5x default;
6. enter/retract motion is smooth and reduced-motion mode is immediate;
7. physical pinch/pan behavior from the original release gate still passes.

Do not create or commit final hardware acceptance evidence until the user reports these checks.
