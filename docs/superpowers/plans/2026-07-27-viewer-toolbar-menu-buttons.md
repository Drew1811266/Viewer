# Viewer Toolbar Menu Buttons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `筛选与排序`, `结果视图`, and `•••` workspace-header menu triggers visibly stateful buttons without changing their existing behavior.

**Architecture:** Keep the current React `<details>/<summary>` structure and add a narrowly scoped CSS presentation contract. The parent `<details open>` attribute drives the expanded appearance, while pseudo-classes provide hover and keyboard-focus feedback.

**Tech Stack:** React 19, TypeScript 6, CSS, Vitest 4, pnpm

## Global Constraints

- Preserve the current menu behavior, accessible names, and keyboard handling.
- Change only the direct summaries of `.search-options-panel`, `.search-view-panel`, and `.project-menu`.
- Do not replace `<details>/<summary>` or change menu contents, positioning, data flow, persistence, or backend commands.
- Support the existing light and dark color schemes.
- Add no dependency.

---

### Task 1: Add the stateful toolbar-menu button treatment

**Files:**
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/app.css:145-160`
- Modify: `ui/src/styles/app.css:1799-1891`

**Interfaces:**
- Consumes: the existing `.search-options-panel`, `.search-view-panel`, and `.project-menu` class names and the native `<details open>` attribute.
- Produces: default, hover, expanded, and `:focus-visible` visual states for those three summaries, plus compact square sizing for `.project-menu > summary`.

- [ ] **Step 1: Write the failing CSS contract test**

Add this test inside `describe('workspace style contracts', ...)` in
`ui/src/styles/app.test.ts`:

```ts
it('renders workspace menu summaries as stateful toolbar buttons', () => {
  const rules = parseRules(appCss)
  const baseSelectors = [
    '.search-options-panel > summary',
    '.search-view-panel > summary',
    '.project-menu > summary',
  ]

  for (const selector of baseSelectors) {
    expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe('#fff')
    expect(winningDeclaration(rules, new Set([selector]), 'border'), selector).toBe(
      '1px solid #c8ced6',
    )
    expect(winningDeclaration(rules, new Set([selector]), 'border-radius'), selector).toBe('6px')
    expect(winningDeclaration(rules, new Set([selector]), 'cursor'), selector).toBe('pointer')
    expect(winningDeclaration(rules, new Set([selector]), 'min-height'), selector).toBe('30px')
  }

  const hoverSelectors = [
    '.search-options-panel > summary:hover',
    '.search-view-panel > summary:hover',
    '.project-menu > summary:hover',
  ]
  for (const selector of hoverSelectors) {
    expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe('#f3f5f7')
    expect(winningDeclaration(rules, new Set([selector]), 'border-color'), selector).toBe(
      '#aeb6c1',
    )
  }

  const openSelectors = [
    '.search-options-panel[open] > summary',
    '.search-view-panel[open] > summary',
    '.project-menu[open] > summary',
  ]
  for (const selector of openSelectors) {
    expect(winningDeclaration(rules, new Set([selector]), 'background'), selector).toBe('#d9e8ff')
    expect(winningDeclaration(rules, new Set([selector]), 'border-color'), selector).toBe(
      '#2477d4',
    )
  }

  const focusSelectors = [
    '.search-options-panel > summary:focus-visible',
    '.search-view-panel > summary:focus-visible',
    '.project-menu > summary:focus-visible',
  ]
  for (const selector of focusSelectors) {
    expect(winningDeclaration(rules, new Set([selector]), 'outline'), selector).toBe(
      '2px solid #2477d4',
    )
    expect(winningDeclaration(rules, new Set([selector]), 'outline-offset'), selector).toBe('2px')
  }

  expect(
    winningDeclaration(rules, new Set(['.project-menu > summary']), 'min-width'),
  ).toBe('30px')

  const darkRules = [
    ...rules,
    ...parseRules(mediaBody(appCss, '(prefers-color-scheme: dark)')),
  ]
  for (const selector of baseSelectors) {
    expect(winningDeclaration(darkRules, new Set([selector]), 'background'), selector).toBe(
      '#24282f',
    )
    expect(winningDeclaration(darkRules, new Set([selector]), 'border-color'), selector).toBe(
      '#4d5663',
    )
    expect(winningDeclaration(darkRules, new Set([selector]), 'color'), selector).toBe('#f3f5f7')
  }
  for (const selector of openSelectors) {
    expect(winningDeclaration(darkRules, new Set([selector]), 'background'), selector).toBe(
      '#244d7d',
    )
    expect(winningDeclaration(darkRules, new Set([selector]), 'border-color'), selector).toBe(
      '#5d9ee8',
    )
  }
  for (const selector of focusSelectors) {
    expect(winningDeclaration(darkRules, new Set([selector]), 'outline-color'), selector).toBe(
      '#8ec8ff',
    )
  }
})
```

- [ ] **Step 2: Run the focused test and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "renders workspace menu summaries"
```

