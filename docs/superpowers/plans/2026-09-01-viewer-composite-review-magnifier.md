# Viewer Composite Review Magnifier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the image magnifier render image-anchored review rectangles, brush strokes, ordinals, and live draft geometry through the same source-space projection as the magnified image.

**Architecture:** Keep the generic preview independent from review domain types by adding a narrow optional `MagnifierOverlayPainter` port. Extract a normalized, read-only annotation scene and shared canvas painter from `AnnotationCanvas`; the ordinary stage and the magnifier consume that same scene through different projections, while the existing DOM marker/handle layer remains the sole interaction owner.

**Tech Stack:** React 19, TypeScript 6, Canvas 2D, CSS custom properties, Vitest, Testing Library, Viewer visual acceptance harness, Biome, and Vite.

**Spec:** `docs/superpowers/specs/2026-09-01-viewer-composite-review-magnifier-design.md`

## Global Constraints

- The magnifier observes one composite image-and-review presentation; do not implement annotation-aware lens clipping or obstacle avoidance.
- `ImagePreviewSurface` and `ImageMagnifier` must not import review Controller, Feedback DTO, archive, history, or persistence types.
- Normalized image anchors remain the only source of review geometry; never derive magnifier annotations from DOM bounds or screenshots.
- Rectangles, strokes, and transient geometry use the same source sample, scale, rotation, and frame as the magnified image.
- Geometry scales with the image; canvas stroke width is clamped to 2–5 CSS px and ordinal badges remain a fixed readable size.
- Asset-level feedback is excluded from the lens because it has no image geometry.
- The lens remains `pointer-events: none`, `aria-hidden`, bounded by the existing shell placement, and hidden outside actual transformed image pixels.
- The magnifier layer is above ordinary annotations and below the inline feedback editor and modal surfaces.
- Painter or Canvas failure degrades to the current image-only lens and must not affect review writes or ordinary annotation editing.
- Do not change Feedback, Anchor, snapshot, archive, history, or Agent reader protocols.
- Add no runtime dependency and do not allocate an original-image-sized annotation canvas.
- Each production change follows an observed focused-test failure.
- Viewer remains in early development; signing, notarization, formal installers, listing, release, and sales are not plan, acceptance, or completion tasks.

## File Structure

### New files

- `ui/src/components/imagePreview/magnifierOverlay.ts` — generic painter/frame contract shared by `ImageMagnifier` and optional preview consumers.
- `ui/src/components/review/annotationScene.ts` — normalized review scene construction plus projection-independent Canvas rendering.
- `ui/src/components/review/annotationScene.test.ts` — scene filtering, geometry, transient state, line-width, and ordinal rendering tests.

### Modified files

- `ui/src/components/imagePreview/magnifierGeometry.ts` and `.test.ts` — pure normalized-source to lens-local projection.
- `ui/src/components/imagePreview/ImageMagnifier.tsx` and `.test.tsx` — high-DPI overlay Canvas, frame-coalesced painter invocation, and safe fallback.
- `ui/src/components/imagePreview/ImagePreviewSurface.tsx` and `.test.tsx` — optional painter slot plus capture-phase magnifier sampling.
- `ui/src/components/review/AnnotationCanvas.tsx` and `.test.tsx` — reuse the shared annotation scene for ordinary stage geometry while retaining DOM interactions.
- `ui/src/components/review/ImageReviewWorkspace.tsx` and `.test.tsx` — build one scene snapshot and inject its lens painter.
- `ui/src/components/review/ImageReviewWorkbench.integration.test.tsx` — live drawing and review-magnifier integration.
- `ui/src/styles/app.css`, `ui/src/styles/review.css`, and `ui/src/styles/app.test.ts` — composite-layer stacking, clipping, theme token, and non-interactive overlay rules.
- `ui/src/acceptance/acceptanceStateCatalog.json`, `acceptanceStateCatalog.test.ts`, `scenes/reviewScenes.tsx`, `scenes/reviewScenes.test.tsx`, `scenes/reviewWorkbenchScenes.ts`, and `scenes/reviewWorkbenchScenes.test.tsx` — deterministic `RVW-33` composite magnifier state.
- `docs/product/FEATURE_REFERENCE.md` and `docs/product/USER_GUIDE.md` — current user-visible composite magnifier behavior.

