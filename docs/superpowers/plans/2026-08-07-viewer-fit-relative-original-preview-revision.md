# Viewer Fit-Relative Original Preview Revision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make fitted display the only 100% baseline, progressively replace the fit proxy with the current original, make trackpad zoom more responsive, and couple the pointer-adjacent magnifier to the current viewport scale with a clearly visible enter/exit animation.

**Architecture:** Keep one fit-relative viewport model in `useImageViewport`, with pure source/stage geometry in `imageGeometry`. `ImagePreview` concurrently owns bounded fit-proxy prefetch and one abortable current-original request, renders both representations against original-size geometry, and passes the resulting source-to-stage scale to `ImageMagnifier`. Trackpad events and all toolbar/pointer actions continue to feed the same viewport actions; visual motion remains CSS-only and pointer placement remains animation-frame bounded.

**Tech Stack:** React 19, TypeScript 6, Vitest 4, Testing Library, CSS custom properties/keyframes, Tauri 2, Rust workspace verification, Playwright visual acceptance, macOS Accessibility native acceptance.

## Global Constraints

- “适应窗口” is the only `100%` baseline; there is no original-pixel 100% display mode or button.
- The visible percentage is relative to fitted size and remains bounded from `10%` through `800%`.
- The current image ends on `original100_percent`; `fit_preview` is only a temporary first paint or an explicit safety/load fallback.
- Keep at most one current original request and one current original representation; adjacent images may retain only bounded fit proxies.
- Original decoding remains bounded by the existing `100,000,000` pixel and `700 MB` safety budgets.
- Magnifier choices remain exactly `1.5`, `2`, and `3`, and multiply the current viewport source-to-stage scale.
- Q/button parity, visible mouse pointer, pointer-adjacent lens placement, edge flipping, enabled-state memory, and settings schema version 3 remain unchanged.
- Pinch starts with exponent sensitivity `0.004`, is frame-coalesced, anchor-preserving, and must pass a real MacBook trackpad gate before completion.
- Enter motion is about `180 ms` from roughly 70% scale with a slight overshoot; exit is about `140 ms` toward roughly 80% scale; pointer position itself never transitions.
- `prefers-reduced-motion: reduce` removes decorative lens animation.
- Do not add dependencies or bypass the existing Tauri image protocol, CSP, cache, or path-isolation boundaries.

---

## File Responsibility Map

- `ui/src/components/imagePreview/imageGeometry.ts`: pure fit-relative scale, anchor, rotation, source mapping, and pan bounds.
- `ui/src/components/imagePreview/useImageViewport.ts`: React ownership of fit/free state and viewport actions.
- `ui/src/components/imagePreview/usePreviewGestures.ts`: non-passive WebKit wheel normalization and pointer-pan adaptation.
- `ui/src/components/imagePreview/useCurrentOriginal.ts`: one abortable current-original lifecycle with stale-result isolation.
- `ui/src/components/ImagePreview.tsx`: progressive representation selection, toolbar semantics, error/fallback rendering, and orchestration.
- `ui/src/components/imagePreview/ImageMagnifier.tsx`: lens shell lifecycle and original-source rendering at the supplied viewport scale.
- `ui/src/components/imagePreview/magnifierGeometry.ts`: fixed lens size and pointer-adjacent/source placement math.
- `ui/src/styles/app.css`: GPU hints and visible lens enter/exit motion.
- `ui/src/acceptance/scenes/viewingScenes.tsx`: deterministic formal preview states.
- `ui/src/acceptance/acceptanceStateCatalog.json`: revised PRE-02 fit-reset meaning.
- `scripts/viewer-native-acceptance.mjs`: native recipe for the revised fit baseline.
- `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md` and `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`: stable acceptance semantics and evidence status.
- `docs/reviews/2026-08-07-viewer-fit-relative-preview-acceptance.md`: final automated, visual, native, resource, and physical-test evidence.

---

### Task 1: Replace Original-Pixel Mode with One Fit-Relative Viewport and Toolbar

**Files:**
- Modify: `ui/src/components/imagePreview/imageGeometry.ts`
- Modify: `ui/src/components/imagePreview/imageGeometry.test.ts`
- Modify: `ui/src/components/imagePreview/useImageViewport.ts`
- Modify: `ui/src/components/imagePreview/useImageViewport.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `scripts/viewer-native-acceptance.mjs`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`

**Interfaces:**
- Produces: `PreviewMode = 'fit' | 'free'`.
- Produces: `displayScale(state, geometry): number`, where free scale is `fitScale * state.zoom`.
- Produces: `ImageViewport` without `setOriginal`; `setFit()` is the sole reset action.
- Preserves: `zoomBy(factor, anchor)`, `panBy(delta)`, `rotateClockwise()`, and `setMeasurements(stage, source)` signatures.