Expected: FAIL because the summaries currently resolve no white background or
neutral border and still resolve `cursor: default`.

- [ ] **Step 3: Add the minimal light-scheme button CSS**

Insert after the existing details-marker rule in `ui/src/styles/app.css`:

```css
.search-options-panel > summary,
.search-view-panel > summary,
.project-menu > summary {
  align-items: center;
  background: #fff;
  border: 1px solid #c8ced6;
  border-radius: 6px;
  box-sizing: border-box;
  cursor: pointer;
  display: flex;
  justify-content: center;
  min-height: 30px;
  padding: 4px 8px;
}

.project-menu > summary {
  min-width: 30px;
}

.search-options-panel > summary:hover,
.search-view-panel > summary:hover,
.project-menu > summary:hover {
  background: #f3f5f7;
  border-color: #aeb6c1;
}

.search-options-panel[open] > summary,
.search-view-panel[open] > summary,
.project-menu[open] > summary {
  background: #d9e8ff;
  border-color: #2477d4;
}

.search-options-panel > summary:focus-visible,
.search-view-panel > summary:focus-visible,
.project-menu > summary:focus-visible {
  outline: 2px solid #2477d4;
  outline-offset: 2px;
}
```

- [ ] **Step 4: Add the dark-scheme state overrides**

Inside the existing `@media (prefers-color-scheme: dark)` block in
`ui/src/styles/app.css`, after the workspace-header/popover color rule, add:

```css
  .search-options-panel > summary,
  .search-view-panel > summary,
  .project-menu > summary {
    background: #24282f;
    border-color: #4d5663;
    color: #f3f5f7;
  }

  .search-options-panel > summary:hover,
  .search-view-panel > summary:hover,
  .project-menu > summary:hover {
    background: #2f343d;
    border-color: #6c7888;
  }

  .search-options-panel[open] > summary,
  .search-view-panel[open] > summary,
  .project-menu[open] > summary {
    background: #244d7d;
    border-color: #5d9ee8;
  }

  .search-options-panel > summary:focus-visible,
  .search-view-panel > summary:focus-visible,
  .project-menu > summary:focus-visible {
    outline-color: #8ec8ff;
  }
```

- [ ] **Step 5: Run the focused style and interaction tests**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts src/components/SearchToolbar.test.tsx src/App.test.tsx
```

Expected: PASS. The CSS contract covers all visual states, and the component
tests confirm that click and keyboard activation still toggle the menus.

- [ ] **Step 6: Run formatting, type, full UI, and production-build verification**

Run:

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
```

Expected: all commands exit with code 0.

- [ ] **Step 7: Inspect the running development build**

With the existing Tauri development process open, verify:

- all three triggers have a visible button boundary at rest;
- hover and expanded feedback match the approved hierarchy;
- keyboard Tab focus displays the blue ring;
- `•••` aligns with the adjacent settings button;
- the same controls remain legible in dark appearance.

- [ ] **Step 8: Commit the implementation**

Stage only the two implementation files so unrelated working-tree changes stay
untouched:

```bash
git add ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: style toolbar menu triggers as buttons"
```
