# Viewer Filmstrip Scrollbar and Thumbnail Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make each folder-filmstrip scrollbar appear only for row hover or keyboard focus, and make visible-content thumbnail progress settle correctly under React `StrictMode`.

**Architecture:** Keep the filmstrip's native `overflow-x: auto` viewport and express its resting, hover, focus, and reduced-motion states entirely in component-scoped CSS. Keep thumbnail accounting inside `ContentBrowser`, but correct its mounted-lifetime effect so StrictMode setup cleanup cannot permanently disable promise-settlement updates.

**Tech Stack:** React 19, TypeScript, CSS, Vitest, Testing Library, Tauri 2 WebView.

## Global Constraints

- Preserve native filmstrip scrolling, trackpad inertia, dragging, scroll anchoring, and virtualization.
- The scrollbar thumb is transparent at rest, visible for complete-row hover or `focus-within`, and fades over exactly 180 ms.
- Reduced-motion users receive the same states without a transition.
- Do not replace the native scrollbar or change filmstrip geometry.
- Count visible-content thumbnail requests only; do not change project cache or backend protocols.
- Promise settlements after a real unmount must not update React state.
- Add no dependency and do not change task-bar dismissal or aggregation.
- Follow test-driven development: observe each regression test fail for the intended missing behavior before editing production code.

---

### Task 1: Folder filmstrip scrollbar visibility

**Files:**
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: existing `.folder-filmstrip-row` and `.folder-filmstrip` markup from `FolderFilmstripRow.tsx`.
- Produces: CSS-only resting, hover, `focus-within`, WebKit, standards-fallback, and reduced-motion scrollbar states.

- [ ] **Step 1: Add the failing scrollbar style contract**

Extend the existing `keeps folder identity fixed beside an independently scrolling filmstrip` test in `ui/src/styles/app.test.ts` with explicit rules for the native scrollbar:

```ts
const thumb = rules.find(
  (rule) => rule.selector === '.folder-filmstrip::-webkit-scrollbar-thumb',
)
const visibleThumb = rules.find(
  (rule) =>
    rule.selector ===
    '.folder-filmstrip-row:hover .folder-filmstrip::-webkit-scrollbar-thumb, .folder-filmstrip-row:focus-within .folder-filmstrip::-webkit-scrollbar-thumb',
)
const visibleFallback = rules.find(
  (rule) =>
    rule.selector ===
    '.folder-filmstrip-row:hover .folder-filmstrip, .folder-filmstrip-row:focus-within .folder-filmstrip',
)
const reducedThumb = parseRules(mediaBody(appCss, '(prefers-reduced-motion: reduce)')).find(
  (rule) => rule.selector === '.folder-filmstrip::-webkit-scrollbar-thumb',
)

expect(filmstrip?.declarations['scrollbar-color']).toBe('transparent transparent')
expect(thumb?.declarations).toMatchObject({
  'background-color': 'transparent',
  transition: 'background-color 180ms ease',
})
expect(visibleThumb?.declarations['background-color']).toBe('var(--viewer-text-tertiary)')
expect(visibleFallback?.declarations['scrollbar-color']).toBe(
  'var(--viewer-text-tertiary) transparent',
)
expect(reducedThumb?.declarations.transition).toBe('none')
```

- [ ] **Step 2: Run the style test and observe the intended failure**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts
```

Expected: FAIL because `.folder-filmstrip` has no resting `scrollbar-color`, no WebKit thumb rule, no hover/focus visibility rule, and no reduced-motion transition override.

- [ ] **Step 3: Add the minimal native-scrollbar CSS**

Add the following beside the existing `.folder-filmstrip` rule in `ui/src/styles/app.css`:

```css
.folder-filmstrip {
  scrollbar-color: transparent transparent;
}

.folder-filmstrip::-webkit-scrollbar {
  height: 8px;
}

.folder-filmstrip::-webkit-scrollbar-track {
  background: transparent;
}

.folder-filmstrip::-webkit-scrollbar-thumb {
  background-clip: padding-box;
  background-color: transparent;
  border: 2px solid transparent;
  border-radius: 999px;
  transition: background-color 180ms ease;
}

.folder-filmstrip-row:hover .folder-filmstrip,
.folder-filmstrip-row:focus-within .folder-filmstrip {
  scrollbar-color: var(--viewer-text-tertiary) transparent;
}