- [ ] **Step 1: Write failing pure-geometry tests for the fit-relative model**

Replace original-mode expectations in `imageGeometry.test.ts` with explicit fit-relative assertions:

```ts
it('treats fit as the only 100% baseline', () => {
  expect(displayScale(state(), GEOMETRY)).toBeCloseTo(0.45)
  expect(displayScale(state({ mode: 'free', zoom: 1.5 }), GEOMETRY)).toBeCloseTo(0.675)
  expect(displayScale(state({ mode: 'free', zoom: 2 }), GEOMETRY)).toBeCloseTo(0.9)
  expect(panBounds(state({ mode: 'free', zoom: 2 }), GEOMETRY)).toEqual({ x: 200, y: 160 })
})

it('reports relative zoom independently from source pixel density', () => {
  const zoomed = zoomAtAnchor(state(), GEOMETRY, 1.25, { x: 250, y: 200 })
  expect(zoomed.mode).toBe('free')
  expect(zoomed.zoom).toBe(1.25)
  expect(displayScale(zoomed, GEOMETRY)).toBeCloseTo(0.5625)
})
```

Delete every test fixture using `mode: 'original'` and replace rotation round trips with `mode: 'free', zoom: 1 / fitScale` only when a literal one-source-pixel-per-CSS-pixel test is still useful.

- [ ] **Step 2: Write failing hook and toolbar tests**

In `useImageViewport.test.tsx`, remove `setOriginal()` calls and prove `setFit()` is the sole reset:

```ts
act(() => hook.result.current.zoomBy(2, { x: 400, y: 300 }))
expect(hook.result.current.state.mode).toBe('free')
expect(hook.result.current.state.zoom).toBe(2)

act(() => hook.result.current.setFit())
expect(hook.result.current.state).toEqual({
  mode: 'fit',
  zoom: 1,
  rotation: 0,
  offset: { x: 0, y: 0 },
})
```

In `ImagePreview.test.tsx`, add a toolbar contract that rejects the old control and verifies reset:

```ts
expect(screen.getByRole('button', { name: '适应窗口' })).toHaveAttribute(
  'aria-pressed',
  'true',
)
expect(screen.queryByRole('button', { name: '按 100% 显示' })).not.toBeInTheDocument()

fireEvent.click(screen.getByRole('button', { name: '放大' }))
expect(screen.getByText('125%', { selector: '.preview-scale-label' })).toBeVisible()
fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
expect(screen.getByText('100%', { selector: '.preview-scale-label' })).toBeVisible()
```

- [ ] **Step 3: Write failing formal and native acceptance semantics**

Rename PRE-02 in `acceptanceStateCatalog.json` from `preview-100` to `preview-fit-reset`. Change the scene union from `original` to `fit_reset`, and drive that state through zoom then reset:

```tsx
if (state === 'fit_reset') {
  namedButton('放大')?.click()
  namedButton('适应窗口')?.click()
}
```

Update `viewingScenes.test.tsx` to assert PRE-01 and PRE-02 expose no old button and PRE-02 finishes at `100%` with “适应窗口” pressed.

Change the PRE-02 native recipe to:

```js
'PRE-02': [
  ...openImagePreview(),
  { kind: 'click', target: { role: 'AXButton', name: '放大' } },
  { kind: 'click', target: { name: '适应窗口' } },
  { kind: 'assert', target: { role: 'AXStaticText', name: '100%' } },
],
```

Update its frozen assertion in `viewer-native-acceptance.test.mjs`. Revise the two ledger rows so PRE-02 is `preview-fit-reset`, describes zoom-then-fit, and no longer claims explicit original-pixel mode.

- [ ] **Step 4: Run focused tests and verify the old model fails**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/useImageViewport.test.tsx src/components/ImagePreview.test.tsx src/acceptance/scenes/viewingScenes.test.tsx
node --test scripts/viewer-native-acceptance.test.mjs
```

Expected: failures mention `original`, `setOriginal`, the old `按 100% 显示` button, or PRE-02's old click recipe.

- [ ] **Step 5: Implement the minimal fit-relative state and toolbar**

Use this state contract in `imageGeometry.ts`:

```ts
export type PreviewMode = 'fit' | 'free'

export function displayScale(state: ImageViewportState, geometry: ImageViewportGeometry): number {
  if (!hasArea(geometry.stage) || !hasArea(geometry.source)) return 0
  const quarterTurn = state.rotation === 90 || state.rotation === 270
  const sourceWidth = quarterTurn ? geometry.source.height : geometry.source.width
  const sourceHeight = quarterTurn ? geometry.source.width : geometry.source.height
  const fitScale = Math.min(
    1,
    (geometry.stage.width * geometry.fitInset) / sourceWidth,
    (geometry.stage.height * geometry.fitInset) / sourceHeight,
  )
  return fitScale * (state.mode === 'free' ? state.zoom : 1)
}
```

Delete `setOriginal` from `ImageViewport` and its return object. In `ImagePreview.tsx`, render exactly this segmented control:

```tsx
function resetToFit() {
  setError(null)
  viewport.setFit()
}