---

### Task 1: Define the pure magnifier content projection

**Files:**
- Modify: `ui/src/components/imagePreview/magnifierGeometry.ts`
- Test: `ui/src/components/imagePreview/magnifierGeometry.test.ts`

**Interfaces:**
- Consumes: existing `Point`, `Size`, and `PreviewRotation` from `imageGeometry.ts`.
- Produces:

```ts
export interface MagnifierContentProjection {
  lensSize: Size
  normalizedToLens(point: Point): Point | null
}

export function createMagnifierContentProjection(input: {
  sourceSize: Size
  sourcePoint: Point
  lensSize: Size
  contentScale: number
  rotation: PreviewRotation
}): MagnifierContentProjection
```

- [ ] **Step 1: Write failing projection tests for scale and all rotations**

Add table-driven tests that keep the sampled source point at the lens center and map one normalized point for each rotation:

```ts
it.each([
  [0, { x: 140, y: 80 }],
  [90, { x: 120, y: 100 }],
  [180, { x: 100, y: 80 }],
  [270, { x: 120, y: 60 }],
] as const)('projects normalized geometry at %i°', (rotation, expected) => {
  const projection = createMagnifierContentProjection({
    sourceSize: { width: 100, height: 50 },
    sourcePoint: { x: 50, y: 25 },
    lensSize: { width: 240, height: 160 },
    contentScale: 2,
    rotation,
  })
  expect(projection.normalizedToLens({ x: 0.6, y: 0.5 })).toEqual(expected)
  expect(projection.normalizedToLens({ x: 0.5, y: 0.5 })).toEqual({ x: 120, y: 80 })
})
```

Add invalid-input cases for non-finite normalized points and non-positive source/lens sizes; `normalizedToLens` must return `null` instead of producing CSS/Canvas `NaN`.

- [ ] **Step 2: Run the focused geometry test and observe the missing export**

Run:

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/magnifierGeometry.test.ts
```

Expected: FAIL because `createMagnifierContentProjection` is not exported.

- [ ] **Step 3: Implement one shared source-to-lens transform**

Implement the projection with explicit quarter-turn rotation after applying uniform content scale:

```ts
function rotate(delta: Point, rotation: PreviewRotation): Point {
  if (rotation === 90) return { x: -delta.y, y: delta.x }
  if (rotation === 180) return { x: -delta.x, y: -delta.y }
  if (rotation === 270) return { x: delta.y, y: -delta.x }
  return delta
}
```

Convert normalized coordinates to original source pixels, subtract `sourcePoint`, multiply by `contentScale`, rotate once, and add `lensSize / 2`. Keep shell placement unchanged.

- [ ] **Step 4: Run geometry tests and confirm green**

Run the command from Step 2. Expected: all magnifier geometry tests pass, including existing shell placement and source-origin contracts.

- [ ] **Step 5: Commit the projection boundary**

```bash
git add ui/src/components/imagePreview/magnifierGeometry.ts ui/src/components/imagePreview/magnifierGeometry.test.ts
git commit -m "feat: project review geometry into magnifier"
```

---

### Task 2: Extract one normalized annotation scene and painter

**Files:**
- Create: `ui/src/components/review/annotationScene.ts`
- Create: `ui/src/components/review/annotationScene.test.ts`
- Modify: `ui/src/components/review/AnnotationCanvas.tsx`
- Test: `ui/src/components/review/AnnotationCanvas.test.tsx`

**Interfaces:**
- Consumes: `SavedImageFeedback`, `ReviewAnchor`, and a transient anchor selected by the workbench.
- Produces:

```ts
export type AnnotationSceneAppearance = 'saved' | 'selected' | 'transient'

export interface AnnotationSceneItem {
  itemId: string | null
  ordinal: number | null
  anchor: ReviewAnchor
  appearance: AnnotationSceneAppearance
}

export interface AnnotationSceneProjection {
  normalizedToLocal(point: Point): Point | null
}

export interface AnnotationPaintOptions {
  color: string
  lineWidth: number
  drawOrdinals: boolean
  ordinalRadius: number
}

