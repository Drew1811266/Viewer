# Viewer Compare Original Images and Eight-Item Limit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make multi-image comparison accept exactly 2–8 images, disable the radial compare action above eight without truncating selection, and automatically replace transient fit proxies with original-resolution representations in every mounted pane.

**Architecture:** Keep compare cardinality in the existing shared policy, then let all UI and state entry points consume that policy. Extract original-image scheduling from `CompareWorkspace` into a focused cancellation-aware FIFO scheduler with concurrency two; each mounted `ComparePane` requests a transient fit proxy and the original, always preferring a current original. Existing Image I/O limits remain the safety boundary.

**Tech Stack:** React 19, TypeScript 6, Vitest/Testing Library, Tauri 2, Rust, macOS Image I/O, Node native-acceptance scripts.

## Global Constraints

- Comparison accepts 2–8 supported unique images; 1 or 9+ is invalid.
- Above eight, radial “并排对比” stays visible but disabled with “最多同时对比 8 张图片”.
- Never truncate or partially accept a 9+ item selection.
- Every mounted previewable pane requests `original100_percent`; `fit_preview` is only a transient placeholder or explicit fallback.
- “适应窗口” and “100%” change geometry only, never representation source.
- At most two original requests run concurrently; queued work is FIFO and cancellation-aware.
- Keep the existing 100,000,000-pixel and 700,000,000-byte per-image limits.
- Do not expose source paths or original bytes to the WebView.
- Native acceptance must not change display scaling, resolution, Dock, global scrollbars, or other system settings.
- Run one bounded native acceptance sequence; no repeated file-opening or screenshot loops.

## File Structure

- `ui/src/state/comparePolicy.ts`: source of truth for 2–8 validation and copy.
- `ui/src/components/radialMenuModel.ts`: radial disabled state and reason.
- `ui/src/components/compareOriginalRequestScheduler.ts`: new two-wide FIFO scheduler.
- `ui/src/components/ComparePane.tsx`: proxy/original lifecycle and original-first selection.
- `ui/src/components/CompareWorkspace.tsx`: scheduler ownership and geometry mode.
- Existing colocated tests: TDD coverage.
- Acceptance scene/scripts: formal eight-image evidence.
- macOS Image I/O test: full oriented original dimensions and safety regression.

---

### Task 1: Enforce the Shared Eight-Image Policy

**Files:**
- Modify: `ui/src/state/comparePolicy.ts`
- Modify: `ui/src/state/comparePolicy.test.ts`
- Modify: `ui/src/components/radialMenuModel.ts`
- Modify: `ui/src/components/radialMenuModel.test.ts`
- Modify: `ui/src/components/RadialFileMenu.test.tsx`
- Modify: `ui/src/state/viewerReducer.test.ts`
- Modify: `ui/src/state/compareModel.test.ts`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/CompareWorkspace.test.tsx`

**Interfaces:**
- Consumes: existing `validateCompareCandidates(candidates)` and `MAX_COMPARE_IMAGES` consumers.
- Produces: `MAX_COMPARE_IMAGES = 8`, `compareValidationMessage(...) === '请选择 2–8 张图片进行对比。'`, and `最多同时对比 8 张图片`.

- [ ] **Step 1: Write failing boundary and radial tests**

```ts
it('accepts exactly 2 through 8 supported unique images', () => {
  expect(validateCompareCandidates([image(0), image(1)])).toEqual({ ok: true })
  expect(validateCompareCandidates(Array.from({ length: 8 }, (_, index) => image(index)))).toEqual({ ok: true })
  expect(validateCompareCandidates(Array.from({ length: 9 }, (_, index) => image(index)))).toEqual({
    ok: false,
    reason: 'invalid_cardinality',
  })
})