<ViewerSegmentedControl label="图片显示控制">
  <ViewerButton active={viewport.state.mode === 'fit'} onClick={resetToFit}>
    适应窗口
  </ViewerButton>
  <ViewerIconButton icon="minus" label="缩小" onClick={() => zoomFromToolbar(0.8)} />
  <span className="preview-scale-label" aria-live="polite">
    {Math.round((viewport.state.mode === 'free' ? viewport.state.zoom : 1) * 100)}%
  </span>
  <ViewerIconButton icon="plus" label="放大" onClick={() => zoomFromToolbar(1.25)} />
</ViewerSegmentedControl>
```

Until Task 3 makes original loading automatic, keep the original request lens-only and make the main representation unconditionally use the fit proxy:

```ts
const original = useCurrentOriginal({
  file,
  needed: magnifierEnabled,
  available: !transformsDisabled,
  requestImage,
})
const representation = fitRepresentation
```

Delete the `viewport.state.mode === 'original'` representation branch and original-mode failure fallback effect. Retain the existing disabled conditions and `setError(null)` behavior in `resetToFit` until Task 3 separates fatal and original-fallback errors.

- [ ] **Step 6: Run focused and complete UI tests**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/useImageViewport.test.tsx src/components/ImagePreview.test.tsx src/acceptance/scenes/viewingScenes.test.tsx
node --test scripts/viewer-native-acceptance.test.mjs
pnpm --dir ui check
```

Expected: all commands exit 0; no source or acceptance recipe contains `按 100% 显示` or `mode: 'original'` for single-image preview.

- [ ] **Step 7: Commit the viewport semantic change**

```bash
git add ui/src/components/imagePreview/imageGeometry.ts ui/src/components/imagePreview/imageGeometry.test.ts ui/src/components/imagePreview/useImageViewport.ts ui/src/components/imagePreview/useImageViewport.test.tsx ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx ui/src/acceptance/acceptanceStateCatalog.json scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md
git commit -m "fix: make fitted preview the 100 percent baseline"
```

---

### Task 2: Make Trackpad Pinch More Responsive Without Adding Lag

**Files:**
- Modify: `ui/src/components/imagePreview/usePreviewGestures.ts`
- Modify: `ui/src/components/imagePreview/usePreviewGestures.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`

**Interfaces:**
- Consumes: `zoomBy(factor: number, anchor: Point)` from Task 1.
- Preserves: ordinary wheel-to-pan and pointer-drag contracts.
- Produces: frame-coalesced pinch factor `Math.exp(-deltaY * 0.004)`.
- Produces: visible percentage updates immediately while its polite announcement settles once after 300 ms of inactivity.

- [ ] **Step 1: Write failing sensitivity and fine-delta tests**

Update the existing pinch expectation and add a fine-input coalescing case:

```ts
expect(actions.zoomBy).toHaveBeenCalledWith(Math.exp(0.08), { x: 220, y: 130 })

for (const deltaY of [-2, -3, -5]) {
  stage.dispatchEvent(
    new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      ctrlKey: true,
      clientX: 320,
      clientY: 180,
      deltaY,
    }),
  )
}
expect(frames).toHaveLength(1)
frames.shift()?.(16)
expect(actions.zoomBy).toHaveBeenCalledWith(Math.exp(0.04), { x: 220, y: 130 })
```

Keep the existing two-axis pan assertion `{ x: -12, y: 18 }` and toolbar/outside ownership checks.

Add an `ImagePreview.test.tsx` accessibility case using fake timers. Dispatch three pinch frames, assert the visible `.preview-scale-label` changes immediately, then assert the polite text changes only after the 300 ms settle window:

```ts
vi.useFakeTimers()
const frames: FrameRequestCallback[] = []
vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
  frames.push(callback)
  return frames.length
})
const dispatchPinch = (stage: HTMLElement, deltaY: number) => {
  stage.dispatchEvent(
    new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      ctrlKey: true,
      clientX: 320,
      clientY: 240,
      deltaY,
    }),
  )
}
const flushAnimationFrame = () => act(() => frames.shift()?.(0))

const live = screen.getByTestId('preview-scale-announcement')
expect(live).toHaveTextContent('100%')

dispatchPinch(stage, -10)
flushAnimationFrame()
expect(screen.getByText(/%/, { selector: '.preview-scale-label' })).not.toHaveTextContent('100%')
expect(live).toHaveTextContent('100%')

act(() => vi.advanceTimersByTime(300))
expect(live.textContent).toBe(screen.getByText(/%/, { selector: '.preview-scale-label' }).textContent)
vi.useRealTimers()
vi.unstubAllGlobals()
```