export function buildAnnotationScene(input: {
  feedback: ReadonlyArray<SavedImageFeedback>
  selectedItemId: string | null
  transientAnchor: ReviewAnchor | null
}): ReadonlyArray<AnnotationSceneItem>

export function paintAnnotationScene(
  context: CanvasRenderingContext2D,
  scene: ReadonlyArray<AnnotationSceneItem>,
  projection: AnnotationSceneProjection,
  options: AnnotationPaintOptions,
): number
```

- [ ] **Step 1: Write failing scene-construction tests**

Cover image rectangles, image strokes, asset feedback exclusion, selected appearance, and a dashed transient item:

```ts
expect(
  buildAnnotationScene({
    feedback: [rectFeedback, strokeFeedback, assetFeedback],
    selectedItemId: strokeFeedback.itemId,
    transientAnchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
  }),
).toMatchObject([
  { itemId: rectFeedback.itemId, ordinal: 1, appearance: 'saved' },
  { itemId: strokeFeedback.itemId, ordinal: 2, appearance: 'selected' },
  { itemId: null, ordinal: null, appearance: 'transient' },
])
```

- [ ] **Step 2: Write failing painter tests**

Use a mocked `CanvasRenderingContext2D` and a projection that maps `{x, y}` to `{x: x * 200, y: y * 100}`. Assert:

- rectangle uses `strokeRect` with projected dimensions;
- stroke uses `moveTo`/`lineTo` and one `stroke`;
- transient geometry applies `[6, 4]` dash and restores context;
- `drawOrdinals: false` does not call `arc` or `fillText`;
- `drawOrdinals: true` draws a fixed `ordinalRadius` badge at the same projected marker anchor used by the ordinary DOM marker;
- the return value equals the number of image-anchored items actually painted.

- [ ] **Step 3: Run focused tests and observe the missing module**

```bash
pnpm --dir ui exec vitest run src/components/review/annotationScene.test.ts src/components/review/AnnotationCanvas.test.tsx
```

Expected: FAIL because `annotationScene.ts` and the shared interfaces do not exist.

- [ ] **Step 4: Implement scene filtering and projection-independent drawing**

Move the existing `drawAnchor`, rectangle marker-point, and stroke endpoint logic out of `AnnotationCanvas.tsx`. Preserve the current deterministic order and ignore `asset` anchors. The painter must never mutate anchors or feedback arrays.

- [ ] **Step 5: Make the ordinary annotation canvas consume the shared painter**

Build the current scene once per render, then paint ordinary geometry with:

```ts
paintAnnotationScene(context, scene, stageProjection, {
  color: annotationColor(element),
  lineWidth: 2,
  drawOrdinals: false,
  ordinalRadius: 14,
})
```

Keep `AnnotationMarker`, rectangle handles, pointer capture, keyboard adjustment, and controller commands unchanged.

- [ ] **Step 6: Run focused tests and confirm the stage has no behavioral regression**

Run the command from Step 3. Expected: the new scene/painter tests and all existing annotation interaction tests pass.

- [ ] **Step 7: Commit the shared scene boundary**

```bash
git add ui/src/components/review/annotationScene.ts ui/src/components/review/annotationScene.test.ts ui/src/components/review/AnnotationCanvas.tsx ui/src/components/review/AnnotationCanvas.test.tsx
git commit -m "refactor: share annotation scene rendering"
```

---

### Task 3: Add the generic magnifier overlay painter port

**Files:**
- Create: `ui/src/components/imagePreview/magnifierOverlay.ts`
- Modify: `ui/src/components/imagePreview/ImageMagnifier.tsx`
- Test: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`
- Modify: `ui/src/components/imagePreview/ImagePreviewSurface.tsx`
- Test: `ui/src/components/imagePreview/ImagePreviewSurface.test.tsx`
- Modify: `ui/src/styles/app.css`
- Test: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: `createMagnifierContentProjection` from Task 1.
- Produces:

```ts
export interface MagnifierOverlayFrame {
  projection: MagnifierContentProjection
  magnification: number
  pixelRatio: number
}

export type MagnifierOverlayPainter = (
  context: CanvasRenderingContext2D,
  frame: MagnifierOverlayFrame,
) => number
```

