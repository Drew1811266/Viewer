# Viewer Filmstrip Scrollbar and Thumbnail Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make each folder-filmstrip scrollbar appear only for row hover or keyboard focus, and make visible-content thumbnail progress settle correctly under React `StrictMode`.

**Architecture:** Keep the filmstrip's native `overflow-x: auto` viewport for all scrolling behavior, hide its platform-drawn scrollbar, and render a non-interactive visual thumb whose geometry follows the native viewport. CSS controls its resting, hover, focus, and reduced-motion states. Keep thumbnail accounting inside `ContentBrowser`, but correct its mounted-lifetime effect so StrictMode setup cleanup cannot permanently disable promise-settlement updates.

**Tech Stack:** React 19, TypeScript, CSS, Vitest, Testing Library, Tauri 2 WebView.

## Global Constraints

- Preserve native filmstrip scrolling, trackpad inertia, dragging, scroll anchoring, and virtualization.
- The visual scrollbar thumb is transparent at rest, visible for complete-row hover or `focus-within`, and fades over exactly 180 ms.
- Reduced-motion users receive the same states without a transition.
- Preserve the native scroll viewport and its input/accessibility behavior. The visual overlay must be non-interactive and must not change filmstrip geometry.
- Count visible-content thumbnail requests only; do not change project cache or backend protocols.
- Promise settlements after a real unmount must not update React state.
- Add no dependency and do not change task-bar dismissal or aggregation.
- Follow test-driven development: observe each regression test fail for the intended missing behavior before editing production code.

---

### Task 1: Folder filmstrip scrollbar visibility

**Files:**
- Modify: `ui/src/components/FolderFilmstripRow.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: existing native `.folder-filmstrip` `scrollLeft`, viewport width, and computed filmstrip geometry.
- Produces: a hidden platform scrollbar plus a non-interactive `.folder-filmstrip-scrollbar` overlay with deterministic thumb width/offset and CSS-only hover, `focus-within`, and reduced-motion states.

- [x] **Step 1: Add failing component and style contracts**

Add a `FolderFilmstripRow` regression that sizes an overflowing viewport, asserts a 36 px minimum visual thumb at the leading edge, scrolls halfway, and expects the thumb to move to the matching midpoint. Extend the style contract to require a hidden native scrollbar, transparent resting overlay, complete-row hover/focus visibility, a 180 ms fade, and no transition for reduced motion.

- [x] **Step 2: Run the focused tests and observe the intended failures**

Run:

```bash
pnpm --dir ui exec vitest run src/components/FolderFilmstripRow.test.tsx src/styles/app.test.ts
```

Observed: FAIL because no overlay markup or style rules existed.

- [x] **Step 3: Add the native-scroll visual overlay**

Wrap the native scroll viewport in `.folder-filmstrip-shell`. Hide its platform scrollbar while keeping `overflow-x: auto`. Derive an overlay thumb from `clientWidth`, `geometry.totalWidth`, and `scrollLeft`; enforce a 36 px minimum; clamp its progress to `[0, 1]`; and render it only for real overflow. Make the overlay non-interactive and reveal it through complete-row hover or `focus-within`.

- [x] **Step 4: Run the focused component/style tests and confirm green**

Run:

```bash
pnpm --dir ui exec vitest run src/components/FolderFilmstripRow.test.tsx src/styles/app.test.ts
```

Observed: 42/42 passed, including existing geometry, virtualization, focus, semantic-color, and compact-layout contracts.

- [x] **Step 5: Commit the independently testable scrollbar fix**

```bash
git add ui/src/components/FolderFilmstripRow.tsx ui/src/components/FolderFilmstripRow.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: stabilize filmstrip scrollbar visibility"
```

### Task 2: StrictMode-safe thumbnail progress

**Files:**
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`

**Interfaces:**
- Consumes: `ContentBrowser` prop `onThumbnailTaskChange?: (task: TaskFeedback | null) => void` and its existing `ThumbnailWork` counters.
- Produces: a mounted-lifetime effect that is true for every active setup and false only after the corresponding cleanup; no public API changes.

- [x] **Step 1: Import StrictMode and add a failing successful-settlement regression**

Change the React import in `ui/src/components/ContentBrowser.test.tsx` to:

```ts
import { type ComponentProps, StrictMode, useState } from 'react'
```

Add this test near the existing thumbnail-request tests:

```tsx
it('settles successful thumbnail progress after StrictMode effect preflight', async () => {
  const pending = deferred<string>()
  const onThumbnailTaskChange = vi.fn()

  render(
    <StrictMode>
      <ContentBrowser
        workspace={ratioWorkspace([{ width: 1, height: 1 }])}
        requestThumbnail={() => pending.promise}
        onThumbnailTaskChange={onThumbnailTaskChange}
      />
    </StrictMode>,
  )

  await waitFor(() =>
    expect(onThumbnailTaskChange).toHaveBeenCalledWith(
      expect.objectContaining({ status: 'running', completed: 0, failed: 0 }),
    ),
  )

  await act(async () => {
    pending.resolve('viewer-image://thumbnail/strict-success')
    await pending.promise
  })

  await waitFor(() => {
    const settled = onThumbnailTaskChange.mock.calls
      .map(([task]) => task)
      .find(
        (task) =>
          task?.requested > 0 &&
          task.completed === task.requested &&
          task.failed === 0 &&
          task.status === 'complete',
      )
    expect(settled).toBeDefined()
  })
})
```

