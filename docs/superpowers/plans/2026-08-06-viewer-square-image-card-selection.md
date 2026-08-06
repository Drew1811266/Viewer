# Viewer Square Image Card Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make content-folder image cards square and draw selection around the complete file card, including its filename, without changing selection or file interaction behavior.

**Architecture:** Keep `aria-selected` on `ImageCell` as the single selection source and move the non-measuring CSS overlay from the thumbnail frame to the complete card. Mirror the same geometry in the complete visual atlas so design reference and product cannot diverge. Preserve the existing outer keyboard-focus outline and place the organization handle above the non-interactive selection overlay.

**Tech Stack:** React 19, TypeScript, CSS, Vitest, Testing Library, JSDOM visual-atlas contracts, Tauri 2 WebView, Viewer native and visual acceptance controllers.

## Global Constraints

- Apply the change only to file items in the selected content folder's primary image grid.
- The complete file card and its selection boundary use exactly `0` corner radius.
- The resting card keeps its existing one-pixel neutral border and receives no shadow.
- The selected overlay is exactly `2px solid var(--viewer-accent)`, `inset: 0`, border-box sized, absolutely positioned, and `pointer-events: none`.
- The boundary encloses the thumbnail stage and filename area; no selected overlay or state attribute remains on the thumbnail frame.
- `aria-selected="true"` remains the accessibility and styling source of truth.
- Preserve card measurement, grid gap, aspect-ratio layout, filename height, virtualization, multi-selection, keyboard navigation, radial-menu invocation, preview, and drag behavior.
- Preserve the existing outer keyboard-focus outline as a state independent from selection.
- Keep the organization handle visually and interactively above the selection overlay.
- Map the complete-card boundary to `Highlight` in forced-colors mode.
- Do not change folder-overview filmstrips, preview, comparison, sidebar, menu, dialog, task, or empty-state cards.
- Add no dependency and do not adjust system display scaling, Dock settings, or global scrollbar settings during acceptance.
- Follow test-driven development: each production change follows an observed regression-test failure for the intended missing behavior.

---

### Task 1: Formal content-grid card and selection boundary

**Files:**
- Modify: `ui/src/components/ContentBrowser.test.tsx:674-692`
- Modify: `ui/src/styles/app.test.ts:94-155`
- Modify: `ui/src/components/contentBrowser/ImageCell.tsx:51-104`
- Modify: `ui/src/styles/app.css:1567-1601, 1770-1774, 2680-2684`

**Interfaces:**
- Consumes: `ImageCell` prop `selected: boolean`, the option element's existing `aria-selected`, `.image-cell`, `.image-cell-thumbnail-frame`, `.image-cell-meta`, and shared focus/forced-color tokens.
- Produces: `.image-cell[aria-selected="true"]::after` as the only visual selection overlay; no public component prop or callback changes.

- [ ] **Step 1: Replace the DOM regression with the complete-card contract**

Rename the test at `ui/src/components/ContentBrowser.test.tsx:674` to `uses the complete image card as the selection owner and explains the radial action`. After clicking the option, assert the filename metadata is inside that option, the option owns `aria-selected`, and the thumbnail frame has no selected-state attribute:

```tsx
it('uses the complete image card as the selection owner and explains the radial action', () => {
  render(<ContentBrowser workspace={workspace(1)} />)
  const option = screen.getByRole('option', { name: '1.jpg' })

  fireEvent.click(option)

  expect(option).toHaveAttribute('aria-selected', 'true')
  const metadata = option.querySelector('.image-cell-meta')
  expect(metadata).toContainElement(screen.getByText('1.jpg'))
  expect(option.querySelector('.image-cell-thumbnail-frame')).not.toHaveAttribute('data-selected')
  expect(option).not.toHaveAttribute('data-selected')
  expect(screen.getByRole('status', { name: '选择摘要' })).toHaveTextContent(
    '右键打开圆盘菜单 · Esc 取消选择',
  )

  fireEvent.keyDown(screen.getByRole('listbox', { name: '图片文件' }), { key: 'Escape' })

  expect(option).toHaveAttribute('aria-selected', 'false')
  expect(screen.queryByRole('status', { name: '选择摘要' })).not.toBeInTheDocument()
})
```