- Adds `overlayPainter?: MagnifierOverlayPainter` to `ImageMagnifierProps`.
- Adds `magnifierOverlayPainter?: MagnifierOverlayPainter` to `ImagePreviewSurfaceSlots`.

- [ ] **Step 1: Write a failing `ImageMagnifier` overlay test**

Render a ready original with a spy painter, call the imperative `place`, flush the animation frame, and assert:

```ts
expect(screen.getByTestId('image-magnifier-overlay')).toHaveAttribute(
  'data-has-content',
  'true',
)
expect(painter).toHaveBeenCalledWith(
  expect.anything(),
  expect.objectContaining({ magnification: 2, pixelRatio: 2 }),
)
expect(screen.getByTestId('image-magnifier')).toHaveAttribute('data-visible', 'true')
```

Add a painter-throws case: the image lens remains visible, the Canvas loses `data-has-content`, and no exception escapes the frame callback.

Add an original-identity case: when the file/original representation changes, the old overlay is cleared before the next original becomes ready; loading, budget-error, and decode-error states must never retain the previous image's annotations.

- [ ] **Step 2: Write a failing generic-surface slot test**

Pass `slots={{ magnifierOverlayPainter: painter }}` to `ImagePreviewSurface`, enable the magnifier, move over real image pixels, and assert the painter receives the original-size projection. Render again without the slot and assert no overlay Canvas is present.

- [ ] **Step 3: Run the focused component and style tests**

```bash
pnpm --dir ui exec vitest run src/components/imagePreview/ImageMagnifier.test.tsx src/components/imagePreview/ImagePreviewSurface.test.tsx src/styles/app.test.ts
```

Expected: FAIL because the painter slot and overlay Canvas do not exist.

- [ ] **Step 4: Implement the high-DPI lens-sized Canvas**

Inside `ImageMagnifier`, render the Canvas only when `overlayPainter` exists:

```tsx
<canvas
  ref={overlayCanvas}
  className="image-magnifier__overlay"
  data-testid="image-magnifier-overlay"
  aria-hidden="true"
/>
```

During the existing `flush` callback:

1. configure Canvas CSS size to the lens dimensions;
2. configure physical size to `lens × clamp(devicePixelRatio, 1, 4)`;
3. clear and set the logical CSS-pixel transform;
4. create the projection from the exact source point, original dimensions, content scale, and rotation used by the image;
5. invoke the latest painter and set `data-has-content="true"` only when it returns a positive count;
6. catch painter/Canvas errors, clear the Canvas, and keep the image lens visible.

Use refs for the latest painter and placement. Painter updates and preference/rotation/size changes schedule the same existing animation-frame flush; do not introduce pointer-driven React state.

- [ ] **Step 5: Add clipping and stacking styles**

Add:

```css
.image-magnifier__overlay {
  height: 100%;
  inset: 0;
  pointer-events: none;
  position: absolute;
  width: 100%;
}
```

Raise `.image-magnifier` from z-index 2 to z-index 4. Preserve `.annotation-canvas-layer` at 3 and `.inline-feedback-editor` at 5. Update the style contract test to assert this exact ordering and `pointer-events: none` on both lens and overlay.

- [ ] **Step 6: Run focused tests and confirm generic preview fallback**

Run the command from Step 3. Expected: all tests pass, including painter failure and no-slot image-only behavior.

- [ ] **Step 7: Commit the generic overlay port**

```bash
git add ui/src/components/imagePreview/magnifierOverlay.ts ui/src/components/imagePreview/ImageMagnifier.tsx ui/src/components/imagePreview/ImageMagnifier.test.tsx ui/src/components/imagePreview/ImagePreviewSurface.tsx ui/src/components/imagePreview/ImagePreviewSurface.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: add magnifier overlay painter"
```

---

### Task 4: Wire the review scene into the lens and preserve gesture ownership

**Files:**
- Modify: `ui/src/components/review/annotationScene.ts`
- Test: `ui/src/components/review/annotationScene.test.ts`
- Modify: `ui/src/components/review/AnnotationCanvas.tsx`
- Test: `ui/src/components/review/AnnotationCanvas.test.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkspace.tsx`
- Test: `ui/src/components/review/ImageReviewWorkspace.test.tsx`
- Modify: `ui/src/components/imagePreview/ImagePreviewSurface.tsx`
- Test: `ui/src/components/imagePreview/ImagePreviewSurface.test.tsx`
- Test: `ui/src/components/review/ImageReviewWorkbench.integration.test.tsx`
- Modify: `ui/src/styles/review.css`