Keep these helpers local to the test; do not introduce a production testing API.

- [ ] **Step 2: Run the gesture test and verify the old coefficient fails**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/usePreviewGestures.test.tsx
```

Expected: the old factor `Math.exp(0.04)` differs from the new `Math.exp(0.08)` expectation.

- [ ] **Step 3: Increase only the exponential pinch coefficient**

Change:

```ts
const PINCH_SENSITIVITY = 0.004
```

Keep `pendingWheel`, `requestAnimationFrame`, accumulated `deltaY`, the latest stage-local anchor, and `event.preventDefault()` unchanged. Do not add a CSS transform transition or timer-based interpolation.

In `ImagePreview.tsx`, remove `aria-live` from the rapidly changing visible label and add one debounced hidden announcement:

```ts
const scalePercent = Math.round((viewport.state.mode === 'free' ? viewport.state.zoom : 1) * 100)
const [announcedScalePercent, setAnnouncedScalePercent] = useState(scalePercent)

useEffect(() => {
  const timer = window.setTimeout(() => setAnnouncedScalePercent(scalePercent), 300)
  return () => window.clearTimeout(timer)
}, [scalePercent])
```

```tsx
<span className="preview-scale-label">{scalePercent}%</span>
<span
  className="visually-hidden"
  data-testid="preview-scale-announcement"
  aria-live="polite"
>
  {announcedScalePercent}%
</span>
```

- [ ] **Step 4: Run gesture, viewport, and integration tests**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/usePreviewGestures.test.tsx src/components/imagePreview/useImageViewport.test.tsx src/components/ImagePreview.test.tsx
```

Expected: all tests pass; pinch remains one viewport update per frame and ordinary two-finger input remains pan.

- [ ] **Step 5: Commit the gesture calibration**

```bash
git add ui/src/components/imagePreview/usePreviewGestures.ts ui/src/components/imagePreview/usePreviewGestures.test.tsx ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx
git commit -m "fix: increase preview pinch responsiveness"
```

---

### Task 3: Progressively Replace the Fit Proxy with One Current Original

**Files:**
- Modify: `ui/src/components/imagePreview/useCurrentOriginal.ts`
- Modify: `ui/src/components/imagePreview/useCurrentOriginal.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`

**Interfaces:**
- Produces: `useCurrentOriginal({ file, available, requestImage }): CurrentOriginalState`; remove `needed`.
- Produces: visible representation priority `ready original > current fit proxy > loading/error`.
- Produces: viewport source size priority `file.imageMetadata > current original dimensions > fit dimensions`.
- Preserves: one abort controller and revision per current original identity.

- [ ] **Step 1: Write failing current-original lifecycle tests**

Replace the demand-gated hook test with automatic current-image loading:

```ts
const hook = renderHook(({ available }) =>
  useCurrentOriginal({ file: image('one'), available, requestImage }),
  { initialProps: { available: false } },
)
expect(requestImage).not.toHaveBeenCalled()

hook.rerender({ available: true })
await waitFor(() => expect(hook.result.current.status).toBe('ready'))
expect(requestImage).toHaveBeenCalledTimes(1)
expect(requestImage).toHaveBeenCalledWith(
  expect.objectContaining({ entityId: 'one' }),
  { kind: 'original100_percent' },
  expect.any(AbortSignal),
)
```

Keep and adapt the existing identity dedupe, stale completion, navigation abort, unavailability, unmount, budget-error, generic-error, and AbortError tests.

- [ ] **Step 2: Write failing progressive replacement and geometry tests**

In `ImagePreview.test.tsx`, add `ImageRepresentation` to the existing API type import, add these local test helpers, and use deferred proxy/original responses:

```ts
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

function loaded(name: string, width: number, height: number): ImageRepresentation {
  return {
    cacheKey: name,
    url: `viewer-image://localhost/session/${name}`,
    width,
    height,
    backend: 'image_io',
  }
}

const fit = deferred<ImageRepresentation>()
const original = deferred<ImageRepresentation>()
const request = vi.fn((_file, representation) =>
  representation.kind === 'original100_percent' ? original.promise : fit.promise,
)
const target = { ...image(1), imageMetadata: { width: 6000, height: 4000 } }

render(
  <ImagePreview
    file={target}
    files={[target]}
    magnifier={MAGNIFIER}
    pointerClientPoint={POINTER_CLIENT_POINT}
    requestImage={request}
    onNavigate={vi.fn()}
    onClose={vi.fn()}
  />,
)
await waitFor(() => {
  expect(request.mock.calls.some(([, value]) => value.kind === 'fit_preview')).toBe(true)
  expect(request.mock.calls.some(([, value]) => value.kind === 'original100_percent')).toBe(true)
})