- [ ] **Step 2: Replace the style contract with square complete-card assertions**

Replace the current thumbnail-only selection test in `ui/src/styles/app.test.ts` with:

```ts
it('paints selection around the square complete card and keyboard focus outside it', () => {
  const rules = parseRules(appCss)
  const card = rules.find((rule) => rule.selector === '.image-cell')
  const selectionOverlay = rules.find(
    (rule) => rule.selector === '.image-cell[aria-selected="true"]::after',
  )
  const selectedCard = rules.find(
    (rule) => rule.selector === '.image-cell[aria-selected="true"]',
  )
  const legacyOverlay = rules.find(
    (rule) => rule.selector === ".image-cell-thumbnail-frame[data-selected='true']::after",
  )
  const organizationHandle = rules.find(
    (rule) => rule.selector === '.image-cell > .organization-drag-handle',
  )
  const activeFocus = rules.find(
    (rule) =>
      rule.selector === '.aspect-virtual-grid:focus-visible .image-cell[data-active="true"]',
  )
  const forcedSelection = parseRules(mediaBody(appCss, '(forced-colors: active)')).find(
    (rule) => rule.selector === '.image-cell[aria-selected="true"]::after',
  )

  expect(card?.declarations['border-radius']).toBe('0')
  expect(selectionOverlay?.declarations).toMatchObject({
    border: '2px solid var(--viewer-accent)',
    'border-radius': '0',
    'box-sizing': 'border-box',
    inset: '0',
    'pointer-events': 'none',
    position: 'absolute',
    'z-index': '2',
  })
  expect(legacyOverlay).toBeUndefined()
  expect(selectedCard?.declarations.background).toBeUndefined()
  expect(selectedCard?.declarations['box-shadow']).toBeUndefined()
  expect(organizationHandle?.declarations['z-index']).toBe('3')
  expect(activeFocus?.declarations).toMatchObject({
    outline: 'var(--viewer-focus-outline)',
    'outline-offset': 'var(--viewer-focus-offset)',
  })
  expect(forcedSelection?.declarations).toMatchObject({
    'border-color': 'Highlight',
    'border-width': '2px',
  })
})
```

In `uses the approved quiet card and compact file metadata hierarchy`, change the expected card radius from `9px` to `0` and leave every other geometry and metadata assertion unchanged.

- [ ] **Step 3: Run the focused contracts and observe the intended failures**

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts src/components/ContentBrowser.test.tsx
```

Expected: FAIL because the card still reports `9px`, the complete-card overlay and handle stacking rule do not exist, forced colors still target the thumbnail frame, and the rendered thumbnail frame still has `data-selected="true"`.

- [ ] **Step 4: Remove selected state from the thumbnail frame**

In `ImageCell.tsx`, replace:

```tsx
<div className="image-cell-thumbnail-frame" data-selected={selected || undefined}>
```

with:

```tsx
<div className="image-cell-thumbnail-frame">
```

Do not move the filename, marker, export surface, drag handle, or event handlers. Keep `aria-selected={selected}` on the outer `.image-cell`.

- [ ] **Step 5: Move the visual overlay to the complete square card**

In `app.css`, change `.image-cell` to `border-radius: 0`. Replace the legacy thumbnail-frame selected rule with:

```css
.image-cell[aria-selected="true"]::after {
  border: 2px solid var(--viewer-accent);
  border-radius: 0;
  box-sizing: border-box;
  content: "";
  inset: 0;
  pointer-events: none;
  position: absolute;
  z-index: 2;
}
```

Add `z-index: 3` to `.image-cell > .organization-drag-handle`. In the existing `@media (forced-colors: active)` block, replace the thumbnail-frame selector with `.image-cell[aria-selected="true"]::after`. Do not add a transition, selected fill, shadow, or physical border-width change.

- [ ] **Step 6: Run the focused contracts and confirm green**

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts src/components/ContentBrowser.test.tsx
```

Expected: both files pass, including the updated complete-card selection contract and all existing selection-summary, preview, radial-menu, thumbnail, and virtual-grid behavior in `ContentBrowser`.

- [ ] **Step 7: Commit the independently testable formal component change**