**Interfaces:**
- Consumes: `MagnifierOverlayPainter`, `MagnifierOverlayFrame`, and `paintAnnotationScene` from Tasks 2–3.
- Produces:

```ts
export function createAnnotationMagnifierPainter(
  scene: ReadonlyArray<AnnotationSceneItem>,
): MagnifierOverlayPainter
```

- [ ] **Step 1: Write failing review-painter tests**

Create a scene containing a rectangle, stroke, asset item, selected item, and transient anchor. Invoke the painter with a fake magnifier projection and assert:

- only image anchors are painted;
- geometry uses `frame.projection.normalizedToLens`;
- `lineWidth` is `clamp(2 * magnification, 2, 5)` for 1×, 2×, and 4×;
- ordinals use a 14px radius regardless of magnification;
- transient geometry stays dashed;
- color resolves from `--review-annotation` on `context.canvas`, falling back to `CanvasText`.

- [ ] **Step 2: Write a failing workbench integration test for the composite lens**

Open an annotated image workbench, enable the magnifier, point at a location whose lens overlaps a saved rectangle, and assert:

```ts
expect(screen.getByTestId('image-magnifier-overlay')).toHaveAttribute(
  'data-has-content',
  'true',
)
expect(screen.getByTestId('image-magnifier')).toHaveAttribute('data-visible', 'true')
expect(screen.getByTestId('image-magnifier')).toHaveStyle({ 'pointer-events': 'none' })
```

Inspect the painter spy/frame to prove the rectangle corner and ordinal are projected through the same source sample as the image, not through stage DOM bounds.

- [ ] **Step 3: Write a failing capture-phase drawing test**

Provide a `stageOverlay` child that calls `event.stopPropagation()` on pointer move. Enable the magnifier, dispatch pointer movement on that child, and assert the lens source variables still update. In the full workbench integration test, begin a brush or rectangle draft and assert the painter receives the transient scene while the controller remains the pointer owner.

- [ ] **Step 4: Run the focused review and surface tests**

```bash
pnpm --dir ui exec vitest run src/components/review/annotationScene.test.ts src/components/review/AnnotationCanvas.test.tsx src/components/review/ImageReviewWorkspace.test.tsx src/components/imagePreview/ImagePreviewSurface.test.tsx src/components/review/ImageReviewWorkbench.integration.test.tsx
```

Expected: FAIL because the review painter is not injected and stage pointer sampling still depends on bubbled events.

- [ ] **Step 5: Build one scene snapshot in the review composition layer**

In `ImageReviewWorkspace`, derive `transientAnchor` from the same current Controller conditions currently used by `AnnotationCanvas`, build one annotation scene, and pass it to both:

```tsx
<AnnotationCanvas projection={projection} controller={controller} scene={scene} />
```

and:

```ts
magnifierOverlayPainter: createAnnotationMagnifierPainter(scene)
```

Memoize the scene/painter by feedback, selected item, editor state, and draft anchor. Do not pass the Controller into `ImagePreviewSurface` or `ImageMagnifier`.

- [ ] **Step 6: Separate capture-phase lens sampling from bubble-phase gestures**

Split the current combined `sampleMagnifier` handler:

```tsx
onPointerDownCapture={trackMagnifierPointer}
onPointerMoveCapture={trackMagnifierPointer}
onPointerDown={gestures.onPointerDown}
onPointerMove={gestures.onPointerMove}
```

Keep pointer enter/leave visibility behavior. Capture handlers only record/source-sample; bubble handlers retain pan and annotation ownership. Ensure one physical event does not invoke `place` twice.

- [ ] **Step 7: Apply the review color token to the generic overlay Canvas**

In `review.css`, assign `--review-annotation: var(--viewer-danger-strong)` to the lens overlay only when the preview contains an annotation layer. Do not put review tokens in the generic `ImageMagnifier` component.

- [ ] **Step 8: Run focused tests and confirm green**

