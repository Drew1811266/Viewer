# Viewer Five-Level Thumbnail Slider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the three named thumbnail-density radio cards with one accessible five-stop slider and extend Viewer end to end to the approved 96/132/168/204/240 CSS-pixel thumbnail sizes.

**Architecture:** Preserve `ThumbnailDensity` as the typed cross-layer persistence contract and add `extra_large` and `maximum`. The UI alone maps visible levels 1–5 to typed density values. Existing optimistic, serialized settings persistence remains unchanged; all layout consumers continue receiving a typed density and a derived height. The product, formal visual atlas, browser acceptance catalog and native acceptance recipe must expose the same five states.

**Tech Stack:** React 19, TypeScript, Vitest/Testing Library, CSS range pseudo-elements, Rust/Serde, Tauri 2, JSDOM visual-atlas tests, browser and macOS-native acceptance runners.

## Global Constraints

- Implement the approved specification in `docs/superpowers/specs/2026-08-06-viewer-five-level-thumbnail-slider-design.md` without redesigning other settings.
- Keep `schemaVersion: 1`, the `thumbnailDensity` JSON key and the `update_thumbnail_density` IPC command.
- Preserve `standard` as the first-run, malformed-file and unsupported-value recovery default.
- Use a real `input[type="range"]`; do not build a simulated slider or make the visible numbers interactive.
- Keep the existing optimistic serialized write queue. A later selection wins; the latest failed save rolls the slider and layout back together.
- Do not change system display scaling, screen resolution, Dock visibility or other macOS preferences during testing.
- Run bounded targeted tests after each task. Do not use repeated open/close or screenshot loops.
- Preserve unrelated working-tree changes if any appear during execution.

---

### Task 1: Add the five-level TypeScript model and pure mappings

**Files:**

- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/settings/thumbnailDensity.ts`
- Create: `ui/src/settings/thumbnailDensity.test.ts`

**Interfaces:**

```ts
export type ThumbnailDensity =
  | 'compact'
  | 'standard'
  | 'large'
  | 'extra_large'
  | 'maximum'

export type ThumbnailLevel = 1 | 2 | 3 | 4 | 5

export interface ThumbnailLevelOption {
  level: ThumbnailLevel
  density: ThumbnailDensity
  height: number
}

export const THUMBNAIL_LEVELS: readonly ThumbnailLevelOption[]
export function thumbnailLevelForDensity(density: ThumbnailDensity): ThumbnailLevel
export function thumbnailDensityForLevel(level: number): ThumbnailDensity
export function thumbnailLevelValueText(level: ThumbnailLevel): string
```

- [ ] **Step 1: Write the failing mapping and request-bound tests**

Create `ui/src/settings/thumbnailDensity.test.ts` with exact mapping coverage:

```ts
import { describe, expect, it } from 'vitest'
import {
  THUMBNAIL_HEIGHT,
  THUMBNAIL_LEVELS,
  thumbnailDensityForLevel,
  thumbnailLevelForDensity,
  thumbnailLevelValueText,
  thumbnailRequestSize,
} from './thumbnailDensity'