```bash
git add ui/src/components/ContentBrowser.test.tsx ui/src/styles/app.test.ts ui/src/components/contentBrowser/ImageCell.tsx ui/src/styles/app.css
git commit -m "fix: select complete square image cards"
```

### Task 2: Keep the complete visual atlas synchronized

**Files:**
- Modify: `ui/src/visualAtlas.test.ts:97-112`
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html:525-551, 1424`

**Interfaces:**
- Consumes: the atlas `.image-card`, `.image-stage`, `.image-meta`, selected class, `aria-selected`, and forced-colors accessibility scene.
- Produces: an atlas reference whose square whole-card selection geometry matches the formal `.image-cell` contract.

- [ ] **Step 1: Add the failing visual-atlas geometry contract**

Add this test after the proportional-density test in `ui/src/visualAtlas.test.ts`:

```ts
it('models square complete-card selection instead of an inset thumbnail ring', () => {
  expect(html).toMatch(/\.image-card\s*\{[^}]*border-radius:\s*0;/s)
  expect(html).toMatch(
    /\.image-card\.selected::after\s*\{[^}]*border:\s*2px solid var\(--accent\);[^}]*border-radius:\s*0;[^}]*box-sizing:\s*border-box;[^}]*inset:\s*0;[^}]*pointer-events:\s*none;[^}]*position:\s*absolute;/s,
  )
  expect(html).not.toContain('.image-card.selected .image-stage::after')
  expect(html).toMatch(
    /\.accessible-viewer\[data-accessible-viewer-state="forced"\]\s+\.image-card\.selected::after\s*\{[^}]*border-color:\s*Highlight;/s,
  )
})
```

- [ ] **Step 2: Run the atlas test and observe the intended failure**

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts -t "models square complete-card selection"
```

Expected: FAIL because the atlas still uses a `9px` card radius and `.image-card.selected .image-stage::after` with an inset rounded boundary.

- [ ] **Step 3: Update the atlas card and forced-colors reference**

In `viewer-complete-ui-visual-atlas.html`, change `.image-card` to `border-radius: 0`. Replace the legacy selected rule with:

```css
.image-card.selected::after {
  border: 2px solid var(--accent);
  border-radius: 0;
  box-sizing: border-box;
  content: "";
  inset: 0;
  pointer-events: none;
  position: absolute;
  z-index: 2;
}
```

Change the forced-colors selector from `.image-card.selected .image-stage::after` to `.image-card.selected::after`. Do not change atlas card size, density variants, thumbnail stage, metadata, or selection interactions.

- [ ] **Step 4: Run the complete atlas suite and confirm green**

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS for the new geometry contract and all existing screen/state, interaction, contrast, and local-asset contracts.

- [ ] **Step 5: Commit the independently testable atlas synchronization**

```bash
git add ui/src/visualAtlas.test.ts docs/prototypes/viewer-complete-ui-visual-atlas.html
git commit -m "docs: sync square image card selection atlas"
```

### Task 3: Focused verification, same-state visual acceptance, and handoff evidence

**Files:**
- Modify: `design-qa.md`
- Modify: `docs/README.md`
- Modify: `docs/superpowers/plans/2026-08-06-viewer-square-image-card-selection.md`

**Interfaces:**
- Consumes: Tasks 1 and 2, acceptance states `THU-04` through `THU-07`, the approved annotated target, and the canonical `pnpm start:viewer` launcher.
- Produces: one complete verification result, combined reference/product screenshots, a native single-selection capture, a `design-qa.md` verdict, and a restarted single-instance development app.