Run the command from Step 4. Expected: composite overlay, live draft, capture-phase tracking, and existing annotation interaction tests all pass.

- [ ] **Step 9: Commit the review integration**

```bash
git add ui/src/components/review/annotationScene.ts ui/src/components/review/annotationScene.test.ts ui/src/components/review/AnnotationCanvas.tsx ui/src/components/review/AnnotationCanvas.test.tsx ui/src/components/review/ImageReviewWorkspace.tsx ui/src/components/review/ImageReviewWorkspace.test.tsx ui/src/components/imagePreview/ImagePreviewSurface.tsx ui/src/components/imagePreview/ImagePreviewSurface.test.tsx ui/src/components/review/ImageReviewWorkbench.integration.test.tsx ui/src/styles/review.css
git commit -m "feat: magnify image review annotations"
```

---

### Task 5: Add deterministic visual acceptance and current product documentation

**Files:**
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.test.ts`
- Modify: `ui/src/acceptance/scenes/reviewScenes.tsx`
- Modify: `ui/src/acceptance/scenes/reviewScenes.test.tsx`
- Modify: `ui/src/acceptance/scenes/reviewWorkbenchScenes.ts`
- Modify: `ui/src/acceptance/scenes/reviewWorkbenchScenes.test.tsx`
- Modify: `docs/product/FEATURE_REFERENCE.md`
- Modify: `docs/product/USER_GUIDE.md`

**Interfaces:**
- Produces visual state `RVW-33` / `review-composite-magnifier` in the review scene group.
- Preserves every existing acceptance ID and increments catalog totals from 136 to 137.

- [ ] **Step 1: Write failing catalog and scene registration tests**

Append this exact catalog entry:

```json
{
  "id": "RVW-33",
  "wave": 4,
  "referenceState": "review-composite-magnifier",
  "sceneGroup": "review",
  "components": ["App", "ImageReviewWorkspace", "ImageMagnifier", "AnnotationCanvas"]
}
```

Update the catalog test's review tuple list and total/unique counts to 137. Update `REVIEW_ACCEPTANCE_SCENE_IDS` tests to require `RVW-33` without changing previous ordering.

- [ ] **Step 2: Write the failing deterministic readiness test**

The `RVW-33` recipe must:

1. open the first annotated image;
2. enable the existing `放大镜` button once;
3. dispatch one stable pointer sample over real image pixels so the lens crosses at least one rectangle and one ordinal;
4. return ready only when the original image, ordinary annotations, visible lens, and `.image-magnifier__overlay[data-has-content="true"]` are all present;
5. require no task bar, inline editor, save error, or pending image state.

The unit test must reject readiness when the overlay Canvas or `data-has-content` is missing.

- [ ] **Step 3: Run focused acceptance tests and observe missing registration**

```bash
pnpm --dir ui exec vitest run src/acceptance/acceptanceStateCatalog.test.ts src/acceptance/scenes/reviewScenes.test.tsx src/acceptance/scenes/reviewWorkbenchScenes.test.tsx
```

Expected: FAIL because `RVW-33` is absent from the catalog and scene registry.

- [ ] **Step 4: Register and implement the acceptance recipe**

Add `RVW-33` to the review arrays and `REVIEW_WORKBENCH_SCENES`. Reuse the existing four-annotation snapshot; do not introduce a backend/protocol fixture. Set readiness from authoritative visible DOM state, not from a timeout.

- [ ] **Step 5: Update current user documentation**

In `FEATURE_REFERENCE.md` section “图片预览与放大镜”, state that image-anchored review rectangles, brush paths, live draft geometry, and ordinals are projected with the magnified image while the lens stays non-interactive. Add `annotationScene.ts` and `RVW-33` to maintenance evidence.

In `USER_GUIDE.md` section “8.3 放大镜”, add one paragraph explaining that, in the image review workbench, the lens magnifies image content and its attached review marks together; users continue editing through the ordinary marks outside the non-interactive lens.

Do not change shortcuts, settings options, persistence, formats, privacy, or troubleshooting because those contracts do not change.

- [ ] **Step 6: Run focused tests and product-document policy**

```bash
pnpm --dir ui exec vitest run src/acceptance/acceptanceStateCatalog.test.ts src/acceptance/scenes/reviewScenes.test.tsx src/acceptance/scenes/reviewWorkbenchScenes.test.tsx
pnpm test:policy
```

Expected: both commands pass and existing Current product version strings remain `0.1.7`.

- [ ] **Step 7: Commit acceptance and documentation**

```bash
git add ui/src/acceptance/acceptanceStateCatalog.json ui/src/acceptance/acceptanceStateCatalog.test.ts ui/src/acceptance/scenes/reviewScenes.tsx ui/src/acceptance/scenes/reviewScenes.test.tsx ui/src/acceptance/scenes/reviewWorkbenchScenes.ts ui/src/acceptance/scenes/reviewWorkbenchScenes.test.tsx docs/product/FEATURE_REFERENCE.md docs/product/USER_GUIDE.md
git commit -m "test: accept composite review magnifier"
```

---

### Task 6: Verify the complete interaction and architecture

**Files:**
- Modify only if verification exposes a defect: files already listed in Tasks 1–5.
- Review: `docs/superpowers/specs/2026-09-01-viewer-composite-review-magnifier-design.md`
- Review: `docs/superpowers/plans/2026-09-01-viewer-composite-review-magnifier.md`

**Interfaces:**
- Consumes all deliverables from Tasks 1–5.
- Produces a verified implementation with no protocol, release, or installer work.

- [ ] **Step 1: Run all focused magnifier and review tests together**

```bash
pnpm --dir ui exec vitest run \
  src/components/imagePreview/magnifierGeometry.test.ts \
  src/components/imagePreview/ImageMagnifier.test.tsx \
  src/components/imagePreview/ImagePreviewSurface.test.tsx \
  src/components/review/annotationScene.test.ts \
  src/components/review/AnnotationCanvas.test.tsx \
  src/components/review/ImageReviewWorkspace.test.tsx \
  src/components/review/ImageReviewWorkbench.integration.test.tsx \
  src/acceptance/acceptanceStateCatalog.test.ts \
  src/acceptance/scenes/reviewScenes.test.tsx \
  src/acceptance/scenes/reviewWorkbenchScenes.test.tsx \
  src/styles/app.test.ts