describe('thumbnail density levels', () => {
  it('maps every visible level to one typed density and exact height', () => {
    expect(THUMBNAIL_LEVELS).toEqual([
      { level: 1, density: 'compact', height: 96 },
      { level: 2, density: 'standard', height: 132 },
      { level: 3, density: 'large', height: 168 },
      { level: 4, density: 'extra_large', height: 204 },
      { level: 5, density: 'maximum', height: 240 },
    ])
    expect(THUMBNAIL_HEIGHT).toEqual({
      compact: 96,
      standard: 132,
      large: 168,
      extra_large: 204,
      maximum: 240,
    })
  })

  it.each(THUMBNAIL_LEVELS)('round-trips level $level', ({ level, density, height }) => {
    expect(thumbnailDensityForLevel(level)).toBe(density)
    expect(thumbnailLevelForDensity(density)).toBe(level)
    expect(thumbnailLevelValueText(level)).toBe(`档位 ${level}，${height} 像素`)
  })

  it('rejects values that are not one of the five discrete stops', () => {
    expect(() => thumbnailDensityForLevel(0)).toThrow(RangeError)
    expect(() => thumbnailDensityForLevel(6)).toThrow(RangeError)
    expect(() => thumbnailDensityForLevel(2.5)).toThrow(RangeError)
  })

  it('keeps the maximum DPR request below the existing safety cap', () => {
    expect(thumbnailRequestSize(240, 240, 4)).toEqual({ maxPixels: 960, scaleMilli: 4_000 })
  })
})
```

- [ ] **Step 2: Run the new test and confirm RED**

Run:

```bash
pnpm --dir ui exec vitest run src/settings/thumbnailDensity.test.ts
```

Expected: FAIL because the two new density values and mapping exports do not exist.

- [ ] **Step 3: Implement the typed five-level table as the single source of truth**

Extend `ThumbnailDensity` in `ui/src/api/types.ts`. In `ui/src/settings/thumbnailDensity.ts`, define the ordered table exactly once, derive `THUMBNAIL_HEIGHT`, and implement lookup helpers that throw `RangeError` for invalid numeric values. Do not silently clamp arbitrary input.

```ts
export const THUMBNAIL_LEVELS = [
  { level: 1, density: 'compact', height: 96 },
  { level: 2, density: 'standard', height: 132 },
  { level: 3, density: 'large', height: 168 },
  { level: 4, density: 'extra_large', height: 204 },
  { level: 5, density: 'maximum', height: 240 },
] as const satisfies readonly ThumbnailLevelOption[]
```

Keep `MAX_THUMBNAIL_DEVICE_SCALE = 4`, `MAX_THUMBNAIL_PHYSICAL_EDGE = 4096` and the existing request-size algorithm unchanged.

- [ ] **Step 4: Run the focused TypeScript tests and type check**

Run:

```bash
pnpm --dir ui exec vitest run src/settings/thumbnailDensity.test.ts
pnpm --dir ui check
```

Expected: PASS; any exhaustiveness errors elsewhere identify the exact downstream files handled in later tasks, but `thumbnailDensity.test.ts` must pass before continuing.

- [ ] **Step 5: Commit the level model**

```bash
git add ui/src/api/types.ts ui/src/settings/thumbnailDensity.ts ui/src/settings/thumbnailDensity.test.ts
git commit -m "feat: add five thumbnail size levels"
```

---

### Task 2: Extend Rust persistence, desktop DTO and the TypeScript bridge boundary

**Files:**

- Modify: `crates/viewer-application/src/settings.rs`
- Modify: `crates/viewer-infrastructure/src/settings.rs`
- Modify: `src-tauri/src/dto/settings.rs`
- Modify: `ui/src/api/viewer.test.ts`

**Interfaces:**

```rust
#[serde(rename_all = "snake_case")]
pub enum ThumbnailDensity {
    Compact,
    Standard,
    Large,
    ExtraLarge,
    Maximum,
}
```

The DTO must expose the same five variants and serialize the additions as `extra_large` and `maximum`.

- [ ] **Step 1: Write failing application, storage, DTO and bridge tests**

Add tests that prove:

- `ViewerSettings::default()` is still `Standard`.
- the application service saves and returns `ExtraLarge` and `Maximum`.
- existing version-one JSON values `compact`, `standard` and `large` load unchanged.
- version-one `extra_large` and `maximum` values round-trip through `JsonViewerSettingsStore` with the same JSON shape.
- the desktop DTO serializes/deserializes all five exact snake-case public values and rejects `huge`, `thumbnail_density` and `dense`.
- `tauriViewerBridge.updateThumbnailDensity('maximum')` invokes:

```ts
expect(invoke).toHaveBeenCalledWith('update_thumbnail_density', { density: 'maximum' })
```

- [ ] **Step 2: Run the boundary tests and confirm RED**

```bash
cargo test --locked -p viewer-application settings
cargo test --locked -p viewer-infrastructure settings
cargo test --locked -p viewer-desktop dto::settings
pnpm --dir ui exec vitest run src/api/viewer.test.ts
```

Expected: FAIL on missing `ExtraLarge`/`Maximum` variants.

- [ ] **Step 3: Add the two enum and DTO variants without changing schema shape**

Update both Rust enums and both conversion `match` expressions exhaustively. Do not alter `StoredViewerSettings`, `ViewerSettingsDto`, the Tauri command signature or `VIEWER_SETTINGS_SCHEMA_VERSION`.

For storage tests, use a table so all accepted version-one strings are proven:

```rust
for (serialized, expected) in [
    ("compact", ThumbnailDensity::Compact),
    ("standard", ThumbnailDensity::Standard),
    ("large", ThumbnailDensity::Large),
    ("extra_large", ThumbnailDensity::ExtraLarge),
    ("maximum", ThumbnailDensity::Maximum),
] {
    // write the exact schemaVersion-1 document, then assert load() == expected
}
```

- [ ] **Step 4: Re-run the focused boundary suite**

Run the four commands from Step 2. Expected: PASS.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt --all
git add crates/viewer-application/src/settings.rs crates/viewer-infrastructure/src/settings.rs src-tauri/src/dto/settings.rs ui/src/api/viewer.test.ts
git commit -m "feat: persist five thumbnail sizes"
```

