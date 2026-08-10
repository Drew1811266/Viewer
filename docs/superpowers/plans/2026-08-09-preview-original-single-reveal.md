# Preview Original Single-Reveal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the loading progress visible until the current original image is browser-loaded and fitted, then reveal it once without an intermediate fit-proxy frame.

**Architecture:** Keep `useCurrentOriginal` as the original-request owner and keep the existing fit cache for recovery. `ImagePreview` will select exactly one display candidate: the original while it is available, or the fit representation only after original failure. A candidate identity, browser-load identity, and final viewport geometry must all agree before CSS makes the image visible and hides progress.

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, CSS, pnpm

## Global Constraints

- The first visible image frame must use `original100_percent` unless the original request has reached `budget_error` or `error`.
- `fit_preview` remains a cache and recovery representation; it must never be an intermediate visible frame.
- The preview overlay opens immediately and keeps indeterminate progress visible while the original loads.
- The image must remain visually hidden until its browser `load` event and final fitted geometry are both ready.
- Navigation must not reuse a previous entity or cache key's browser-load readiness.
- Preserve zoom, pan, rotation, magnifier, navigation, and existing fallback warnings.
- Add no dependencies and make no backend changes.

---

### Task 1: Gate the first visible frame on the loaded original

**Files:**
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: `CurrentOriginalState`, `ImageRepresentation`, `usePreviewStageSize`, and `useImageViewport`.
- Produces: an `ImagePreview` DOM contract where `.image-preview-image[data-visible="false"]` is mounted only as a hidden browser-loading candidate and `[data-visible="true"]` is the single revealed frame.

- [ ] **Step 1: Replace the progressive-proxy expectation with a failing single-reveal regression test**

Replace the current `progressively replaces the fit proxy...` test with a test that resolves fit first, resolves original second, and fires the browser load event last:

```tsx
it('keeps the fit proxy hidden and reveals the loaded original as the first image frame', async () => {
  const fit = deferred<ImageRepresentation>()
  const original = deferred<ImageRepresentation>()
  const target = { ...image(1), imageMetadata: { width: 6000, height: 4000 } }
  const request = vi.fn((_file: BrowserFile, representation: ImageRepresentationRequest) =>
    representation.kind === 'original100_percent' ? original.promise : fit.promise,
  )
  const view = render(
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

  await act(async () => fit.resolve(loaded('fit', 900, 600)))
  expect(screen.queryByRole('img', { name: '1.jpg' })).not.toBeInTheDocument()
  expect(screen.getByTestId('image-preview-loading')).toHaveAttribute('data-visible', 'true')

  await act(async () => original.resolve(loaded('original', 6000, 4000)))
  const candidate = view.container.querySelector<HTMLImageElement>('.image-preview-image')
  expect(candidate).toHaveAttribute('data-representation', 'original')
  expect(candidate).toHaveAttribute('data-visible', 'false')
  expect(screen.queryByRole('img', { name: '1.jpg' })).not.toBeInTheDocument()

  fireEvent.load(candidate as HTMLImageElement)
  const visible = await screen.findByRole('img', { name: '1.jpg' })
  expect(visible).toHaveAttribute('data-visible', 'true')
  expect(visibleImageSize(visible)).toEqual({ width: 576, height: 384 })
  expect(screen.getByTestId('image-preview-loading')).toHaveAttribute('data-visible', 'false')
})
```

The mutation this test catches is restoring `fitRepresentation` as the normal pre-original display candidate or hiding progress before the original browser load event.

- [ ] **Step 2: Run the regression test and verify the current implementation fails for the expected reason**

Run:

```bash
pnpm --dir ui test --run src/components/ImagePreview.test.tsx -t "keeps the fit proxy hidden"
```

Expected: FAIL after `fit.resolve(...)` because the current component exposes a `data-representation="fit"` image and hides progress.

- [ ] **Step 3: Add failing recovery and stale-readiness coverage**

Add two component tests:

```tsx
it('reveals a loaded fit preview only after the original reaches a terminal failure', async () => {
  const fit = deferred<ImageRepresentation>()
  const original = deferred<ImageRepresentation>()
  const target = image(1)
  const request = vi.fn((_file: BrowserFile, representation: ImageRepresentationRequest) =>
    representation.kind === 'original100_percent' ? original.promise : fit.promise,
  )
  const view = render(
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

  await act(async () => fit.resolve(loaded('fit', 2400, 1600)))
  expect(view.container.querySelector('.image-preview-image')).toBeNull()
  await act(async () => original.reject(new Error('original failed')))

  const fallback = view.container.querySelector<HTMLImageElement>('.image-preview-image')
  expect(fallback).toHaveAttribute('data-representation', 'fit')
  expect(fallback).toHaveAttribute('data-visible', 'false')
  fireEvent.load(fallback as HTMLImageElement)
  expect(await screen.findByRole('img', { name: '1.jpg' })).toHaveAttribute('data-visible', 'true')
  expect(screen.getByText('无法加载原图，已继续使用适窗预览。')).toBeVisible()
})

it('does not reuse browser-load readiness after navigating to another image', async () => {
  const files = [image(1), image(2)]
  const request = vi.fn(async (file: BrowserFile, representation: ImageRepresentationRequest) =>
    loaded(`${file.entityId}-${representation.kind}`, 6000, 4000),
  )
  const view = render(
    <ImagePreview
      file={files[0] as BrowserFile}
      files={files}
      magnifier={MAGNIFIER}
      pointerClientPoint={POINTER_CLIENT_POINT}
      requestImage={request}
      onNavigate={vi.fn()}
      onClose={vi.fn()}
    />,
  )
  await waitFor(() =>
    expect(view.container.querySelector('.image-preview-image')).not.toBeNull(),
  )
  const first = view.container.querySelector<HTMLImageElement>('.image-preview-image')
  fireEvent.load(first as HTMLImageElement)
  expect(await screen.findByRole('img', { name: '1.jpg' })).toBeVisible()

  view.rerender(
    <ImagePreview
      file={files[1] as BrowserFile}
      files={files}
      magnifier={MAGNIFIER}
      pointerClientPoint={POINTER_CLIENT_POINT}
      requestImage={request}
      onNavigate={vi.fn()}
      onClose={vi.fn()}
    />,
  )
  expect(screen.queryByRole('img', { name: '2.jpg' })).not.toBeInTheDocument()
  expect(screen.getByTestId('image-preview-loading')).toHaveAttribute('data-visible', 'true')
})
```

Run the two tests and confirm they fail because current readiness is representation-based rather than keyed browser-load readiness.

- [ ] **Step 4: Implement the single display-candidate and browser-load gate**

In `ImagePreview.tsx`, replace the normal `original ?? fit` display selection with original-first terminal fallback selection:

```tsx
const originalFallback = originalFallbackCopy(currentOriginal.status)
const displayRepresentation =
  originalRepresentation ?? (originalFallback === null ? null : (fitRepresentation ?? null))
const displayCandidateKey =
  displayRepresentation === null
    ? null
    : `${file.entityId}:${displayRepresentation.cacheKey}`
const [browserLoadedCandidateKey, setBrowserLoadedCandidateKey] = useState<string | null>(null)
const sourceDimensions: Size = file.imageMetadata ?? displayRepresentation ?? EMPTY_STAGE
```

Keep geometry measurement in `useLayoutEffect`, but derive readiness only when the viewport geometry matches the current measurements:

```tsx
useLayoutEffect(() => {
  viewport.setMeasurements(stageSize, sourceDimensions)
}, [sourceDimensions, stageSize, viewport.setMeasurements])

const geometryReady =
  isPositiveSize(stageSize) &&
  isPositiveSize(sourceDimensions) &&
  sameSize(viewport.geometry.stage, stageSize) &&
  sameSize(viewport.geometry.source, sourceDimensions)
const previewReady =
  displayCandidateKey !== null &&
  browserLoadedCandidateKey === displayCandidateKey &&
  geometryReady
```