```

Expected: zero failures; expected jsdom Canvas warnings may remain only where existing tests deliberately lack a Canvas implementation.

- [ ] **Step 2: Run UI static checks, complete tests, and both production builds**

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm build:visual-acceptance
pnpm test:visual-acceptance
```

Expected: all commands exit 0. Existing non-failing Biome configuration deprecation and Vite chunk-size notices may be recorded but must not conceal new errors.

- [ ] **Step 3: Perform browser visual acceptance at three viewports**

Start the acceptance server:

```bash
pnpm dev:visual-acceptance
```

Inspect `visual-acceptance.html?id=RVW-33` at 1440×900 Retina/DPR 2, 1024×720, and 720×720. Confirm:

- the lens crosses a rectangle/stroke without showing an unscaled duplicate;
- the image and annotation stay aligned at the sampled pixel;
- the ordinal remains readable and does not expand with image倍率;
- lens clipping remains circular/rounded-rectangle and inside the stage;
- ordinary markers, handles, rail, toolbar, navigation, and inline editor retain their stacking and click behavior;
- no horizontal overflow or stale annotation appears after previous/next navigation.

Reset the temporary browser viewport and stop the acceptance server after inspection.

- [ ] **Step 4: Run repository policy and full verification**

```bash
git diff --check
pnpm test:policy
pnpm verify
```

Expected: all commands exit 0. Do not run signing, notarization, installer, listing, or release commands.

- [ ] **Step 5: Review the final diff against the approved spec**

Use:

```bash
git diff --stat
git diff -- ui/src/components/imagePreview ui/src/components/review ui/src/acceptance ui/src/styles docs/product
```

Confirm every spec acceptance item maps to a test or visual check, generic preview files import no review types, and protocol/Rust/Tauri files are untouched.

- [ ] **Step 6: Commit any verification-only correction and record completion**

If Step 1–5 required a correction, stage only its focused files and commit:

```bash
git commit -m "fix: close composite magnifier verification gap"
```

If no correction was needed, create no empty commit. Update the task plan only after fresh verification evidence is available.