it('enables compare at 8 and disables it at 9', () => {
  expect(compareItem({ selectedCount: 8, selectedImageCount: 8 })).toMatchObject({ disabled: false })
  expect(compareItem({ selectedCount: 9, selectedImageCount: 9 })).toMatchObject({
    disabled: true,
    disabledReason: '最多同时对比 8 张图片',
  })
})
```

Update reducer/model tests to accept eight IDs and reject a nine-ID action while preserving the previous eight IDs. Update App, radial component, and invalid-workspace assertions from `2–20`/`20` to `2–8`/`8`.

- [ ] **Step 2: Run the focused tests and verify failure**

```bash
pnpm --dir ui test -- src/state/comparePolicy.test.ts src/components/radialMenuModel.test.ts src/components/RadialFileMenu.test.tsx src/state/viewerReducer.test.ts src/state/compareModel.test.ts src/App.test.tsx src/components/CompareWorkspace.test.tsx
```

Expected: FAIL on the old 20-image constant and copy.

- [ ] **Step 3: Implement the shared limit and copy**

```ts
export const MIN_COMPARE_IMAGES = 2
export const MAX_COMPARE_IMAGES = 8

export function compareValidationMessage(_reason: CompareValidationReason): string {
  return '请选择 2–8 张图片进行对比。'
}
```

Keep the radial reason decision order and derive both numbers from the shared constants:

```ts
context.selectedCount > MAX_COMPARE_IMAGES
  ? `最多同时对比 ${MAX_COMPARE_IMAGES} 张图片`
  : context.selectedCount < MIN_COMPARE_IMAGES
    ? `请选择 ${MIN_COMPARE_IMAGES}–${MAX_COMPARE_IMAGES} 张图片`
    : context.selectedImageCount !== context.selectedCount
      ? '仅支持图片'
      : undefined
```

- [ ] **Step 4: Run focused tests and typecheck**

```bash
pnpm --dir ui test -- src/state/comparePolicy.test.ts src/components/radialMenuModel.test.ts src/components/RadialFileMenu.test.tsx src/state/viewerReducer.test.ts src/state/compareModel.test.ts src/App.test.tsx src/components/CompareWorkspace.test.tsx
pnpm --dir ui typecheck
```

Expected: PASS and TypeScript exits 0.

- [ ] **Step 5: Commit**

```bash
git add ui/src/state/comparePolicy.ts ui/src/state/comparePolicy.test.ts ui/src/components/radialMenuModel.ts ui/src/components/radialMenuModel.test.ts ui/src/components/RadialFileMenu.test.tsx ui/src/state/viewerReducer.test.ts ui/src/state/compareModel.test.ts ui/src/App.test.tsx ui/src/components/CompareWorkspace.test.tsx
git commit -m "fix: cap image comparison at eight"
```

### Task 2: Add a Fair Bounded Original Scheduler

**Files:**
- Create: `ui/src/components/compareOriginalRequestScheduler.ts`
- Create: `ui/src/components/compareOriginalRequestScheduler.test.ts`

**Interfaces:**
- Consumes: stable job key, `run: () => Promise<T>`, optional `AbortSignal`.
- Produces: `createCompareOriginalRequestScheduler<T>(maxConcurrent?: number)` with `enqueue(key, run, signal)` and `dispose()`.

- [ ] **Step 1: Write scheduler tests**

```ts
const scheduler = createCompareOriginalRequestScheduler<string>(2)
const first = deferred<string>()
const second = deferred<string>()
const third = deferred<string>()
const firstRun = vi.fn(() => first.promise)
const secondRun = vi.fn(() => second.promise)
const thirdRun = vi.fn(() => third.promise)
const jobs = [
  scheduler.enqueue('a:1', firstRun),
  scheduler.enqueue('b:1', secondRun),
  scheduler.enqueue('c:1', thirdRun),
]
expect(firstRun).toHaveBeenCalledOnce()
expect(secondRun).toHaveBeenCalledOnce()
expect(thirdRun).not.toHaveBeenCalled()
first.resolve('a')
await expect(jobs[0]).resolves.toBe('a')
expect(thirdRun).toHaveBeenCalledOnce()
```

Add separate cases for FIFO order, duplicate-key coalescing, queued cancellation, running cancellation observation, disposal, and rejection of new work after disposal. Cancellation must reject with `{ code: 'image_request_cancelled' }`.

- [ ] **Step 2: Run the missing-module test**

```bash
pnpm --dir ui test -- src/components/compareOriginalRequestScheduler.test.ts
```

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement the generic scheduler**

```ts
export interface CompareOriginalRequestScheduler<T> {
  enqueue(key: string, run: () => Promise<T>, signal?: AbortSignal): Promise<T>
  dispose(): void
}