- [x] **Step 2: Run the success regression and observe the intended failure**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx -t "settles successful thumbnail progress after StrictMode effect preflight"
```

Expected: FAIL at the terminal-task assertion. The current StrictMode preflight cleanup leaves `mounted.current === false`, so `completed` remains zero.

- [x] **Step 3: Add and observe the failing rejected-settlement regression**

Add the companion rejection test:

```tsx
it('settles failed thumbnail progress after StrictMode effect preflight', async () => {
  const pending = deferred<string>()
  const onThumbnailTaskChange = vi.fn()

  render(
    <StrictMode>
      <ContentBrowser
        workspace={ratioWorkspace([{ width: 1, height: 1 }])}
        requestThumbnail={() => pending.promise}
        onThumbnailTaskChange={onThumbnailTaskChange}
      />
    </StrictMode>,
  )

  await waitFor(() =>
    expect(onThumbnailTaskChange).toHaveBeenCalledWith(
      expect.objectContaining({ status: 'running', completed: 0, failed: 0 }),
    ),
  )

  await act(async () => {
    pending.reject(new Error('decode failed'))
    await Promise.resolve()
  })

  await waitFor(() => {
    const settled = onThumbnailTaskChange.mock.calls
      .map(([task]) => task)
      .find(
        (task) =>
          task?.requested > 0 &&
          task.completed === 0 &&
          task.failed === task.requested &&
          task.status === 'failed',
      )
    expect(settled).toBeDefined()
  })
})
```

Run:

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx -t "settles failed thumbnail progress after StrictMode effect preflight"
```

Expected: FAIL because `failed` remains zero after the promise rejection.

- [x] **Step 4: Repair the mounted-lifetime effect at its source**

Replace the current cleanup-only effect in `ContentBrowser.tsx`:

```ts
useEffect(
  () => () => {
    mounted.current = false
  },
  [],
)
```

with a setup-and-cleanup pair:

```ts
useEffect(() => {
  mounted.current = true
  return () => {
    mounted.current = false
  }
}, [])
```

Do not change request totals, cache identity, task-bar logic, or promise error propagation.

- [x] **Step 5: Run the focused ContentBrowser suite and confirm green**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx
```

Expected: PASS for both new StrictMode regressions and all existing content-grid, selection, virtualization, and thumbnail tests.

- [x] **Step 6: Commit the independently testable progress fix**

```bash
git add ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx
git commit -m "fix: settle thumbnail progress in StrictMode"
```

### Task 3: Integrated verification and native acceptance

**Files:**
- Modify: `docs/README.md`
- Modify: `docs/superpowers/plans/2026-08-05-viewer-filmstrip-scrollbar-thumbnail-progress.md`

**Interfaces:**
- Consumes: the completed CSS and lifecycle fixes from Tasks 1 and 2.
- Produces: repository-wide verification evidence and a restarted single-instance development app.

- [x] **Step 1: Run the combined focused regression set**

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts src/components/ContentBrowser.test.tsx src/components/TaskBar.test.tsx src/app/projectThumbnailCache.test.ts
```

Observed: the expanded six-file affected set passed 140/140, including the
filmstrip row/overview, styles, ContentBrowser, TaskBar, and project thumbnail
cache.

- [x] **Step 2: Check formatting and diff integrity**

```bash
pnpm --dir ui check
git diff --check
```

Expected: both commands exit zero. Biome may print its existing informational deprecation notice, but it must report no errors and apply no changes.

- [x] **Step 3: Run the repository's complete verification command once**

```bash
pnpm verify
```

Observed: exit zero. UI reported 79/79 files, 743 passed and one
intentional skip; production build, Rust formatting/Clippy/workspace tests,
Tauri security boundaries, dependency policy, and license policy all passed.

- [x] **Step 4: Restart the development app through the canonical single-instance launcher**

```bash
pnpm start:viewer
```

Observed: the launcher reported one `target/debug/viewer-desktop` process
sourced from `codex/fix-filmstrip-scrollbar-thumbnail-progress`; the same
process received the final hot update without opening a second Viewer window.

- [x] **Step 5: Perform native visual acceptance**

In the running Tauri window:

1. Open the representative test project and display folder overview rows with horizontal overflow.
2. Verify every scrollbar thumb is visually absent while its row is idle.
3. Move the pointer into one complete row and verify only that row's thumb fades in.
4. Move the pointer outside the row and verify the thumb fades out without row-height or scroll-position movement.
5. Tab focus into the row and verify its thumb remains visible until focus leaves.
6. Open uncached 90-image and 300-image projects and verify the native task
   reaches its terminal state without remaining at `0 / N`. The work completed
   faster than the controller's 500 ms screenshot sample, while the dedicated
   StrictMode success/failure regressions verify the exact counter advance and
   terminal counts deterministically.
7. Confirm switching folders still preserves progressive content display and does not introduce a full-workspace flash.

Observed: native 1024x720 captures have distinct hashes for row hover versus
rest and row focus versus focus leaving the row. Visual inspection confirmed a
single thin thumb for only the active row, complete disappearance at rest,
stable row geometry, and populated content throughout. Both uncached fixture
runs reached a task-free terminal UI; neither remained at `0 / N`.

- [x] **Step 6: Mark this implementation plan Historical in the documentation index and commit verification evidence**

Ensure `docs/README.md` contains this plan in `Historical implementation plans`, keep the design document Active, check every box in this plan after its evidence exists, then run:

```bash
node --test scripts/repository-policy.test.mjs
git add docs/README.md docs/superpowers/plans/2026-08-05-viewer-filmstrip-scrollbar-thumbnail-progress.md
git commit -m "docs: record scrollbar and thumbnail progress acceptance"
```

Expected: the policy test passes and the final documentation commit records only completed evidence.
