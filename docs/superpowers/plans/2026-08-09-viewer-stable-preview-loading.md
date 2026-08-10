# Viewer Stable Preview Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the visible 640×480 temporary preview with a real-stage readiness gate and an accessible indeterminate loading animation, so the first image appears once at its final fitted size.

**Architecture:** Extract actual preview-stage measurement into a focused hook that publishes only finite, positive DOM sizes. `ImagePreview` will commit viewport geometry and the current entity's first reveal together in a layout effect; a focused loading component remains visible until that commit, then fades out while the image fades in without scale motion.

**Tech Stack:** React 19, TypeScript 6, CSS keyframes and media queries, ResizeObserver, Vitest, Testing Library

## Global Constraints

- A zero or unavailable stage measurement remains pending; never replace it with 640×480 or another invented size.
- The first visible image uses the real stage and final fitted geometry.
- Loading copy is exactly `正在载入图片` and `正在准备高清预览…`.
- Progress is indeterminate and exposes no fabricated percentage or ARIA value.
- Loading dismissal and initial image reveal last approximately 180ms and animate opacity only.
- Under `prefers-reduced-motion: reduce`, progress movement and reveal/dismiss transitions are disabled while the static loading surface remains.
- Fit-proxy-to-original replacement changes pixels without changing the visible fitted size or replaying the initial reveal.
- Existing zoom, pan, magnifier, navigation, rotation and close behavior remains functional.
- Add no dependency and change no desktop image API or persistence schema.

---

## File Structure

- Create `ui/src/components/imagePreview/usePreviewStageSize.ts` — own real DOM stage measurement and reject zero/invented sizes.
- Create `ui/src/components/imagePreview/usePreviewStageSize.test.tsx` — verify initial, observed, fallback and cleanup measurement behavior.
- Create `ui/src/components/imagePreview/ImagePreviewLoading.tsx` — render the accessible loading copy and indeterminate progress semantics.
- Modify `ui/src/components/ImagePreview.tsx` — consume the stage hook, gate first reveal by committed geometry and integrate loading state.
- Modify `ui/src/components/ImagePreview.test.tsx` — exercise zero-to-real stage timing, stable proxy upgrades, progress semantics and entity navigation.
- Modify `ui/src/styles/app.css` — style the progress sweep, 180ms opacity reveals and reduced-motion behavior.
- Modify `ui/src/styles/app.test.ts` — lock animation, token, duration and reduced-motion contracts.

### Task 1: Measure Only The Actual Preview Stage

**Files:**
- Create: `ui/src/components/imagePreview/usePreviewStageSize.ts`
- Create: `ui/src/components/imagePreview/usePreviewStageSize.test.tsx`

**Interfaces:**
- Consumes: `RefObject<HTMLElement | null>` for the always-mounted preview stage.
- Produces: `usePreviewStageSize(stage): Size`, initially `{ width: 0, height: 0 }`, then the latest rounded finite positive stage size.

- [ ] **Step 1: Write the stage-measurement hook tests**

Create `ui/src/components/imagePreview/usePreviewStageSize.test.tsx`:

```tsx
import { act, render, screen } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { usePreviewStageSize } from './usePreviewStageSize'

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('usePreviewStageSize', () => {
  it('keeps zero measurements pending until ResizeObserver publishes a real stage', () => {
    const resize = installResizeObserver()
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(0, 0))

    render(<StageHarness />)
    const stage = screen.getByTestId('preview-stage')
    expect(screen.getByRole('status')).toHaveTextContent('0×0')

    resize.trigger(stage, Number.NaN, 800)
    expect(screen.getByRole('status')).toHaveTextContent('0×0')

    resize.trigger(stage, 1512.4, 982.6)
    expect(screen.getByRole('status')).toHaveTextContent('1512×983')
  })

  it('uses a real bounding rectangle when ResizeObserver is unavailable', () => {
    vi.stubGlobal('ResizeObserver', undefined)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(1280, 720))

    render(<StageHarness />)

    expect(screen.getByRole('status')).toHaveTextContent('1280×720')
  })

  it('disconnects the stage observer on unmount', () => {
    const resize = installResizeObserver()
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(0, 0))
    const rendered = render(<StageHarness />)
    const stage = screen.getByTestId('preview-stage')

    rendered.unmount()

    expect(resize.disconnected(stage)).toBe(1)
  })
})

function StageHarness() {
  const stage = useRef<HTMLDivElement>(null)
  const size = usePreviewStageSize(stage)
  return (
    <>
      <div ref={stage} data-testid="preview-stage" />
      <output>{size.width}×{size.height}</output>
    </>
  )
}

function installResizeObserver() {
  const callbacks = new Map<Element, ResizeObserverCallback>()
  const disconnects = new Map<Element, number>()
  class Observer {
    private node: Element | null = null
    private readonly callback: ResizeObserverCallback
    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }
    observe(node: Element) {
      this.node = node
      callbacks.set(node, this.callback)
    }
    disconnect() {
      if (this.node === null) return
      callbacks.delete(this.node)
      disconnects.set(this.node, (disconnects.get(this.node) ?? 0) + 1)
      this.node = null
    }
  }
  vi.stubGlobal('ResizeObserver', Observer)
  return {
    disconnected: (node: Element) => disconnects.get(node) ?? 0,
    trigger: (node: Element, width: number, height: number) => {
      act(() => {
        callbacks.get(node)?.(
          [{ target: node, contentRect: { width, height } } as ResizeObserverEntry],
          {} as ResizeObserver,
        )
      })
    },
  }
}

function rect(width: number, height: number): DOMRect {
  return {
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: width,
    bottom: height,
    width,
    height,
    toJSON: () => undefined,
  }
}
```

- [ ] **Step 2: Run the new hook test and verify the missing module failure**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/usePreviewStageSize.test.tsx
```

Expected: FAIL because `./usePreviewStageSize` does not exist.

- [ ] **Step 3: Implement the real-size-only hook**

Create `ui/src/components/imagePreview/usePreviewStageSize.ts`:

```ts
import { type RefObject, useLayoutEffect, useState } from 'react'
import type { Size } from './imageGeometry'

const EMPTY_STAGE: Size = { width: 0, height: 0 }