- [ ] **Step 1: Run the affected regression set once**

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts src/components/ContentBrowser.test.tsx src/components/contentBrowser/contentSelection.test.ts src/components/AspectVirtualGrid.test.tsx src/visualAtlas.test.ts
```

Expected: PASS. This covers the formal CSS/DOM contract, single and multiple selection, independent keyboard focus, virtual-grid navigation, radial-action copy, and atlas synchronization.

- [ ] **Step 2: Check source quality and diff integrity**

```bash
pnpm --dir ui check
git diff --check
```

Expected: both commands exit zero and apply no formatting changes.

- [ ] **Step 3: Run the repository-wide verification command exactly once**

```bash
pnpm verify
```

Expected: exit zero for repository policy, clean checks, Biome, TypeScript, all UI tests, production UI build, Rust formatting, Clippy, workspace tests, Tauri security boundaries, dependency policy, and license policy. Do not repeat this command unless it reports a real failure and code changes are made to correct that failure.

- [ ] **Step 4: Generate targeted two-viewport reference/product evidence**

Run only the four affected visual states; the command defaults to both approved viewports (`1024x720` and `1440x900`):

```bash
pnpm accept:visual --id THU-04 --id THU-05 --id THU-06 --id THU-07 --output-root target/viewer-visual-acceptance/square-image-card-selection
```

Expected: eight combined artifacts are generated under `target/viewer-visual-acceptance/`; no unrelated acceptance state is opened. Inspect the combined `THU-04`, `THU-05`, `THU-06`, and `THU-07` images together with the approved annotated target. Pass only when resting cards are square, every selected boundary encloses the filename, no thumbnail-only ring remains, multi-selection has one boundary per card, and keyboard focus remains distinct.

- [ ] **Step 5: Start one current development app and capture the native selected state**

Start through the canonical launcher:

```bash
pnpm start:viewer
```

After the launcher reports one current `target/debug/viewer-desktop` process, run one native single-selection recipe at the larger viewport:

```bash
node scripts/viewer-native-acceptance.mjs --id THU-05 --viewport 1440x900 --output-root /Users/abc/Project/Viewer/target/atlas-product-migration-acceptance/square-image-card-selection
```

Expected: the controller uses its disposable `thumbnail-selection` fixture, selects `商品-01.jpg`, records a clean-worktree manifest and combined image, and does not mutate the user's project. It must not change macOS display scaling, Dock settings, or global scrollbar settings.

- [ ] **Step 6: Record the final design QA verdict**

Append this section to the existing `design-qa.md`; preserve all earlier acceptance history and replace only the evidence-path observations with the files actually produced:

```markdown
## 2026-08-06 — Square Image Card Selection

## Inputs

- Approved design: `docs/superpowers/specs/2026-08-06-viewer-square-image-card-selection-design.md`
- User target: annotated complete-card red boundary supplied on 2026-08-06
- Automated combined evidence root: `target/viewer-visual-acceptance/square-image-card-selection/`; the command result records exact commit, viewport, and `THU-04` through `THU-07` artifact paths.
- Native evidence root: `target/atlas-product-migration-acceptance/`; the command result records the exact commit path for `1440x900/THU-05/combined.png`.

## Checklist

- [x] Resting content-grid cards have square corners.
- [x] The selection boundary encloses thumbnail and filename.
- [x] No inset thumbnail-only boundary remains.
- [x] Selection does not move card content or grid geometry.
- [x] Multi-selection draws one boundary per card.
- [x] Keyboard focus remains visually independent.
- [x] The organization handle remains above and operable.
- [x] Forced-colors styling follows the complete card.
- [x] Folder filmstrips and other card families are unchanged.

## Verdict

PASS — the formal component and visual atlas match the approved complete-card square selection design at both approved visual-acceptance viewports and in the native Tauri state.
```

If any checklist item fails, write `FAIL`, record the mismatch, fix only that mismatch, rerun its focused test and affected acceptance state, and update the evidence before proceeding.

- [ ] **Step 7: Mark the plan Historical and commit acceptance evidence metadata**

After all evidence exists, check every box in this plan and add this plan to `Historical implementation plans` in `docs/README.md`. Keep the design spec in `Active sources of truth`. Run:

```bash
node --test scripts/repository-policy.test.mjs
git diff --check
git add design-qa.md docs/README.md docs/superpowers/plans/2026-08-06-viewer-square-image-card-selection.md
git commit -m "docs: record square image card selection acceptance"
```

Expected: policy and diff checks pass, and the final documentation commit contains only the QA verdict, index update, and completed plan evidence.

- [ ] **Step 8: Restart the final integrated development build**

After integrating the implementation branch into `main`, run:

```bash
pnpm start:viewer
```

Expected: exactly one Viewer development process runs from `/Users/abc/Project/Viewer/target/debug/viewer-desktop`, showing the integrated latest code without opening a second app window.
