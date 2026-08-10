# Viewer Bottom Shelf Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin the mixed-folder `其它文件` shelf to the bottom of the content window and give the image browser all remaining height above it.

**Architecture:** Restore one bounded flex column in which the image slot grows and the other-file shelf remains intrinsic or capped. Remove the mixed-only content-height mode from `AspectVirtualGrid`; the stable grid node will consume the measured image-slot height while the shelf continues to control only its own rows.

**Tech Stack:** React 19, TypeScript 6, CSS flexbox, ResizeObserver-backed viewport measurement, Vitest, Testing Library

## Global Constraints

- The collapsed mixed shelf remains one intrinsic disclosure row of about 38 pixels.
- The expanded mixed shelf uses disclosure plus actual rows and remains capped at exactly 20% of the content body.
- Rows beyond the 20% cap scroll inside the existing virtual list.
- The image slot and `AspectVirtualGrid` must not be keyed or remounted during shelf changes.
- Preserve selection, keyboard ownership, scroll position, recovered dimensions and thumbnail work.
- Other-file-only folders keep their full-height primary-list behavior.
- Add no dependency and change no desktop API or persistence schema.

---

## File Structure

- Modify `ui/src/components/ContentBrowser.test.tsx` — assert that mixed content uses the measured remaining image-slot height.
- Modify `ui/src/components/contentBrowser/OtherFilePanel.test.tsx` — lock the one-file expanded shelf to one row.
- Modify `ui/src/styles/adaptiveOtherFilePanel.test.ts` — lock the flex contract and reject the mixed-only shrink override.
- Modify `ui/src/components/ContentBrowser.tsx` — stop asking the grid to fit only its laid-out rows.
- Modify `ui/src/components/AspectVirtualGrid.tsx` — remove the now-unused `fitContentHeight` API and use the measured viewport height directly.
- Modify `ui/src/styles/adaptiveOtherFilePanel.css` — let the mixed image slot retain its base `flex: 1 1 auto` behavior.

### Task 1: Restore The Bounded Bottom-Shelf Layout

**Files:**
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/contentBrowser/OtherFilePanel.test.tsx`
- Modify: `ui/src/styles/adaptiveOtherFilePanel.test.ts`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/AspectVirtualGrid.tsx`
- Modify: `ui/src/styles/adaptiveOtherFilePanel.css`

**Interfaces:**
- Consumes: `useMeasuredElementHeight(viewportHeight)` returning the live image-slot height.
- Produces: `AspectVirtualGrid` whose height is always its `viewportHeight` prop; mixed content uses the inherited `flex: 1 1 auto` image slot.

- [ ] **Step 1: Replace the mixed-content shrink regression with a remaining-height regression**

In `ui/src/components/ContentBrowser.test.tsx`, replace the test named
`sizes a mixed image slot to its laid-out rows and reserves the selection summary layer`
with:

```tsx
it('gives a mixed image grid the full measured slot above the bottom shelf', () => {
  render(<ControlledContentBrowser workspace={workspace(3)} density="compact" />)

  const slot = screen.getByTestId('content-image-slot')
  triggerResize(slot, 900, 640)
  flushAnimationFrames()

  expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveStyle({
    height: '640px',
  })
  expect(slot.nextElementSibling).toBe(
    screen.getByRole('button', { name: '其它文件 · 1' }).closest('.other-file-panel'),
  )

  const browser = screen.getByRole('region', { name: '文件内容' })
  expect(browser).not.toHaveAttribute('data-has-selection')
  fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
  expect(browser).toHaveAttribute('data-has-selection', 'true')
})
```

In `ui/src/styles/adaptiveOtherFilePanel.test.ts`, add this test after the
workspace-bounds contract:

```ts
it('lets the image slot fill mixed content above the intrinsic bottom shelf', () => {
  const rules = parseRules(adaptiveOtherFilePanelCss)

  expect(declarationsFor(rules, '.content-browser-image-slot')).toMatchObject({
    flex: '1 1 auto',
    'min-height': '0',
    overflow: 'hidden',
  })
  expect(
    declarationsFor(
      rules,
      '.content-browser[data-content-mode^="mixed_"] .content-browser-image-slot',
    ),
  ).toBeUndefined()
})
```