export function usePreviewStageSize(stage: RefObject<HTMLElement | null>): Size {
  const [size, setSize] = useState<Size>(EMPTY_STAGE)

  useLayoutEffect(() => {
    const node = stage.current
    if (node === null) return

    const publish = (candidate: Size) => {
      if (!isPositiveSize(candidate)) return
      const next = {
        width: Math.round(candidate.width),
        height: Math.round(candidate.height),
      }
      setSize((current) =>
        current.width === next.width && current.height === next.height ? current : next,
      )
    }

    const bounds = node.getBoundingClientRect()
    publish({
      width: bounds.width || node.clientWidth,
      height: bounds.height || node.clientHeight,
    })

    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver((entries) => {
      const content = entries[0]?.contentRect
      if (content !== undefined) publish(content)
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [stage])

  return size
}

export function isPositiveSize(size: Size): boolean {
  return (
    Number.isFinite(size.width) &&
    Number.isFinite(size.height) &&
    size.width > 0 &&
    size.height > 0
  )
}
```

- [ ] **Step 4: Run the hook tests and type checks**

Run:

```bash
pnpm --dir ui test -- src/components/imagePreview/usePreviewStageSize.test.tsx
pnpm --dir ui check
```

Expected: all hook tests PASS and TypeScript plus Biome report no errors.

- [ ] **Step 5: Commit the isolated measurement primitive**

Run:

```bash
git diff --check
git add \
  ui/src/components/imagePreview/usePreviewStageSize.ts \
  ui/src/components/imagePreview/usePreviewStageSize.test.tsx
git commit -m "feat: measure actual preview stage size"
```

### Task 2: Gate The First Image By Committed Real Geometry

**Files:**
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`

**Interfaces:**
- Consumes: `usePreviewStageSize(stage): Size` and `isPositiveSize(size): boolean` from Task 1.
- Produces: `previewReady`, true only when the current entity has committed a valid stage, source and representation together.

- [ ] **Step 1: Add deterministic preview-stage measurement to the component tests**

In `ui/src/components/ImagePreview.test.tsx`, extend the Vitest import:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
```

After `POINTER_CLIENT_POINT`, add this harness:

```ts
const nativeGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect
const previewResizeCallbacks = new Map<Element, ResizeObserverCallback>()
let initialPreviewStage = { width: 640, height: 480 }

beforeEach(() => {
  previewResizeCallbacks.clear()
  initialPreviewStage = { width: 640, height: 480 }
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function () {
    if (this.classList.contains('image-preview-stage')) {
      return stageRect(initialPreviewStage.width, initialPreviewStage.height)
    }
    return nativeGetBoundingClientRect.call(this)
  })
  class Observer {
    private node: Element | null = null
    private readonly callback: ResizeObserverCallback
    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }
    observe(node: Element) {
      this.node = node
      previewResizeCallbacks.set(node, this.callback)
    }
    disconnect() {
      if (this.node !== null) previewResizeCallbacks.delete(this.node)
      this.node = null
    }
  }
  vi.stubGlobal('ResizeObserver', Observer)
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

function publishPreviewStage(stage: Element, width: number, height: number) {
  act(() => {
    previewResizeCallbacks.get(stage)?.(
      [{ target: stage, contentRect: { width, height } } as ResizeObserverEntry],
      {} as ResizeObserver,
    )
  })
}

function stageRect(width: number, height: number): DOMRect {
  return {
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: width,
    bottom: height,
    width,
    height,
    toJSON: () => undefined,
  }
}
```

This gives existing tests a real 640×480 test stage without relying on product
fallback behavior. Individual gesture tests may continue to spy on their stage
instance after render.

- [ ] **Step 2: Write the zero-to-real stage regression**

Add this test immediately after the existing progressive proxy test:

```tsx
it('waits for a real stage and never commits the 576×384 fallback image', async () => {
  initialPreviewStage = { width: 0, height: 0 }
  const fit = deferred<ImageRepresentation>()
  const committedSizes: Array<{ width: number; height: number }> = []
  const target = image(1)
  const request = vi.fn((_file: BrowserFile, representation: ImageRepresentationRequest) =>
    representation.kind === 'fit_preview'
      ? fit.promise
      : new Promise<ImageRepresentation>(() => undefined),
  )

  const view = render(
    <Profiler
      id="real-stage-preview"
      onRender={() => {
        const preview = document.querySelector<HTMLElement>('.image-preview-image')
        if (preview) committedSizes.push(visibleImageSize(preview))
      }}
    >
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />
    </Profiler>,
  )

  await act(async () => fit.resolve(loaded('fit', 2400, 1600)))
  expect(screen.queryByRole('img', { name: '1.jpg' })).not.toBeInTheDocument()

  const stage = view.container.querySelector('.image-preview-stage') as HTMLElement
  publishPreviewStage(stage, 2048, 1060)
  const preview = await screen.findByRole('img', { name: '1.jpg' })

  expect(visibleImageSize(preview)).toEqual({ width: 1431, height: 954 })
  expect(committedSizes.length).toBeGreaterThan(0)
  expect(committedSizes).not.toContainEqual({ width: 576, height: 384 })
  expect(committedSizes.every(({ width }) => width > 1000)).toBe(true)
})
```

- [ ] **Step 3: Run the component test and verify the fallback is exposed**

Run:

```bash
pnpm --dir ui test -- src/components/ImagePreview.test.tsx
```

Expected: FAIL in the new regression because the current `DEFAULT_STAGE`
publishes 640×480 and mounts a 576×384 image before the observer update.

- [ ] **Step 4: Replace fallback measurement with the hook and commit readiness atomically**

In `ui/src/components/ImagePreview.tsx`, import the Task 1 hook:

```ts
import {
  isPositiveSize,
  usePreviewStageSize,
} from './imagePreview/usePreviewStageSize'
```

Delete:

```ts
const DEFAULT_STAGE: Size = { width: 640, height: 480 }
```

Delete the `stageSize` state and the entire effect that reads
`getBoundingClientRect`, inserts `DEFAULT_STAGE`, and creates a
`ResizeObserver`. Replace the state declaration with:

```ts
const stageSize = usePreviewStageSize(stage)
const [readyEntityId, setReadyEntityId] = useState<string | null>(null)
```

Replace the current measurement layout effect with:

```ts
useLayoutEffect(() => {
  viewport.setMeasurements(stageSize, sourceDimensions)
  if (
    representation !== null &&
    representation !== undefined &&
    isPositiveSize(stageSize) &&
    isPositiveSize(sourceDimensions)
  ) {
    setReadyEntityId((current) => (current === file.entityId ? current : file.entityId))
  }
}, [
  file.entityId,
  representation,
  sourceDimensions,
  stageSize,
  viewport.setMeasurements,
])

const previewReady = readyEntityId === file.entityId
```

The stage/source geometry and `readyEntityId` state updates occur in the same
layout-effect flush, so the browser cannot paint a ready image with the prior
geometry. Do not clear `readyEntityId` in a passive effect; entity equality
hides the prior image immediately during navigation.

Change the image branch to:

```tsx
previewReady && representation && renderedSource.width > 0 && renderedSource.height > 0
```

Until Task 3 replaces the presentation, change the current informational
feedback condition from a missing representation check to:

```tsx
!previewReady
```

while keeping the existing previewable, unavailable and fatal-failure guards.

- [ ] **Step 5: Run readiness, viewport and gesture regressions**

Run:

```bash
pnpm --dir ui test -- \
  src/components/ImagePreview.test.tsx \
  src/components/imagePreview/useImageViewport.test.tsx \
  src/components/imagePreview/usePreviewGestures.test.tsx \
  src/components/imagePreview/ImageMagnifier.test.tsx
pnpm --dir ui check
```

Expected: all tests PASS. The existing proxy-to-original profiler test must
still report one stable visible size.

- [ ] **Step 6: Commit the stable first-reveal gate**

Run:

```bash
git diff --check
git add ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx
git commit -m "fix: reveal previews after real geometry"
```

### Task 3: Add The Dynamic Indeterminate Loading Presentation

**Files:**
- Create: `ui/src/components/imagePreview/ImagePreviewLoading.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: `visible: boolean` derived from preview readiness and fatal state.
- Produces: `ImagePreviewLoading({ visible })`, a persistent fade-capable status surface with an indeterminate `progressbar`.

- [ ] **Step 1: Write loading semantics and motion contract tests**

In the existing `keeps the formal toolbar mounted while loading and contains
failure feedback in the stage` test in `ui/src/components/ImagePreview.test.tsx`,
replace the loading assertions with:

```tsx
const loadingStage = pending.container.querySelector('.image-preview-stage') as HTMLElement
expect(screen.getByRole('toolbar', { name: '图片预览工具' })).toHaveClass('viewer-toolbar')
const loading = within(loadingStage).getByRole('status')
expect(loading).toHaveClass('image-preview-loading')
expect(loading).toHaveTextContent('正在载入图片')
expect(loading).toHaveTextContent('正在准备高清预览…')
const progress = within(loading).getByRole('progressbar', { name: '正在准备高清预览' })
expect(progress).not.toHaveAttribute('aria-valuenow')
expect(progress).not.toHaveAttribute('aria-valuemin')
expect(progress).not.toHaveAttribute('aria-valuemax')
```

Add this recoverable-race test after the toolbar/loading test:

```tsx
it('keeps progress active when fit fails while original can still recover', async () => {
  const fit = deferred<ImageRepresentation>()
  const original = deferred<ImageRepresentation>()
  const target = image(1)
  const request = vi.fn((_file: BrowserFile, representation: ImageRepresentationRequest) =>
    representation.kind === 'fit_preview' ? fit.promise : original.promise,
  )
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

  await waitFor(() => expect(request).toHaveBeenCalledTimes(2))
  await act(async () => fit.reject(new Error('fit failed')))
  expect(screen.getByRole('progressbar', { name: '正在准备高清预览' })).toBeVisible()
  expect(screen.queryByRole('alert')).not.toBeInTheDocument()

  await act(async () => original.resolve(loaded('original', 6000, 4000)))
  expect(await screen.findByRole('img', { name: '1.jpg' })).toHaveAttribute(
    'data-representation',
    'original',
  )
  expect(screen.getByTestId('image-preview-loading')).toHaveAttribute('data-visible', 'false')
})
```

At the end of the zero-to-real stage regression from Task 2, add:

```tsx
expect(screen.getByTestId('image-preview-loading')).toHaveAttribute('data-visible', 'false')
expect(screen.getByTestId('image-preview-loading')).toHaveAttribute('aria-hidden', 'true')
expect(preview).toHaveAttribute('data-initial-reveal', 'true')
```

In `ui/src/styles/app.test.ts`, add:

```ts
it('uses indeterminate preview progress and opacity-only reduced motion', () => {
  const rules = parseRules(appCss)
  const loading = rules.find(({ selector }) => selector === '.image-preview-loading')
  const hidden = rules.find(
    ({ selector }) => selector === '.image-preview-loading[data-visible="false"]',
  )
  const track = rules.find(
    ({ selector }) => selector === '.image-preview-loading__progress',
  )
  const indicator = rules.find(
    ({ selector }) => selector === '.image-preview-loading__indicator',
  )
  const reveal = rules.find(
    ({ selector }) =>
      selector === '.image-preview-stage > .image-preview-image[data-initial-reveal="true"]',
  )

  expect(loading?.declarations.transition).toContain('180ms')
  expect(hidden?.declarations).toMatchObject({ opacity: '0', visibility: 'hidden' })
  expect(track?.declarations).toMatchObject({ height: '4px', overflow: 'hidden' })
  expect(indicator?.declarations.animation).toContain('preview-loading-sweep')
  expect(reveal?.declarations.animation).toBe('preview-image-reveal 180ms ease-out both')
  expect(appCss).toContain('@keyframes preview-loading-sweep')
  expect(appCss).toContain('@keyframes preview-image-reveal')

  const reduced = parseRules(mediaBody(appCss, '(prefers-reduced-motion: reduce)'))
  for (const selector of [
    '.image-preview-loading',
    '.image-preview-loading__indicator',
    '.image-preview-stage > .image-preview-image[data-initial-reveal="true"]',
  ]) {
    expect(reduced.find((rule) => rule.selector === selector)?.declarations).toMatchObject({
      animation: 'none',
      transition: 'none',
    })
  }
})
```

- [ ] **Step 2: Run the tests and verify the loading component and CSS are absent**

Run:

```bash
pnpm --dir ui test -- src/components/ImagePreview.test.tsx src/styles/app.test.ts
```

Expected: FAIL because the current loading state is `ViewerLocalFeedback`, has
no nested progressbar, and the preview loading/reveal CSS selectors do not
exist.

- [ ] **Step 3: Create the focused loading component**

Create `ui/src/components/imagePreview/ImagePreviewLoading.tsx`:

```tsx
interface ImagePreviewLoadingProps {
  visible: boolean
}

export default function ImagePreviewLoading({ visible }: ImagePreviewLoadingProps) {
  return (
    <section
      className="image-preview-loading"
      data-testid="image-preview-loading"
      data-visible={visible}
      aria-hidden={visible ? undefined : true}
      role="status"
    >
      <strong>正在载入图片</strong>
      <p>正在准备高清预览…</p>
      <div
        className="image-preview-loading__progress"
        role="progressbar"
        aria-label="正在准备高清预览"
      >
        <span className="image-preview-loading__indicator" />
      </div>
    </section>
  )
}
```

- [ ] **Step 4: Integrate loading visibility without disturbing error handling**

In `ui/src/components/ImagePreview.tsx`, import:

```ts
import ImagePreviewLoading from './imagePreview/ImagePreviewLoading'
```

Before constructing `previewStage`, derive:

```ts
const fatalImageFailure = isFatalImageFailure(error, currentOriginal.status)
const showPreviewLoading =
  !unavailable && isPreviewableImage(file) && !fatalImageFailure && !previewReady
```

Remove the informational `ViewerLocalFeedback` loading branch and render this
after the image branch:

```tsx
{!unavailable && isPreviewableImage(file) && !fatalImageFailure && (
  <ImagePreviewLoading visible={showPreviewLoading} />
)}
```

Reuse `fatalImageFailure` in the fatal alert condition instead of invoking
`isFatalImageFailure` again. Add this attribute to the preview image:

```tsx
data-initial-reveal="true"
```

Do not key the image by representation URL. This keeps the same image DOM node
when the fit proxy upgrades to original pixels and prevents the reveal
animation from replaying.

- [ ] **Step 5: Add the progress sweep, fade and reduced-motion CSS**

In `ui/src/styles/app.css`, insert these rules after
`.image-preview-stage > .viewer-local-feedback` and before the base preview
image rule:

```css
.image-preview-loading {
  display: grid;
  gap: 8px;
  left: 50%;
  opacity: 1;
  position: absolute;
  text-align: center;
  top: 50%;
  transform: translate(-50%, -50%);
  transition:
    opacity 180ms ease-out,
    visibility 0s linear 0s;
  visibility: visible;
  width: min(320px, calc(100% - 48px));
  z-index: 1;
}

.image-preview-loading strong {
  color: var(--viewer-text);
  font-size: 13px;
}

.image-preview-loading p {
  color: var(--viewer-text-secondary);
  font-size: 12px;
  margin: 0;
}

.image-preview-loading[data-visible="false"] {
  opacity: 0;
  transition:
    opacity 180ms ease-out,
    visibility 0s linear 180ms;
  visibility: hidden;
}

.image-preview-loading__progress {
  background: var(--viewer-border);
  border-radius: 999px;
  height: 4px;
  margin-top: 4px;
  overflow: hidden;
}

.image-preview-loading__indicator {
  animation: preview-loading-sweep 900ms ease-in-out infinite;
  background: linear-gradient(90deg, transparent, var(--viewer-accent), transparent);
  display: block;
  height: 100%;
  transform: translateX(-120%);
  width: 42%;
}

@keyframes preview-loading-sweep {
  from {
    transform: translateX(-120%);
  }

  to {
    transform: translateX(350%);
  }
}

.image-preview-stage > .image-preview-image[data-initial-reveal="true"] {
  animation: preview-image-reveal 180ms ease-out both;
}

@keyframes preview-image-reveal {
  from {
    opacity: 0;
  }

  to {
    opacity: 1;
  }
}
```

Inside the existing `@media (prefers-reduced-motion: reduce)` block, add:

```css
.image-preview-loading {
  animation: none;
  transition: none;
}

.image-preview-loading__indicator {
  animation: none;
  transition: none;
}

.image-preview-stage > .image-preview-image[data-initial-reveal="true"] {
  animation: none;
  transition: none;
}
```

- [ ] **Step 6: Run loading, styling and visual-accessibility verification**

Run:

```bash
pnpm --dir ui test -- \
  src/components/ImagePreview.test.tsx \
  src/styles/app.test.ts \
  src/styles/visualAccessibility.test.ts
pnpm --dir ui check
```

Expected: all tests PASS; the loading progressbar has no determinate ARIA
values and reduced-motion rules disable every new animation and transition.

- [ ] **Step 7: Commit the loading presentation**

Run:

```bash
git diff --check
git add \
  ui/src/components/imagePreview/ImagePreviewLoading.tsx \
  ui/src/components/ImagePreview.tsx \
  ui/src/components/ImagePreview.test.tsx \
  ui/src/styles/app.css \
  ui/src/styles/app.test.ts
git commit -m "feat: animate stable preview loading"
```

### Task 4: Complete Regression Verification And Launch The Development Build

**Files:**
- Verify: all files changed by both 2026-08-09 implementation plans
- Verify: `docs/superpowers/specs/2026-08-09-viewer-bottom-shelf-preview-loading-design.md`

**Interfaces:**
- Consumes: the committed bottom-shelf layout, actual stage hook, readiness gate and loading presentation.
- Produces: a verified development binary running from the current branch and commit.

- [ ] **Step 1: Run the complete focused feature suite together**

Run:

```bash
pnpm --dir ui test -- \
  src/components/AspectVirtualGrid.test.tsx \
  src/components/ContentBrowser.test.tsx \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/components/ImagePreview.test.tsx \
  src/components/imagePreview/usePreviewStageSize.test.tsx \
  src/components/imagePreview/useImageViewport.test.tsx \
  src/components/imagePreview/usePreviewGestures.test.tsx \
  src/components/imagePreview/ImageMagnifier.test.tsx \
  src/styles/adaptiveOtherFilePanel.test.ts \
  src/styles/app.test.ts \
  src/styles/visualAccessibility.test.ts
```

Expected: all focused tests PASS.

- [ ] **Step 2: Run repository verification and quality reports**

Run:

```bash
pnpm verify
pnpm quality:report
```

Expected: policy, clean-tree harness, UI checks/tests/build, Rust formatting,
Clippy, Rust tests, security checks, UI coverage, Rust coverage and
architecture health all PASS.

- [ ] **Step 3: Confirm the worktree contains only intended committed changes**

Run:

```bash
git status --short
git log --oneline -6
```

Expected: `git status --short` prints nothing. The recent history contains the
layout, actual-stage, readiness and loading commits from these plans.

- [ ] **Step 4: Restart Viewer from the verified current commit**

Run:

```bash
pnpm start:viewer
```

Expected output includes `Viewer development version is running:` and a
`Source: codex/viewer-trackpad-magnifier @ ...` line whose short SHA exactly
matches the output of `git rev-parse --short HEAD`.

The launcher replaces the prior development Viewer process with the binary
built from this worktree.

- [ ] **Step 5: Physically verify the two reported scenarios**

In the running development Viewer:

1. Open the mixed folder shown in the report. Confirm `其它文件 · 1` is pinned
   to the window bottom, its collapsed height is one row, expanding it shows
   one file row, and collapsing it restores the image viewport without losing
   scroll position.
2. Open the reported 6308×4205 image. Confirm the 576×384 temporary image never
   appears, the indeterminate progress bar is visible during a perceptible
   load, and the image first appears at its final fitted size.
3. Wait for original pixels, then use zoom, two-finger pan/pinch, magnifier,
   rotation, arrow navigation and close. Confirm the fit proxy upgrade does not
   resize the image or replay the reveal animation.

Record the running PID, branch and commit in the completion handoff. If either
physical scenario fails, keep the task open and return to the first failing
focused regression instead of declaring completion.