interface Job<T> {
  key: string
  run: () => Promise<T>
  signal?: AbortSignal
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (reason: unknown) => void
  running: boolean
  settled: boolean
  removeAbortListener: () => void
}

const cancelledImageRequest = () => ({ code: 'image_request_cancelled' as const })

export function createCompareOriginalRequestScheduler<T>(
  maxConcurrent = 2,
): CompareOriginalRequestScheduler<T> {
  if (!Number.isInteger(maxConcurrent) || maxConcurrent < 1) {
    throw new Error('Original request concurrency must be a positive integer')
  }
  let disposed = false
  let running = 0
  const queued: Job<T>[] = []
  const jobs = new Map<string, Job<T>>()

  const finish = (job: Job<T>, result: { value: T } | { reason: unknown }) => {
    if (job.settled) return
    job.settled = true
    job.removeAbortListener()
    jobs.delete(job.key)
    if ('value' in result) job.resolve(result.value)
    else job.reject(result.reason)
  }

  const pump = () => {
    while (!disposed && running < maxConcurrent && queued.length > 0) {
      const job = queued.shift()!
      if (job.signal?.aborted) {
        finish(job, { reason: cancelledImageRequest() })
        continue
      }
      job.running = true
      running += 1
      void job.run().then(
        (value) => finish(job, job.signal?.aborted ? { reason: cancelledImageRequest() } : { value }),
        (reason: unknown) => finish(job, { reason: job.signal?.aborted ? cancelledImageRequest() : reason }),
      ).finally(() => {
        running -= 1
        pump()
      })
    }
  }

  return {
    enqueue(key, run, signal) {
      const existing = jobs.get(key)
      if (existing !== undefined) return existing.promise
      if (disposed || signal?.aborted) return Promise.reject(cancelledImageRequest())

      let resolve!: (value: T) => void
      let reject!: (reason: unknown) => void
      const promise = new Promise<T>((accept, decline) => {
        resolve = accept
        reject = decline
      })
      const job: Job<T> = {
        key,
        run,
        signal,
        promise,
        resolve,
        reject,
        running: false,
        settled: false,
        removeAbortListener: () => undefined,
      }
      const abort = () => {
        const queuedIndex = queued.indexOf(job)
        if (!job.running && queuedIndex >= 0) queued.splice(queuedIndex, 1)
        finish(job, { reason: cancelledImageRequest() })
        pump()
      }
      job.removeAbortListener = () => signal?.removeEventListener('abort', abort)
      signal?.addEventListener('abort', abort, { once: true })
      jobs.set(key, job)
      queued.push(job)
      pump()
      return promise
    },
    dispose() {
      disposed = true
      for (const job of [...jobs.values()]) finish(job, { reason: cancelledImageRequest() })
      queued.length = 0
    },
  }
}
```

Duplicate keys return the existing promise without invoking another `run`.

- [ ] **Step 4: Run scheduler tests and UI checks**

```bash
pnpm --dir ui test -- src/components/compareOriginalRequestScheduler.test.ts
pnpm --dir ui check
```

Expected: PASS with no Biome/TypeScript error.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/compareOriginalRequestScheduler.ts ui/src/components/compareOriginalRequestScheduler.test.ts
git commit -m "feat: queue compare originals fairly"
```

### Task 3: Make Every Mounted Pane Prefer Its Original