---

### Task 3: Replace the settings radios with the accessible five-stop slider

**Files:**

- Modify: `ui/src/components/SettingsDialog.tsx`
- Modify: `ui/src/components/SettingsDialog.test.tsx`
- Modify: `ui/src/settings/ViewerSettingsProvider.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**UI contract:**

```tsx
<input
  type="range"
  name="thumbnail-size"
  min={1}
  max={5}
  step={1}
  value={level}
  aria-label="缩略图大小"
  aria-valuetext={`档位 ${level}，${height} 像素`}
/>
```

- [ ] **Step 1: Replace radio expectations with failing slider tests**

In `SettingsDialog.test.tsx`, assert at both 1024×720 and 720×450 that the slider and Close button are visible. In the interaction test assert:

```ts
const slider = within(dialog).getByRole('slider', { name: '缩略图大小' })
expect(slider).toHaveAttribute('min', '1')
expect(slider).toHaveAttribute('max', '5')
expect(slider).toHaveAttribute('step', '1')
expect(slider).toHaveValue('2')
expect(slider).toHaveAttribute('aria-valuetext', '档位 2，132 像素')
expect(within(dialog).queryAllByRole('radio')).toHaveLength(0)
for (const oldLabel of ['紧凑', '标准', '大图']) {
  expect(within(dialog).queryByText(oldLabel)).not.toBeInTheDocument()
}
fireEvent.change(slider, { target: { value: '5' } })
expect(onDensityChange).toHaveBeenCalledWith('maximum')
```

Assert the presentational number row contains exactly `1`, `2`, `3`, `4`, `5`, current `2` is marked, and the slider receives initial focus. Preserve the existing Escape, focus trap, Close button and focus restoration assertions.

In `ViewerSettingsProvider.test.tsx`, expose buttons for all five typed densities, change the immediate-update case to `maximum`/240, serialize `extra_large` then `maximum`, and verify a failed latest `maximum` write restores the last confirmed density and height.

In `app.test.ts`, add style-contract assertions for the 4px WebKit/Chromium track, 18px circular thumb, minimum 32px input height, five-number alignment, visible focus, reduced motion and forced colours.

- [ ] **Step 2: Run the UI tests and confirm RED**

```bash
pnpm --dir ui exec vitest run src/components/SettingsDialog.test.tsx src/settings/ViewerSettingsProvider.test.tsx src/styles/app.test.ts
```

Expected: FAIL because the dialog still exposes three radios and the slider styles do not exist.

- [ ] **Step 3: Implement the semantic slider and pure mapping call**

Remove `DENSITY_OPTIONS`. Import `THUMBNAIL_LEVELS`, `thumbnailDensityForLevel`, `thumbnailLevelForDensity` and `thumbnailLevelValueText`. Compute the level from the typed density, use one `useRef<HTMLInputElement>` for initial focus, and render the five numbers with `aria-hidden="true"`.

Set a custom progress property without adding component state:

```tsx
const level = thumbnailLevelForDensity(density)
const progress = ((level - 1) / (THUMBNAIL_LEVELS.length - 1)) * 100