await act(async () => fit.resolve(loaded('fit', 1200, 800)))
const image = await screen.findByRole('img', { name: /\.jpg$/ })
expect(image.getAttribute('src')).toContain('fit')
expect(image).toHaveAttribute('width', '6000')
expect(image).toHaveAttribute('height', '4000')
const transformBefore = image.style.transform

await act(async () => original.resolve(loaded('original', 6000, 4000)))
expect(image.getAttribute('src')).toContain('original')
expect(image.style.transform).toBe(transformBefore)
```

Add a test proving magnifier activation after original readiness does not make a second original request, and a navigation test proving the first original signal is aborted before the second completes.

- [ ] **Step 3: Write failing fallback-state tests**

Cover both approved fallbacks while retaining the proxy image:

```ts
expect(await screen.findByRole('status')).toHaveTextContent(
  '原图超出安全预览限制，已继续使用适窗预览。',
)
expect(screen.getByRole('img', { name: file.name })).toHaveAttribute(
  'src',
  expect.stringContaining('fit'),
)
```

Repeat with `无法加载原图，已继续使用适窗预览。`. Add a both-failed case that retains the existing fatal `无法显示这张图片` alert.

- [ ] **Step 4: Run focused tests and verify demand-gated behavior fails**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/useCurrentOriginal.test.tsx src/components/ImagePreview.test.tsx
```

Expected: no original request occurs until the old `needed` condition, the proxy keeps its own width/height, or fallback copy is absent.

- [ ] **Step 5: Make the hook automatic and select the visible representation**

Remove `needed` from `CurrentOriginalOptions` and use only availability:

```ts
if (!available) {
  setState(idle(entityId))
  return
}
```

In `ImagePreview.tsx`:

```ts
const original = useCurrentOriginal({
  file,
  available: !transformsDisabled,
  requestImage,
})
const fitRepresentation = transformsDisabled ? undefined : fitCache.current.get(file.entityId)
const representation =
  original.status === 'ready' && original.representation !== null
    ? original.representation
    : fitRepresentation
const sourceDimensions: Size =
  file.imageMetadata ?? original.representation ?? fitRepresentation ?? EMPTY_STAGE
```

Pass `sourceDimensions` to `viewport.setMeasurements`, and render the selected URL against source geometry:

```tsx
<img
  className="image-preview-image"
  src={representation.url}
  alt={file.name}
  width={sourceDimensions.width}
  height={sourceDimensions.height}
  draggable={false}
  data-representation={original.representation === representation ? 'original' : 'fit'}
  style={{ transform: viewport.transform }}
/>
```

Do not publish fit-representation dimensions through `onDimensions`; publish metadata already known by the file or the ready original dimensions. Keep the adjacent fit-cache window at current index ±1.

- [ ] **Step 6: Separate loading, fallback, and fatal feedback**

Derive states rather than storing one ambiguous `error` string:

```ts
const originalFallback =
  representation === fitRepresentation && original.status === 'budget_error'
    ? '原图超出安全预览限制，已继续使用适窗预览。'
    : representation === fitRepresentation && original.status === 'error'
      ? '无法加载原图，已继续使用适窗预览。'
      : null
```

Render `originalFallback` with `tone="warning"` and title `正在使用适窗预览`. Render the existing danger state only when neither original nor fit representation can be shown and all relevant requests have failed. Abort and entity change must clear both states.