.folder-filmstrip-row:hover .folder-filmstrip::-webkit-scrollbar-thumb,
.folder-filmstrip-row:focus-within .folder-filmstrip::-webkit-scrollbar-thumb {
  background-color: var(--viewer-text-tertiary);
}
```

Inside the existing `@media (prefers-reduced-motion: reduce)` block add:

```css
.folder-filmstrip::-webkit-scrollbar-thumb {
  transition: none;
}
```

- [ ] **Step 4: Run the focused style test and confirm green**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts
```

Expected: PASS, including the existing filmstrip geometry, focus, semantic-color, and compact-layout contracts.

- [ ] **Step 5: Commit the independently testable scrollbar fix**

```bash
git add ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: fade folder filmstrip scrollbars"
```

### Task 2: StrictMode-safe thumbnail progress

**Files:**
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`

**Interfaces:**
- Consumes: `ContentBrowser` prop `onThumbnailTaskChange?: (task: TaskFeedback | null) => void` and its existing `ThumbnailWork` counters.
- Produces: a mounted-lifetime effect that is true for every active setup and false only after the corresponding cleanup; no public API changes.

- [ ] **Step 1: Import StrictMode and add a failing successful-settlement regression**

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

- [ ] **Step 2: Run the success regression and observe the intended failure**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx -t "settles successful thumbnail progress after StrictMode effect preflight"
```

Expected: FAIL at the terminal-task assertion. The current StrictMode preflight cleanup leaves `mounted.current === false`, so `completed` remains zero.

- [ ] **Step 3: Add and observe the failing rejected-settlement regression**

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

- [ ] **Step 4: Repair the mounted-lifetime effect at its source**

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

- [ ] **Step 5: Run the focused ContentBrowser suite and confirm green**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx
```

Expected: PASS for both new StrictMode regressions and all existing content-grid, selection, virtualization, and thumbnail tests.

- [ ] **Step 6: Commit the independently testable progress fix**

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

- [ ] **Step 1: Run the combined focused regression set**

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts src/components/ContentBrowser.test.tsx src/components/TaskBar.test.tsx src/app/projectThumbnailCache.test.ts
```

Expected: all selected files pass with zero failures.

- [ ] **Step 2: Check formatting and diff integrity**

```bash
pnpm --dir ui check
git diff --check
```

Expected: both commands exit zero. Biome may print its existing informational deprecation notice, but it must report no errors and apply no changes.

- [ ] **Step 3: Run the repository's complete verification command once**

```bash
pnpm verify
```

Expected: exit zero across policy checks, UI type checking, UI tests, production build, Rust formatting, Clippy, Rust workspace tests, Tauri security boundaries, dependency policy, and license policy.

- [ ] **Step 4: Restart the development app through the canonical single-instance launcher**

```bash
pnpm start:viewer
```

Expected: the launcher reports one `target/debug/viewer-desktop` process sourced from the current `main` commit and does not open a second Viewer window.

- [ ] **Step 5: Perform native visual acceptance**

In the running Tauri window:

1. Open the representative test project and display folder overview rows with horizontal overflow.
2. Verify every scrollbar thumb is visually absent while its row is idle.
3. Move the pointer into one complete row and verify only that row's thumb fades in.
4. Move the pointer outside the row and verify the thumb fades out without row-height or scroll-position movement.
5. Tab focus into the row and verify its thumb remains visible until focus leaves.
6. Open an image folder with uncached thumbnails and verify `正在生成缩略图` advances above zero and reaches a terminal count.
7. Confirm switching folders still preserves progressive content display and does not introduce a full-workspace flash.

- [ ] **Step 6: Mark this implementation plan Historical in the documentation index and commit verification evidence**

Ensure `docs/README.md` contains this plan in `Historical implementation plans`, keep the design document Active, check every box in this plan after its evidence exists, then run:

```bash
node --test scripts/repository-policy.test.mjs
git add docs/README.md docs/superpowers/plans/2026-08-05-viewer-filmstrip-scrollbar-thumbnail-progress.md
git commit -m "docs: record scrollbar and thumbnail progress acceptance"
```

Expected: the policy test passes and the final documentation commit records only completed evidence.