<fieldset className="thumbnail-size-control">
  <legend>缩略图大小</legend>
  <input
    ref={thumbnailSizeRef}
    className="thumbnail-size-slider"
    type="range"
    name="thumbnail-size"
    min={1}
    max={5}
    step={1}
    value={level}
    aria-label="缩略图大小"
    aria-valuetext={thumbnailLevelValueText(level)}
    style={{ '--thumbnail-level-progress': `${progress}%` } as React.CSSProperties}
    onChange={(event) => onDensityChange(thumbnailDensityForLevel(Number(event.currentTarget.value)))}
  />
  <div className="thumbnail-size-levels" aria-hidden="true">
    {THUMBNAIL_LEVELS.map((option) => (
      <span key={option.level} data-current={option.level === level || undefined}>
        {option.level}
      </span>
    ))}
  </div>
</fieldset>
```

Use a direct `CSSProperties` type import if preferred; do not introduce `any`.

- [ ] **Step 4: Replace radio-card CSS with the approved range styling**

Delete `.density-options` rules. Add:

```css
.thumbnail-size-control {
  border: 0;
  margin: 0;
  padding: 0;
}

.thumbnail-size-control legend {
  color: var(--viewer-text-secondary);
  font-size: 12px;
  font-weight: 650;
  margin-bottom: var(--viewer-space-2);
  padding: 0;
}

.thumbnail-size-slider {
  --thumbnail-level-progress: 25%;
  appearance: none;
  background: transparent;
  height: 32px;
  margin: 0;
  width: 100%;
}

.thumbnail-size-slider::-webkit-slider-runnable-track {
  background: linear-gradient(to right, var(--viewer-accent) 0 var(--thumbnail-level-progress), var(--viewer-control-border) var(--thumbnail-level-progress) 100%);
  border-radius: 999px;
  height: 4px;
}

.thumbnail-size-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  background: var(--viewer-accent);
  border: 2px solid var(--viewer-surface);
  border-radius: 50%;
  box-shadow: 0 0 0 1px var(--viewer-accent);
  height: 18px;
  margin-top: -7px;
  width: 18px;
}

.thumbnail-size-levels {
  display: flex;
  justify-content: space-between;
  padding-inline: 9px;
}
```

Add equivalent `::-moz-range-track`, `::-moz-range-progress` and `::-moz-range-thumb` rules for development-browser parity. Use the existing focus token for `:focus-visible`. Under `prefers-reduced-motion: reduce`, remove slider transition. Under `forced-colors: active`, use `Canvas`, `CanvasText` and `Highlight` and keep the focus outline visible.

- [ ] **Step 5: Re-run UI tests and the UI check**

```bash
pnpm --dir ui exec vitest run src/components/SettingsDialog.test.tsx src/settings/ViewerSettingsProvider.test.tsx src/styles/app.test.ts
pnpm --dir ui check
```

Expected: PASS. Confirm the dialog contains one slider and zero radios.

- [ ] **Step 6: Commit the settings UI**

```bash
git add ui/src/components/SettingsDialog.tsx ui/src/components/SettingsDialog.test.tsx ui/src/settings/ViewerSettingsProvider.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: add thumbnail size slider"
```

---

### Task 4: Prove real layout, selection, virtualization and request behavior at levels 4 and 5

**Files:**

- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/AspectVirtualGrid.test.tsx`
- Modify: `ui/src/layout/aspectLayout.test.ts`

- [ ] **Step 1: Extend the existing density table and state-preservation tests**

Extend the `ContentBrowser.test.tsx` table to:

```ts
it.each([
  ['compact', 96],
  ['standard', 132],
  ['large', 168],
  ['extra_large', 204],
  ['maximum', 240],
] satisfies [ThumbnailDensity, number][])(/* existing geometry assertion */)
```

Change the density reflow test to rerender from `standard` to `maximum`, then assert the selected entity, `aria-activedescendant`, preview identity and a 240px image surface survive. Add a maximum-DPR request assertion proving a square level-5 surface requests 960 physical pixels and never exceeds 4096.

- [ ] **Step 2: Add bounded maximum-size geometry tests**

In `aspectLayout.test.ts`, build flow and filmstrip geometries with `imageHeight = 240` and assert every item is finite, source ordered, within the scroll extent and uses an exact 240px image height.