**Files:**
- Modify: `ui/src/components/ComparePane.tsx`
- Modify: `ui/src/components/ComparePane.test.tsx`

**Interfaces:**
- Consumes: existing `requestImage(file, representation, signal)`.
- Produces: `ComparePane` without `useOriginal`; automatic proxy and original requests; current-original-first display with proxy fallback.

- [ ] **Step 1: Write automatic original-first tests**

```ts
it('shows a proxy transiently and replaces it with the original without changing geometry', async () => {
  const resize = installResizeObserver()
  const proxy = deferred<ImageRepresentation>()
  const original = deferred<ImageRepresentation>()
  const requestImage = vi.fn((_file, request) =>
    request.kind === 'original100_percent' ? original.promise : proxy.promise,
  )
  renderPane({ requestImage })
  act(() => resize(800, 600))

  await act(async () => proxy.resolve(representation('proxy-a', 800, 600)))
  expect(screen.getByRole('img', { name: 'front.jpg' })).toHaveAttribute(
    'src',
    'viewer-image://localhost/proxy-a',
  )
  await act(async () => original.resolve(representation('original-a', 4_000, 3_000)))
  expect(screen.getByRole('img', { name: 'front.jpg' })).toHaveAttribute(
    'src',
    'viewer-image://localhost/original-a',
  )
})
```

Also assert both requests receive abort signals; source revision changes invalidate both; unmount aborts both; budget/load failures retain the proxy and call `onOriginalUnavailable`; marker-only changes do not request again; a late old original cannot replace a new entity.

- [ ] **Step 2: Run the pane test and verify failure**

```bash
pnpm --dir ui test -- src/components/ComparePane.test.tsx
```

Expected: FAIL because current original loading requires `useOriginal=true`.

- [ ] **Step 3: Remove mode gating and prefer the current original**

Remove `useOriginal` from props, component parameters, effects, helper calls, and tests. Start the original request for every previewable file:

```ts
useEffect(() => {
  if (!previewable) return
  const revision = ++originalRevision.current
  const controller = new AbortController()
  const entityId = file.entityId
  const sourceRevision = fileSourceRevision(file)
  setOriginalError(null)
  void requestImage(fileRef.current, { kind: 'original100_percent' }, controller.signal).then(
    (image) => {
      if (originalRevision.current === revision) setOriginal({ entityId, sourceRevision, image })
    },
    (caught: unknown) => {
      if (originalRevision.current !== revision || requestWasAborted(caught, controller.signal)) return
      const budgetExceeded = commandCode(caught) === 'image_budget_exceeded'
      setOriginalError(
        budgetExceeded
          ? '原图超出安全预览限制，继续使用适窗代理。'
          : '无法加载原图，继续使用适窗代理。',
      )
      originalUnavailableRef.current(entityId, budgetExceeded ? 'budget' : 'load')
    },
  )
  return () => controller.abort()
}, [file.entityId, file.kind, file.modifiedNs, file.size, previewable, requestImage])
```

Make `visibleRepresentation` return a matching original first, then a matching proxy. Continue preferring `file.imageMetadata` for geometry so replacement does not jump.

- [ ] **Step 4: Run pane tests and UI checks**

```bash
pnpm --dir ui test -- src/components/ComparePane.test.tsx
pnpm --dir ui check
```