In `ui/src/components/contentBrowser/OtherFilePanel.test.tsx`, add this
one-file content-height contract after the measured mixed-shelf test:

```tsx
it('requests exactly one row of list height for one expanded other file', () => {
  renderPanel({ mode: 'mixed_expanded', files: [otherFiles[0] as BrowserFile] })

  expect(screen.getByRole('listbox', { name: '其它文件' })).toHaveStyle({
    height: `${OTHER_FILE_ROW_HEIGHT}px`,
  })
  expect(screen.getAllByRole('option')).toHaveLength(1)
})
```

- [ ] **Step 2: Run the focused tests and verify the old mixed layout fails**

Run:

```bash
pnpm --dir ui test -- \
  src/components/ContentBrowser.test.tsx \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/styles/adaptiveOtherFilePanel.test.ts
```

Expected: FAIL because the mixed grid still reports its laid-out row height
instead of `640px`, and the mixed-only CSS selector still exists.

- [ ] **Step 3: Remove the mixed-only content-height path**

In `ui/src/components/ContentBrowser.tsx`, delete:

```ts
const mixedContent = mode === 'mixed_collapsed' || mode === 'mixed_expanded'
```

and remove this prop from `AspectVirtualGrid`:

```tsx
fitContentHeight={mixedContent}
```

In `ui/src/components/AspectVirtualGrid.tsx`, remove the optional interface
property and destructured default:

```ts
fitContentHeight?: boolean
```

```ts
fitContentHeight = false,
```

Delete the `effectiveViewportHeight` branch:

```ts
const effectiveViewportHeight = fitContentHeight
  ? Math.min(viewportHeight, geometry.totalHeight)
  : viewportHeight
```

Use `viewportHeight` directly in the geometry effect dependency, visible-row
calculation and rendered style:

```ts
}, [geometry, scrollTop, viewportHeight])

const visibleRows = useMemo(
  () => verticalVisibleRows(geometry, scrollTop, viewportHeight, overscanRows),
  [geometry, overscanRows, scrollTop, viewportHeight],
)
```

```tsx
style={{ height: viewportHeight, overflow: 'auto', position: 'relative' }}
```

In `ui/src/styles/adaptiveOtherFilePanel.css`, delete only this override:

```css
.content-browser[data-content-mode^="mixed_"] .content-browser-image-slot {
  flex: 0 1 auto;
}
```

Keep these shelf rules unchanged:

```css
.other-file-panel--mixed-expanded {
  flex: 0 1 auto;
  max-height: 20%;
}

.other-file-panel--other_only {
  flex: 1 1 auto;
  max-height: none;
}
```

- [ ] **Step 4: Run focused layout verification**

Run:

```bash
pnpm --dir ui test -- \
  src/components/AspectVirtualGrid.test.tsx \
  src/components/ContentBrowser.test.tsx \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/styles/adaptiveOtherFilePanel.test.ts
pnpm --dir ui check
```

Expected: all tests PASS and Biome plus TypeScript complete without errors.

- [ ] **Step 5: Inspect the patch and commit the independent layout fix**

Run:

```bash
git diff --check
git diff -- \
  ui/src/components/AspectVirtualGrid.tsx \
  ui/src/components/ContentBrowser.tsx \
  ui/src/components/ContentBrowser.test.tsx \
  ui/src/components/contentBrowser/OtherFilePanel.test.tsx \
  ui/src/styles/adaptiveOtherFilePanel.css \
  ui/src/styles/adaptiveOtherFilePanel.test.ts
git status --short
```

Confirm that no grid `key` was added and the `20%` mixed-shelf cap remains.
Then commit:

```bash
git add \
  ui/src/components/AspectVirtualGrid.tsx \
  ui/src/components/ContentBrowser.tsx \
  ui/src/components/ContentBrowser.test.tsx \
  ui/src/components/contentBrowser/OtherFilePanel.test.tsx \
  ui/src/styles/adaptiveOtherFilePanel.css \
  ui/src/styles/adaptiveOtherFilePanel.test.ts
git commit -m "fix: pin other-file shelf below image grid"
```