- [ ] **Step 7: Run focused and full UI tests**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/useCurrentOriginal.test.tsx src/components/ImagePreview.test.tsx src/acceptance/scenes/viewingScenes.test.tsx
pnpm --dir ui check
pnpm --dir ui test
```

Expected: all commands exit 0; current original is automatic, proxy-to-original replacement preserves transform, and no second original request is made for the lens.

- [ ] **Step 8: Commit progressive original rendering**

```bash
git add ui/src/components/imagePreview/useCurrentOriginal.ts ui/src/components/imagePreview/useCurrentOriginal.test.tsx ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx
git commit -m "feat: progressively render the current original"
```

---

### Task 4: Couple Magnifier Detail to the Current Viewport Scale

**Files:**
- Modify: `ui/src/components/imagePreview/ImageMagnifier.tsx`
- Modify: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`
- Modify: `ui/src/components/imagePreview/magnifierGeometry.ts`
- Modify: `ui/src/components/imagePreview/magnifierGeometry.test.ts`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx`

**Interfaces:**
- Produces: `ImageMagnifier` prop `sourceScale: number`.
- Produces: CSS variable `--magnifier-scale = sourceScale * magnification`.
- Produces: `magnifierSourcePlacement(sourcePoint, dimensions)` with no unused magnification argument.
- Consumes: `viewport.scale` from Task 1 and the one ready original from Task 3.

- [ ] **Step 1: Write failing magnifier scale tests**

Pass `sourceScale={0.25}` in the existing component harness and assert current-view coupling:

```ts
const handle = createRef<ImageMagnifierHandle>()
render(
  <ImageMagnifier
    ref={handle}
    shape="circle"
    area="small"
    magnification={1.5}
    sourceScale={0.25}
    stageSize={{ width: 640, height: 480 }}
    rotation={0}
    fileName="detail.jpg"
    original={currentOriginal('ready', ORIGINAL)}
  />,
)
act(() => handle.current?.place({
  stagePoint: { x: 200, y: 150 },
  sourcePoint: { x: 1600, y: 1200 },
}))
act(() => frames.shift()?.(0))
expect(screen.getByTestId('image-magnifier')).toHaveStyle({
  '--magnifier-scale': '0.375',
})
```

Add cases for `(sourceScale, magnification) = (0.25, 2)`, `(0.5, 3)`, and a runtime prop change while the pointer is stationary.

In `magnifierGeometry.test.ts`, call `magnifierSourcePlacement(sourcePoint, dimensions)` and preserve center-point assertions.

- [ ] **Step 2: Write failing ImagePreview integration tests**

With the default 640×480 stage and a deterministic 1280×960 source (fit scale 0.45), enable the lens and assert:

```ts
expect(lens.style.getPropertyValue('--magnifier-scale')).toBe('0.675')

fireEvent.click(screen.getByRole('button', { name: '放大' }))
await waitFor(() =>
  expect(lens.style.getPropertyValue('--magnifier-scale')).toBe('0.84375'),
)
```

The example uses fit scale `0.45`, 1.5× lens, then 1.25× viewport zoom. Also assert the lens source URL equals the main original URL and the original request count remains one.

- [ ] **Step 3: Run focused tests and verify fixed-original scaling fails**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/magnifierGeometry.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/ImagePreview.test.tsx
```

Expected: the lens still reports raw setting values such as `1.5` rather than `0.375` or `0.675`.

- [ ] **Step 4: Add source scale to the magnifier configuration**

Use this prop and configuration shape:

```ts
interface ImageMagnifierProps {
  shape: MagnifierShape
  area: MagnifierArea
  magnification: MagnifierMagnification
  sourceScale: number
  stageSize: Size
  rotation: PreviewRotation
  fileName: string
  original: CurrentOriginalState
}

interface MagnifierConfiguration {
  shape: MagnifierShape
  area: MagnifierArea
  magnification: MagnifierMagnification
  sourceScale: number
  stageSize: Size
  rotation: PreviewRotation
}
```

During each frame:

```ts
element.style.setProperty(
  '--magnifier-scale',
  String(current.sourceScale * current.magnification),
)
```

Use the same product for the initial inline style. Add `sourceScale` to configuration refresh and effect dependencies. Remove the unused magnification parameter from `magnifierSourcePlacement`.

- [ ] **Step 5: Pass the viewport scale and preserve original coordinate mapping**

In `ImagePreview.tsx`:

```tsx
<ImageMagnifier
  ref={magnifierHandle}
  shape={magnifier.shape}
  area={magnifier.area}
  magnification={magnifier.magnification}
  sourceScale={viewport.scale}
  stageSize={stageSize}
  rotation={viewport.state.rotation}
  fileName={file.name}
  original={original}
/>
```

Keep `sourcePointAtStagePoint` as the inverse viewport mapping. If `viewport.geometry.source` temporarily differs from the ready original size because metadata was unavailable, continue using `remapSourcePoint`; hide for that transient frame if either size has zero area.

- [ ] **Step 6: Update formal magnifier readiness**

For the 560×373 acceptance fixture the fit scale remains 1, so PRE-08 continues to expect `--magnifier-scale: 1.5`. Add an assertion that the main preview source contains `representation=original100_percent`, proving both views reuse the original representation.

- [ ] **Step 7: Run magnifier and integration tests**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/magnifierGeometry.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/ImagePreview.test.tsx src/acceptance/scenes/viewingScenes.test.tsx
pnpm --dir ui check
```

Expected: all commands exit 0 and lens scale changes immediately when the main viewport changes under a stationary pointer.

- [ ] **Step 8: Commit scale coupling**

```bash
git add ui/src/components/imagePreview/ImageMagnifier.tsx ui/src/components/imagePreview/ImageMagnifier.test.tsx ui/src/components/imagePreview/magnifierGeometry.ts ui/src/components/imagePreview/magnifierGeometry.test.ts ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx
git commit -m "fix: couple magnifier detail to viewport scale"
```

---

### Task 5: Make Lens Enter and Exit Motion Clearly Visible

**Files:**
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`