Expected: PASS and no `useOriginal` reference remains in `ComparePane`.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/ComparePane.tsx ui/src/components/ComparePane.test.tsx
git commit -m "fix: show originals in compare panes"
```

### Task 4: Integrate Scheduling Without Coupling It to 100% Geometry

**Files:**
- Modify: `ui/src/components/CompareWorkspace.tsx`
- Modify: `ui/src/components/CompareWorkspace.test.tsx`
- Modify: `ui/src/components/CompareVirtualViewport.test.tsx`

**Interfaces:**
- Consumes: `createCompareOriginalRequestScheduler<ImageRepresentation>(2)` and `ComparePane` without `useOriginal`.
- Produces: scheduled original requests for every mounted pane; `actualSizeEntityId` tracks geometry only; local fallback without transform reset.

- [ ] **Step 1: Write workspace integration tests**

```ts
it('loads every mounted pane original through a two-wide FIFO without dropping jobs', async () => {
  const originals = new Map(['a', 'b', 'c', 'd'].map((id) => [id, deferred<ImageRepresentation>()]))
  const started: string[] = []
  const requestImage = vi.fn((file, request) => {
    if (request.kind !== 'original100_percent') {
      return Promise.resolve(representation(`proxy-${file.entityId}`, 800, 600))
    }
    started.push(file.entityId)
    return defined(originals.get(file.entityId), 'expected original').promise
  })
  renderWorkspace({ files, requestImage })
  await waitFor(() => expect(started).toEqual(['a', 'b']))
  await act(async () => originals.get('a')?.resolve(representation('original-a', 4_000, 3_000)))
  await waitFor(() => expect(started).toEqual(['a', 'b', 'c']))
})
```

Complete the test by settling all four and asserting all four original URLs appear. Add cases for removal/unmount cancellation, a local failure not resetting neighbor transforms, “适应窗口”/“100%” not requesting another original, and eight-image scroll virtualization requesting only mounted panes.

- [ ] **Step 2: Run workspace and viewport tests and verify failure**

```bash
pnpm --dir ui test -- src/components/CompareWorkspace.test.tsx src/components/CompareVirtualViewport.test.tsx
```

Expected: FAIL because the old lane drops queued requests and loading is tied to one `originalEntityId`.

- [ ] **Step 3: Replace the inline lane with the scheduler**

```ts
const schedulerRef = useRef<CompareOriginalRequestScheduler<ImageRepresentation> | null>(null)
if (schedulerRef.current === null) {
  schedulerRef.current = createCompareOriginalRequestScheduler<ImageRepresentation>(2)
}