In `AspectVirtualGrid.test.tsx`, rerender a selected/anchored grid from 132-equivalent geometry to 240-equivalent geometry and assert:

- the first-visible key keeps its visual offset;
- the active item remains mounted;
- the mounted option count remains below the fixture size with existing overscan;
- scroll clamping converges after the larger rows reduce items per viewport.

- [ ] **Step 3: Run the focused layout tests**

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx src/components/AspectVirtualGrid.test.tsx src/layout/aspectLayout.test.ts src/settings/thumbnailDensity.test.ts
```

Expected: PASS without production changes beyond the new central height table. If a layout assertion reveals a genuine existing hard-coded three-value branch, change only that branch and add the failing regression first.

- [ ] **Step 4: Commit layout coverage**

```bash
git add ui/src/components/ContentBrowser.test.tsx ui/src/components/AspectVirtualGrid.test.tsx ui/src/layout/aspectLayout.test.ts
git commit -m "test: cover larger thumbnail layouts"
```

---

### Task 5: Bring the formal visual atlas to exact product parity

**Files:**

- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Modify: `ui/src/visualAtlas.test.ts`

- [ ] **Step 1: Write failing atlas parity tests**

Add assertions that the atlas contains:

- five browser density states with internal keys `compact`, `standard`, `large`, `extra_large`, `maximum` and visible labels `1`, `2`, `3`, `4`, `5`;
- 204px and 240px image stages;
- proportional card bases 306px and 360px for the two new sizes;
- one settings `input[type="range"]` with min 1, max 5, step 1, value 2, `aria-label="缩略图大小"`, all five visible number labels and no old named radio control;
- focus and current-stop styling matching the product selectors.

Extend the runtime atlas test to click `density-extra-large` and `density-maximum` and assert the grid's `data-density` value.

- [ ] **Step 2: Run the atlas test and confirm RED**

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: FAIL because the atlas currently models only compact/standard/large and a select-based settings mock.

- [ ] **Step 3: Update atlas geometry and settings markup**

Add exact geometry rules:

```css
.image-grid[data-density="extra_large"] .image-card { flex-basis: 306px; }
.image-grid[data-density="extra_large"] .image-stage { height: 204px; }
.image-grid[data-density="maximum"] .image-card { flex-basis: 360px; }
.image-grid[data-density="maximum"] .image-stage { height: 240px; }
```

Replace the generic settings select mock with the same one-category dialog and semantic five-stop slider used in the product. Use the same 4px track, 18px thumb, number alignment, current-stop accent and focus ring. Remove only user-facing legacy size labels from the thumbnail setting; do not rewrite historical design-direction prose elsewhere in the atlas.

- [ ] **Step 4: Re-run atlas tests**

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit atlas parity**

```bash
git add docs/prototypes/viewer-complete-ui-visual-atlas.html ui/src/visualAtlas.test.ts
git commit -m "docs: model five thumbnail sizes in atlas"
```

---

### Task 6: Expand browser and native acceptance from 89 to 91 authoritative states

**Files:**

- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.test.ts`
- Modify: `ui/src/acceptance/scenes/workspaceScenes.tsx`
- Modify: `ui/src/acceptance/scenes/feedbackScenes.tsx`
- Modify: `scripts/viewer-native-acceptance.mjs`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`

**New stable states:**

```json
{ "id": "THU-08", "wave": 1, "referenceState": "density-extra-large", "sceneGroup": "workspace", "components": ["AspectVirtualGrid"] }
{ "id": "THU-09", "wave": 1, "referenceState": "density-maximum", "sceneGroup": "workspace", "components": ["AspectVirtualGrid"] }
```

- [ ] **Step 1: Add failing catalog and recipe tests first**

Update exact catalog cardinality from 89 to 91 and extend native recipe assertions so:

- THU-01/02/03 normalize slider levels 1/2/3;
- THU-08/09 normalize levels 4/5;
- all five recipes open `衣服/A01` after setting the level;
- the A11Y settings scene waits for the focused control named `thumbnail-size` rather than `thumbnail-density`;
- `--smoke` still does not expand to the complete 91-state controller.

Run:

```bash
pnpm --dir ui exec vitest run src/acceptance/acceptanceStateCatalog.test.ts
node --test scripts/viewer-native-acceptance.test.mjs
```

Expected: RED until both authoritative documents and runners include THU-08/09.

- [ ] **Step 2: Add THU-08/09 to the browser scene registry and density resolver**

Register both IDs in `WORKSPACE_SCENES`. Replace the nested density ternary with an explicit helper or lookup map whose exhaustive cases are:

```ts
THU-01 -> compact
THU-02 -> standard
THU-03 -> large
THU-08 -> extra_large
THU-09 -> maximum
```

Include THU-08/09 in the resting-content readiness branch. Leave THU-04..07 selection meanings unchanged.

- [ ] **Step 3: Change native normalization from radio names to slider levels**

Rename recipe fields from `density` to `thumbnailLevel`, defaulting to 2. In `normalizeWorkspaceState`, open Settings, find `{ role: 'AXSlider', name: '缩略图大小' }`, focus it, press Home, then send `thumbnailLevel - 1` ArrowRight keys. Query the slider again and assert its numeric value is the requested stop before closing Settings.

The new recipe matrix must use:

```js
'THU-01': openSettingsAtLevel(1),
'THU-02': openSettingsAtLevel(2),
'THU-03': openSettingsAtLevel(3),
'THU-08': openSettingsAtLevel(4),
'THU-09': openSettingsAtLevel(5),
```

Do not click the range track at a guessed coordinate; Home plus ArrowRight produces deterministic native state at both viewports.

- [ ] **Step 4: Add matching authoritative ledger rows**

Add THU-08 and THU-09 immediately after THU-07 in `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`, with the exact approved reference states, component, user recipe and targeted tests. Change `numberedIds('THU', 7)` to `numberedIds('THU', 9)` in the native runner so its authoritative ID set stays numeric and complete. Keep the historical 89-state evidence paragraph intact; append a dated 2026-08-06 note explaining that the active catalog is now 91 states because levels 4 and 5 were added. Until Task 8 runs, use an explicit `pending — five-level acceptance scheduled in Task 8` evidence status; Task 8 must replace it with the actual browser acceptance manifest/combined-image paths, never invented paths.

- [ ] **Step 5: Re-run catalog, acceptance-scene and native-runner tests**

```bash
pnpm --dir ui exec vitest run src/acceptance/acceptanceStateCatalog.test.ts src/acceptance/AcceptanceApp.test.tsx src/acceptance/scenes/workspaceScenes.test.tsx
node --test scripts/viewer-native-acceptance.test.mjs
```

If a listed scene test file is not present, run `rg --files ui/src/acceptance | sort` and use the existing acceptance-scene test files; do not create a redundant harness solely to match this command.

- [ ] **Step 6: Commit authoritative acceptance coverage**

```bash
git add ui/src/acceptance scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "test: cover all five thumbnail sizes visually"
```

---

### Task 7: Update current product documentation

**Files:**

- Modify: `README.md`
- Modify: `docs/README.md`
- Modify only if the current documentation index requires it: `docs/design-qa.md`

- [ ] **Step 1: Replace current three-choice wording**

Change the current README capability copy from named three-density wording to:

```md
- **自适应缩略图**：按图片原始宽高比展示缩略图，支持 1–5 五档全局缩略图大小设置。
```

Ensure the approved design spec and this implementation plan remain linked from `docs/README.md`. Do not rewrite older historical review descriptions that correctly document the former three-level implementation.

- [ ] **Step 2: Check current documentation for stale product claims**

```bash
rg -n "缩略图密度|紧凑、标准和大图|三档全局" README.md docs --glob '!reviews/**' --glob '!superpowers/specs/2026-07-27-viewer-aspect-aware-thumbnail-density-design.md'
```

Expected: no stale current-product claim. Historical and superseded documents may retain their original wording when clearly dated.

- [ ] **Step 3: Commit documentation**

```bash
git add README.md docs/README.md docs/design-qa.md
git commit -m "docs: describe five thumbnail size levels"
```

If `docs/design-qa.md` was not changed, omit it from `git add`.

---

### Task 8: Run final bounded verification and record real visual evidence

**Files:**

- Modify with actual evidence paths: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify with the completed result if used by the existing process: `docs/design-qa.md`

- [ ] **Step 1: Run all focused UI, native-runner and Rust verification once**

```bash
pnpm --dir ui exec vitest run src/settings/thumbnailDensity.test.ts src/components/SettingsDialog.test.tsx src/settings/ViewerSettingsProvider.test.tsx src/components/ContentBrowser.test.tsx src/components/AspectVirtualGrid.test.tsx src/layout/aspectLayout.test.ts src/styles/app.test.ts src/visualAtlas.test.ts src/acceptance/acceptanceStateCatalog.test.ts
node --test scripts/viewer-native-acceptance.test.mjs
cargo test --locked -p viewer-application settings
cargo test --locked -p viewer-infrastructure settings
cargo test --locked -p viewer-desktop dto::settings
```

Expected: every command exits 0. Fix a failure from its first concrete cause; do not rerun unaffected commands in a loop.

- [ ] **Step 2: Run the repository-wide gate once**

```bash
pnpm verify
```

Expected: policy, formatting, type checks, UI tests, builds, Clippy, Rust tests and security checks all pass.

- [ ] **Step 3: Capture the bounded browser visual matrix**

Use only the slider/settings, five resting sizes and maximum selection states:

```bash
pnpm accept:visual --id DIA-01 --id A11Y-01 --id THU-01 --id THU-02 --id THU-03 --id THU-08 --id THU-09 --id THU-05 --output-root target/viewer-visual-acceptance/five-level-thumbnail-slider
```

Verify 1024×720 and 1440×900 output for each state. Inspect the settings slider, number alignment, current-stop emphasis, keyboard focus, 204/240px content geometry, clipping/overflow, and the complete square selection outline at maximum size.

- [ ] **Step 4: Run one bounded native matrix**

The native runner intentionally accepts one selector and one viewport per invocation. Run only these two representative native states rather than the full 91-state suite:

```bash
pnpm accept:native --id DIA-01 --viewport 1440x900
pnpm accept:native --id THU-09 --viewport 1024x720
```

Use the existing development launch mechanism. Confirm exactly one Viewer app process/window is used. Do not alter system display or Dock settings. Stop after this single matrix; rerun only a failed state with evidence of a product or harness defect.

- [ ] **Step 5: Record exact evidence and re-run policy checks**

Use the actual commit directory and manifest paths printed by the acceptance runner to fill THU-08/09 ledger cells. Record pass/fail truthfully; never manufacture a hash or screenshot path. If the current design-QA log records feature acceptance, append one dated line with the exact output root and both viewports.

```bash
pnpm test:policy
git diff --check
rg -n "TODO|TBD|PLACEHOLDER|<commit>|<hash>" README.md docs ui/src scripts crates src-tauri
```

Expected: policy passes, diff check is clean and no feature placeholder remains.

- [ ] **Step 6: Self-review specification coverage**

Verify each approved requirement directly:

- levels 1–5 map to 96/132/168/204/240;
- visible setting contains one slider, five numbers and no named radio choices;
- native pointer, Home/End and arrow semantics work;
- `aria-valuetext` reports the level and pixels;
- level 2 remains default/recovery;
- old version-one values load unchanged;
- new values round-trip across TypeScript, Tauri DTO, application and JSON storage;
- optimistic latest-wins and rollback behavior passes;
- product and atlas match;
- THU-08/09 exist without reusing THU-04..07;
- both accepted viewports show no clipping, overflow or incomplete selection outline;
- no unrelated UI, system settings or project file behavior changed.

- [ ] **Step 7: Commit final evidence**

```bash
git add docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/design-qa.md
git commit -m "test: record five-level thumbnail acceptance"
```

If only the ledger changed, omit `docs/design-qa.md` from `git add`.

---

## Completion Handoff

After every checkbox is complete, present:

- the exact five-level mapping;
- focused and full verification results;
- browser and native acceptance output roots;
- any remaining platform limitation, especially that Windows-native validation waits for the Windows build;
- the final commit range and working-tree status.

Do not claim completion if THU-08/09 evidence is absent, if the real settings control still contains radios, or if the newest save can leave slider and content at different levels.