**Interfaces:**
- Consumes: existing `data-visible="true"` shell state and pointer-side transform-origin variables.
- Produces: `magnifier-pop-in` keyframes and 140 ms exit transition.
- Preserves: immediate `left`, `top`, and internal original-source position updates.

- [ ] **Step 1: Write failing style-contract tests**

Revise the existing magnifier CSS test:

```ts
expect(lens?.declarations).toMatchObject({
  opacity: '0',
  transform: 'translate(-50%, -50%) scale(0.8)',
  'will-change': 'opacity, transform',
})
expect(lens?.declarations.transition).toContain('140ms')
expect(visible?.declarations.animation).toContain('magnifier-pop-in 180ms')
expect(appCss).toContain('@keyframes magnifier-pop-in')
expect(appCss).toContain('scale(0.7)')
expect(appCss).toContain('scale(1.04)')
```

Keep reduced-motion assertions for `animation: none` and `transition: none`. Add an assertion that neither base nor visible declarations transition `left` or `top`.

- [ ] **Step 2: Run the style test and verify the subtle motion fails**

Run:

```bash
pnpm --dir ui test -- src/styles/app.test.ts src/components/imagePreview/ImageMagnifier.test.tsx
```

Expected: CSS still contains scale `0.85`, 110 ms exit, and no pop-in keyframes.

- [ ] **Step 3: Implement the stronger pop-in and retract motion**

Use:

```css
.image-preview-stage > .image-preview-image {
  will-change: transform;
}

.image-magnifier {
  opacity: 0;
  transform: translate(-50%, -50%) scale(0.8);
  transition:
    opacity 140ms ease-in,
    transform 140ms ease-in,
    visibility 0s linear 140ms;
  visibility: hidden;
  will-change: opacity, transform;
}

.image-magnifier[data-visible="true"] {
  animation: magnifier-pop-in 180ms cubic-bezier(0.16, 1, 0.3, 1) both;
  opacity: 1;
  transform: translate(-50%, -50%) scale(1);
  transition:
    opacity 180ms ease-out,
    transform 180ms ease-out,
    visibility 0s linear 0s;
  visibility: visible;
}

@keyframes magnifier-pop-in {
  0% {
    opacity: 0;
    transform: translate(-50%, -50%) scale(0.7);
  }
  72% {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1.04);
  }
  100% {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1);
  }
}
```

Do not include `left`, `top`, `--magnifier-x`, `--magnifier-y`, or internal source placement in transitions/keyframes. Keep the current transform origin so the shell grows from the pointer-facing corner.

- [ ] **Step 4: Preserve reduced motion and forced colors**

Keep:

```css
@media (prefers-reduced-motion: reduce) {
  .image-magnifier {
    animation: none;
    transition: none;
  }
}
```

Do not change the forced-colors border/background rules or pointer visibility.

- [ ] **Step 5: Run style and component tests**

Run:

```bash
pnpm --dir ui test -- src/styles/app.test.ts src/styles/visualAccessibility.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/ImagePreview.test.tsx
pnpm --dir ui check
```

Expected: all commands exit 0; the animation contract is explicit and position variables remain untransitioned.

- [ ] **Step 6: Commit the animation**

```bash
git add ui/src/styles/app.css ui/src/styles/app.test.ts ui/src/components/imagePreview/ImageMagnifier.test.tsx
git commit -m "fix: strengthen magnifier enter and exit motion"
```

---

### Task 6: Refresh Formal Acceptance, Run Gates, and Record Physical Evidence

**Files:**
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx` if deterministic readiness needs final timing adjustment
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx` if the final original URL/scale assertion needs alignment
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`
- Create: `docs/reviews/2026-08-07-viewer-fit-relative-preview-acceptance.md`

**Interfaces:**
- Consumes: completed Tasks 1–5.
- Produces: automated, visual, native, resource, and user-performed MacBook evidence tied to an exact commit.
- Produces: a running latest development build for user evaluation.

- [ ] **Step 1: Run the focused feature suite**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/imageGeometry.test.ts src/components/imagePreview/useImageViewport.test.tsx src/components/imagePreview/usePreviewGestures.test.tsx src/components/imagePreview/useCurrentOriginal.test.tsx src/components/imagePreview/magnifierGeometry.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/ImagePreview.test.tsx src/styles/app.test.ts src/styles/visualAccessibility.test.ts src/acceptance/scenes/viewingScenes.test.tsx
node --test scripts/viewer-native-acceptance.test.mjs
```

Expected: every selected test passes with no skipped feature test.

- [ ] **Step 2: Run the complete repository gate**

Run:

```bash
pnpm verify
pnpm coverage:ui
pnpm coverage:rust
pnpm architecture:health
```

Expected: all commands exit 0; UI and Rust coverage do not regress below their checked baselines.

- [ ] **Step 3: Generate formal visual evidence at both approved viewports**