Add the local exact-size helper:

```tsx
function sameSize(left: Size, right: Size): boolean {
  return left.width === right.width && left.height === right.height
}
```

Use `displayRepresentation` everywhere the visible viewport previously used the progressive `representation`. Mount a hidden candidate before readiness so the browser can load it, key the DOM node to prevent stale events, and expose it to accessibility only after readiness:

```tsx
{displayRepresentation !== null && isPositiveSize(sourceDimensions) ? (
  <img
    key={displayCandidateKey}
    className="image-preview-image"
    src={displayRepresentation.url}
    alt={file.name}
    width={viewport.geometry.source.width || sourceDimensions.width}
    height={viewport.geometry.source.height || sourceDimensions.height}
    draggable={false}
    aria-hidden={!previewReady}
    data-visible={previewReady}
    data-mode={viewport.state.mode}
    data-representation={originalRepresentation === displayRepresentation ? 'original' : 'fit'}
    onLoad={() => setBrowserLoadedCandidateKey(displayCandidateKey)}
    style={{ transform: viewport.transform }}
  />
) : null}
```

Keep progress visible when `!previewReady`. Keep fit requests and cache eviction unchanged. Use `displayRepresentation` for gesture enablement and source-point mapping so invisible or stale candidates cannot activate transforms or the magnifier.

- [ ] **Step 5: Make hidden candidates visually inert and preserve the reveal animation**

In `app.css`, replace the always-present `data-initial-reveal` selector with explicit visibility states:

```css
.image-preview-stage > .image-preview-image[data-visible="false"] {
  opacity: 0;
  pointer-events: none;
  visibility: hidden;
}

.image-preview-stage > .image-preview-image[data-visible="true"] {
  animation: preview-image-reveal 180ms ease-out both;
  visibility: visible;
}
```

Update the existing CSS contract test to assert both selectors and update the reduced-motion selector from `[data-initial-reveal="true"]` to `[data-visible="true"]`.

- [ ] **Step 6: Update existing component tests to dispatch browser load at the real boundary**

For tests that resolve a representation and then interact with the visible image, locate the hidden candidate with `container.querySelector('.image-preview-image')`, call `fireEvent.load(candidate)`, and only then query it by accessible role. Do not fire load in tests that intentionally keep progress pending. Preserve all existing size, zoom, rotation, pan, magnifier, and navigation assertions.

- [ ] **Step 7: Run focused tests until the full preview suite is green**

Run:

```bash
pnpm --dir ui test --run \
  src/components/ImagePreview.test.tsx \
  src/components/imagePreview/useCurrentOriginal.test.tsx \
  src/components/imagePreview/usePreviewStageSize.test.tsx \
  src/styles/app.test.ts
```

Expected: all selected test files pass with zero failures.

- [ ] **Step 8: Run mutation checks for the regression**

Temporarily change candidate selection back to `originalRepresentation ?? fitRepresentation`; the single-reveal test must fail after fit resolution. Restore the correct selection and rerun the test to confirm it passes. Temporarily remove the `browserLoadedCandidateKey === displayCandidateKey` condition; the same test must fail before `fireEvent.load`. Restore the condition and rerun the focused suite.

- [ ] **Step 9: Run complete project verification**

Run:

```bash
pnpm verify
```

Expected: frontend tests, TypeScript/build checks, Rust tests, formatting, linting, architecture checks, and dependency/security policy checks all exit with code 0.

- [ ] **Step 10: Commit the verified implementation**

```bash
git add ui/src/components/ImagePreview.test.tsx \
  ui/src/components/ImagePreview.tsx \
  ui/src/styles/app.test.ts \
  ui/src/styles/app.css
git commit -m "fix: reveal original preview in one frame"
```

- [ ] **Step 11: Start the latest development build for acceptance**

Run:

```bash
pnpm start:viewer
```

Confirm the launcher reports `codex/preview-original-reveal` at the verified commit and that the Viewer process remains running.