const requestComparedImage = useCallback(
  (file: BrowserFile, representation: ImageRepresentationRequest, signal?: AbortSignal) => {
    if (representation.kind !== 'original100_percent') return requestImage(file, representation, signal)
    return schedulerRef.current!.enqueue(
      `${file.entityId}:${compareSourceRevision(file)}`,
      () => requestImage(file, representation, signal),
      signal,
    )
  },
  [requestImage],
)
```

Dispose the scheduler on unmount. Remove the inline `OriginalRequestJob`, `OriginalRequestLane`, `enqueueOriginalRequest`, `pumpOriginalLane`, and cancellation helpers.

- [ ] **Step 4: Separate geometry state from representation state**

Rename `originalEntityId` to `actualSizeEntityId`. Use it only to recompute actual-size geometry after metrics arrive; “适应窗口” clears it and “100%” sets it. Remove `useOriginal` from the pane call. In `onOriginalUnavailable`, publish the precise status but do not dispatch `fit`, because a local fallback must not reset zoom, pan, or synchronized transforms.

- [ ] **Step 5: Run integrated tests and UI checks**

```bash
pnpm --dir ui test -- src/components/CompareWorkspace.test.tsx src/components/CompareVirtualViewport.test.tsx src/components/ComparePane.test.tsx src/components/compareOriginalRequestScheduler.test.ts
pnpm --dir ui check
```

Expected: PASS; no old lane remains.

- [ ] **Step 6: Commit**

```bash
git add ui/src/components/CompareWorkspace.tsx ui/src/components/CompareWorkspace.test.tsx ui/src/components/CompareVirtualViewport.test.tsx
git commit -m "feat: load compare originals progressively"
```

### Task 5: Update Formal Acceptance and Prove Original Dimensions

**Files:**
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx`
- Modify: `ui/src/components/compareLayoutBenchmark.test.ts`
- Modify: `ui/src/components/compareLayoutEngine.test.ts`
- Modify: `scripts/viewer-native-acceptance.mjs`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Modify: `crates/viewer-platform-macos/src/image/image_io.rs`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`

**Interfaces:**
- Consumes: production 2–8 policy and `Original100Percent` Image I/O path.
- Produces: COM-04 as the eight-image scene and a backend regression for full oriented dimensions.

- [ ] **Step 1: Write the backend dimension regression**

```rust
#[test]
fn original_representation_preserves_oriented_source_dimensions() {
    let output = tempfile::NamedTempFile::new().unwrap();
    let dimensions = ImageIoBackend::default()
        .render_sync(
            image_fixture("rotated-6.jpg"),
            ImageRepresentationKind::Original100Percent,
            output.path(),
        )
        .unwrap();
    assert_eq!(dimensions, (600, 800));
    assert_eq!(png_dimensions(output.path()).unwrap(), dimensions);
}
```

- [ ] **Step 2: Change product-maximum fixtures from twenty to eight**

Use `count={8}` for COM-04, `selectImages(8)` in its native recipe, and expect eight product clicks in the recipe test. Change layout benchmark/model fixtures that specifically represent the product maximum to eight while retaining finite-runtime assertions. Keep unrelated numeric 20 values unchanged.

Update the COM-04 migration-ledger row to “Select exactly eight images” and “8-pane virtualization”. Preserve old evidence links as historical until the bounded new evidence run finishes.

- [ ] **Step 3: Run targeted acceptance contracts and backend test**

```bash
pnpm --dir ui test -- src/acceptance/scenes/viewingScenes.test.tsx src/components/compareLayoutBenchmark.test.ts src/components/compareLayoutEngine.test.ts
node --test scripts/viewer-native-acceptance.test.mjs
cargo test -p viewer-platform-macos image_io::tests::original_representation_preserves_oriented_source_dimensions --locked
```

Expected: PASS; backend output is 600×800, not viewport-sized.

- [ ] **Step 4: Commit**

```bash
git add ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx ui/src/components/compareLayoutBenchmark.test.ts ui/src/components/compareLayoutEngine.test.ts scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs crates/viewer-platform-macos/src/image/image_io.rs docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "test: cover eight-image original comparison"
```

### Task 6: Bounded Verification and Native Visual Acceptance

**Files:**
- Modify only if evidence is produced: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Generate ignored evidence under: `target/viewer-visual-acceptance/` and the normal Viewer session cache.

**Interfaces:**
- Consumes: all implementation slices.
- Produces: one clean repository verification and one bounded native proof using the user’s high-resolution images.

- [ ] **Step 1: Run complete repository verification exactly once**

```bash
pnpm verify
```

Expected: policy, UI checks/tests/build, Rust format/clippy/tests, dependencies, and security all PASS without changing tracked files.

- [ ] **Step 2: Launch exactly one canonical development app**

```bash
pnpm start:viewer
```

Expected: exactly one `target/debug/viewer-desktop` process from the implementation worktree.

- [ ] **Step 3: Perform one bounded native acceptance sequence**

Using `/Users/abc/Downloads/测试图/角色/B10/亚马逊小童6703.jpg` and `亚马逊小童6717.jpg`:

1. Enter two-image comparison once and wait for both panes to sharpen without layout or focus movement.
2. Inspect each generated `original100_percent` session representation once with `sips -g pixelWidth -g pixelHeight`; expect 4380×6570, not the former approximately 398×597 proxy.
3. Return once, select eight images, and confirm comparison opens.
4. Return once, select nine images, and confirm radial “并排对比” is disabled with “最多同时对比 8 张图片” while all nine stay selected.
5. Exit; do not repeat a passing checkpoint.

- [ ] **Step 4: Record evidence and verify worktree state**

Update the COM-04 evidence cell only if a new deterministic artifact exists. Then run:

```bash
git diff --check
git status --short
```

Expected: no uncommitted product changes; ignored cache/evidence is allowed.

- [ ] **Step 5: Commit an evidence-only update if needed**

```bash
git add docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "test: record original compare acceptance"
```

Skip this commit when the ledger did not change.