Run:

```bash
pnpm accept:visual -- --id PRE-01 --id PRE-02 --id PRE-03 --id PRE-04 --id PRE-05 --id PRE-06 --id PRE-07 --id PRE-08
```

Inspect every generated `product.png` and `combined.png` for 1024×720 and 1440×900. Confirm:

- toolbar order is `适应窗口｜−｜百分比｜+` with no duplicate 100% control;
- PRE-01 and PRE-02 both finish at fitted 100%; PRE-03 is 156%;
- original replacement does not alter layout;
- PRE-08 keeps the pointer visible, lens bounded, and original source ready;
- loading/error/navigation states remain reachable and unobscured.

Static images do not prove animation; cite Task 5's CSS/component tests for timing and keyframe evidence.

- [ ] **Step 4: Run native plan tests and focused packaged acceptance**

Run:

```bash
pnpm test:native-acceptance
pnpm accept:native -- --id PRE-01 --viewport 1440x900
pnpm accept:native -- --id PRE-02 --viewport 1440x900
pnpm accept:native -- --id PRE-08 --viewport 1440x900
```

Expected: PRE-01 opens and resets to fit, PRE-02 zooms then returns to 100%, and PRE-08 proves Q/button parity through native Accessibility actions.

- [ ] **Step 5: Start the latest development build and measure idle resources**

Run:

```bash
pnpm start:viewer
```

After the launcher reports the Viewer PID, leave one normal image open and sample it with:

```bash
VIEWER_PROCESS_ID="$(pgrep -n -f '/Viewer.app/Contents/MacOS/viewer-desktop')"
test -n "$VIEWER_PROCESS_ID"
ps -o pid,%cpu,rss,etime,command -p "$VIEWER_PROCESS_ID"
```

Record the real PID and sample in the acceptance document. Confirm CPU settles near idle after original decode and that navigating does not leave several original requests or a steadily rising decoded-image working set. Do not run repeated unbounded screenshot loops.

- [ ] **Step 6: Ask the user to perform the physical MacBook gate**

Provide this exact checklist against the running development build:

1. lightly pinch in and out and confirm immediate response;
2. perform a longer continuous pinch and confirm no stepping or delayed catch-up;
3. confirm the image point between the fingers remains anchored;
4. at enlarged scale, pan horizontally, vertically, and diagonally to all four boundaries;
5. confirm pan never changes image and the page itself never zooms;
6. enable/disable with Q and the toolbar and confirm the stronger pop/retract motion;
7. confirm the arrow remains visible and the lens follows without trailing;
8. compare 1.5×, 2×, and 3× at main preview 100% and 200%;
9. leave/re-enter the image and navigate rapidly while the lens remains enabled;
10. report whether the Mac becomes abnormally hot after activity settles.

If the physical pinch gate fails, stop completion and write a focused native-adapter amendment before changing Tauri/AppKit code.

- [ ] **Step 7: Write the exact acceptance record**

Create `docs/reviews/2026-08-07-viewer-fit-relative-preview-acceptance.md` with:

```markdown
# Viewer Fit-Relative Preview Acceptance

- Commit: record the output of `git rev-parse HEAD`
- Automated gate: record the exact UI/Rust totals printed by `pnpm verify`
- Coverage: record the summaries printed by the UI and Rust baseline checks
- Visual evidence: record the generated PRE-01…PRE-08 output directories for both viewports
- Native evidence: record the generated PRE-01, PRE-02, and PRE-08 result directories and verdicts
- Resource sample: record the PID, CPU, RSS, elapsed time, and open-image test state
- Physical MacBook verdict: record pass/fail for each of the ten approved checks
- Remaining limitation: physical gesture quality is hardware evidence and is not inferred from synthetic wheel tests
```

Populate every line from the measured command output before saving. Update PRE-01…PRE-08 ledger rows to point to the new evidence; mark PRE-08 physical acceptance pass only after the user reports a pass.

- [ ] **Step 8: Re-run repository policy after evidence edits**

Run:

```bash
pnpm test:policy
git diff --check
git status --short
```

Expected: policy and whitespace checks pass; only the intended evidence/ledger files remain uncommitted.

- [ ] **Step 9: Commit verified acceptance evidence**

```bash
git add docs/reviews/2026-08-07-viewer-fit-relative-preview-acceptance.md docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx
git commit -m "docs: verify fit-relative preview revision"
```

Only include the two acceptance scene files if Step 3 required a deterministic readiness correction; otherwise stage the three review files only.

---

## Final Completion Check

Before claiming completion, invoke `superpowers:verification-before-completion` and re-read the final command outputs. The completion report must distinguish automated pass, visual/native pass, and the user's physical MacBook verdict; it must not describe synthetic `wheel` tests as proof of real trackpad quality.
