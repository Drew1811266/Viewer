# Viewer Unified Light Preview Theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give single-image, text, and two-to-four-image previews one shared variable-driven light theme without changing any preview behavior or layout.

**Architecture:** Define semantic preview CSS custom properties in `:root`, migrate the existing single-image and text preview rules to those properties, then replace the comparison workspace’s independent dark palette with the same variables. Keep every React component, state transition, image request, and interaction unchanged; protect the final cascade with CSS contract tests and the existing component suites.

**Tech Stack:** React 19, TypeScript 6, CSS custom properties, Vitest 4, Testing Library, Biome, Vite 8, pnpm, Tauri 2

## Global Constraints

- Execute in an isolated git worktree because the main workspace contains unrelated uncommitted folder-tree, virtual-list, image-selection, style-test, and fixture changes.
- Do not modify `ImagePreview.tsx`, `TextPreview.tsx`, `CompareWorkspace.tsx`, `ComparePane.tsx`, compare state, reducers, image loading, IPC, persistence, or component markup.
- Do not add theme switching or restyle non-preview surfaces.
- Preserve two-, three-, and four-image layouts and every zoom, pan, rotation, synchronization, marker, favorite, removal, keyboard, and completion behavior.
- Preserve the comparison workspace's existing geometry: no outer border, 10-pixel workspace radius, 28-pixel/5-pixel ordinary comparison controls, and 24-pixel compact pane controls.
- Preserve active review and favorite button states when applying comparison button base styles.
- Stage and commit only `ui/src/styles/app.css` and `ui/src/styles/app.test.ts`.
- When integration returns to the dirty main workspace, preserve every pre-existing uncommitted change and untracked fixture.

---

### Task 1: Establish the shared preview theme and migrate existing previews

**Files:**

- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/app.css:1-7`
- Modify: `ui/src/styles/app.css:1530-1630`
- Test: `ui/src/styles/app.test.ts`
- Test: `ui/src/components/ImagePreview.test.tsx`
- Test: `ui/src/components/TextPreview.test.tsx`

**Interfaces:**

- Consumes: existing `parseRules(css)`, `winningDeclaration(rules, selectors, property)`, and `contrastRatio(foreground, background)` helpers in `app.test.ts`.
- Produces: the root custom properties `--preview-surface`, `--preview-chrome`, `--preview-stage`, `--preview-document-surface`, `--preview-panel-surface`, `--preview-text`, `--preview-muted`, `--preview-border`, `--preview-control-border`, `--preview-control-hover-border`, `--preview-control-surface`, `--preview-control-hover-surface`, `--preview-accent`, `--preview-accent-surface`, `--preview-accent-text`, `--preview-danger`, `--preview-danger-surface`, and `--preview-image-shadow`.
- Produces: variable-driven single-image and text preview declarations that Task 2 reuses.

- [ ] **Step 1: Replace the existing light-preview contract with a failing variable-driven contract**

Rename the existing test `renders image and text previews as shared light surfaces` to `uses one light theme for image and text previews`, and replace its body with:

```ts
it('uses one light theme for image and text previews', () => {
  const rules = parseRules(appCss)
  const root = rules.find((rule) => rule.selector === ':root')
  const declaration = (selector: string, property: string) =>
    winningDeclaration(rules, new Set([selector]), property)

  expect(root?.declarations).toMatchObject({
    '--preview-surface': '#f5f6f8',
    '--preview-chrome': '#fbfcfd',
    '--preview-stage': '#edf0f3',
    '--preview-document-surface': '#f7f7f8',
    '--preview-panel-surface': '#fff',
    '--preview-text': '#1f2328',
    '--preview-muted': '#68717d',
    '--preview-border': '#d8dce2',
    '--preview-control-border': '#8a94a3',
    '--preview-control-hover-border': '#747f8e',
    '--preview-control-surface': '#fff',
    '--preview-control-hover-surface': '#f3f5f7',
    '--preview-accent': '#2477d4',
    '--preview-accent-surface': '#d9e8ff',
    '--preview-accent-text': '#174f8f',
    '--preview-danger': '#9d1c13',
    '--preview-danger-surface': '#fff3f0',
    '--preview-image-shadow': '0 8px 28px rgb(34 42 53 / 14%)',
  })

  expect(declaration('.preview-overlay', 'background')).toBe('var(--preview-surface)')
  expect(declaration('.preview-overlay', 'color')).toBe('var(--preview-text)')
  expect(declaration('.preview-toolbar', 'background')).toBe('var(--preview-chrome)')
  expect(declaration('.preview-toolbar', 'border-bottom')).toBe(
    '1px solid var(--preview-border)',
  )
  expect(declaration('.preview-toolbar', 'color')).toBe('var(--preview-text)')
  expect(declaration('.image-preview-stage', 'background')).toBe('var(--preview-stage)')
  expect(declaration('.image-preview-stage img', 'border')).toBe(
    '1px solid var(--preview-border)',
  )
  expect(declaration('.image-preview-stage img', 'box-shadow')).toBe(
    'var(--preview-image-shadow)',
  )
  expect(declaration('.image-preview > footer', 'background')).toBe('var(--preview-chrome)')
  expect(declaration('.image-preview > footer', 'border-top')).toBe(
    '1px solid var(--preview-border)',
  )
  expect(declaration('.image-preview > footer', 'color')).toBe('var(--preview-text)')
  expect(declaration('.preview-overlay [role="alert"]', 'color')).toBe(
    'var(--preview-danger)',
  )
  expect(declaration('.text-preview', 'background')).toBe(
    'var(--preview-document-surface)',
  )
  expect(declaration('.text-preview', 'color')).toBe('var(--preview-text)')
  expect(declaration('.text-preview .preview-toolbar', 'position')).toBe('sticky')
  expect(declaration('.text-preview .preview-toolbar', 'top')).toBe('0')
  expect(declaration('.text-preview .preview-toolbar', 'color')).toBe(
    'var(--preview-text)',
  )

  for (const selector of [
    '.preview-toolbar button',
    '.preview-toolbar select',
    '.image-preview > footer button',
  ]) {
    expect(declaration(selector, 'background')).toBe('var(--preview-control-surface)')
    expect(declaration(selector, 'border')).toBe(
      '1px solid var(--preview-control-border)',
    )
    expect(declaration(selector, 'border-radius')).toBe('6px')
    expect(declaration(selector, 'color')).toBe('var(--preview-text)')
    expect(declaration(selector, 'min-height')).toBe('30px')
  }

  for (const selector of [
    '.preview-toolbar button:hover:not(:disabled)',
    '.preview-toolbar select:hover',
    '.image-preview > footer button:hover:not(:disabled)',
  ]) {
    expect(declaration(selector, 'background')).toBe(
      'var(--preview-control-hover-surface)',
    )
    expect(declaration(selector, 'border-color')).toBe(
      'var(--preview-control-hover-border)',
    )
  }

  for (const selector of [
    '.preview-toolbar button:focus-visible',
    '.preview-toolbar select:focus-visible',
    '.image-preview > footer button:focus-visible',
  ]) {
    expect(declaration(selector, 'outline')).toBe('2px solid var(--preview-accent)')
    expect(declaration(selector, 'outline-offset')).toBe('2px')
  }

  expect(contrastRatio('#1f2328', '#f5f6f8')).toBeGreaterThanOrEqual(4.5)
  expect(contrastRatio('#68717d', '#fbfcfd')).toBeGreaterThanOrEqual(4.5)
  expect(contrastRatio('#8a94a3', '#ffffff')).toBeGreaterThanOrEqual(3)
  expect(contrastRatio('#747f8e', '#f3f5f7')).toBeGreaterThanOrEqual(3)
  expect(contrastRatio('#9d1c13', '#fff3f0')).toBeGreaterThanOrEqual(4.5)
})
```

The production change this test catches is a preview rule or token returning to an independent literal palette instead of consuming the shared theme.

- [ ] **Step 2: Run the focused test and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "uses one light theme for image and text previews"
```

Expected: FAIL because `:root` does not define `--preview-surface` and the existing preview declarations still resolve to literal colors such as `#f5f6f8` instead of `var(--preview-surface)`.

- [ ] **Step 3: Define the exact root preview variables**

Add these declarations to the existing `:root` rule without changing the current global `color`, `background`, font family, or font synthesis:

```css
:root {
  --preview-surface: #f5f6f8;
  --preview-chrome: #fbfcfd;
  --preview-stage: #edf0f3;
  --preview-document-surface: #f7f7f8;
  --preview-panel-surface: #fff;
  --preview-text: #1f2328;
  --preview-muted: #68717d;
  --preview-border: #d8dce2;
  --preview-control-border: #8a94a3;
  --preview-control-hover-border: #747f8e;
  --preview-control-surface: #fff;
  --preview-control-hover-surface: #f3f5f7;
  --preview-accent: #2477d4;
  --preview-accent-surface: #d9e8ff;
  --preview-accent-text: #174f8f;
  --preview-danger: #9d1c13;
  --preview-danger-surface: #fff3f0;
  --preview-image-shadow: 0 8px 28px rgb(34 42 53 / 14%);
  color: #1f2328;
  background: #f5f6f8;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-synthesis: none;
}
```

- [ ] **Step 4: Migrate every single-image and text preview color to the variables**

Replace only the color-bearing declarations on the existing selectors with the following exact declarations; retain every layout, positioning, spacing, sizing, overflow, transform, and object-fit declaration:

```css
.preview-overlay {
  background: var(--preview-surface);
  color: var(--preview-text);
}

.preview-toolbar {
  background: var(--preview-chrome);
  border-bottom: 1px solid var(--preview-border);
  color: var(--preview-text);
}

.preview-toolbar button,
.preview-toolbar select,
.image-preview > footer button {
  border: 1px solid var(--preview-control-border);
  background: var(--preview-control-surface);
  color: var(--preview-text);
}

.preview-toolbar button:hover:not(:disabled),
.preview-toolbar select:hover,
.image-preview > footer button:hover:not(:disabled) {
  border-color: var(--preview-control-hover-border);
  background: var(--preview-control-hover-surface);
}

.preview-toolbar button:focus-visible,
.preview-toolbar select:focus-visible,
.image-preview > footer button:focus-visible {
  outline: 2px solid var(--preview-accent);
}

.preview-overlay [role="alert"] {
  color: var(--preview-danger);
}

.image-preview-stage {
  background: var(--preview-stage);
}

.image-preview-stage img {
  border: 1px solid var(--preview-border);
  box-shadow: var(--preview-image-shadow);
}

.image-preview > footer {
  background: var(--preview-chrome);
  border-top: 1px solid var(--preview-border);
  color: var(--preview-text);
}

.text-preview {
  background: var(--preview-document-surface);
  color: var(--preview-text);
}

.text-preview .preview-toolbar {
  color: var(--preview-text);
}
```

- [ ] **Step 5: Run the focused theme contract and verify green**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "uses one light theme for image and text previews"
```

Expected: PASS.

- [ ] **Step 6: Run single-image and text preview regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/styles/app.test.ts \
  src/components/ImagePreview.test.tsx \
  src/components/TextPreview.test.tsx
```

Expected: PASS. Image fit/original, zoom, rotation, navigation, text decoding, links, sticky toolbar, focus, and close behavior remain unchanged.

- [ ] **Step 7: Commit the shared theme migration**

```bash
git add ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "refactor: share preview theme variables"
```

---

### Task 2: Replace the comparison workspace’s dark palette

**Files:**

- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/app.css:624-779`
- Test: `ui/src/styles/app.test.ts`
- Test: `ui/src/components/CompareWorkspace.test.tsx`
- Test: `ui/src/components/ComparePane.test.tsx`

**Interfaces:**

- Consumes: all `--preview-*` custom properties produced by Task 1.
- Consumes: the existing `.compare-workspace`, `.compare-toolbar`, `.compare-pane`, `.compare-pane-header`, `.compare-pane-stage`, `.compare-pane > footer`, and `.compare-invalid` markup hooks.
- Produces: light comparison surfaces for valid and invalid two-to-four-image workspaces.
- Preserves: the existing `aria-pressed="true"` contract for synchronized mode, review markers, and favorites.

- [ ] **Step 1: Add a failing comparison-theme contract**

Add this test beside the shared preview-theme contract in `ui/src/styles/app.test.ts`:

```ts
it('renders image comparison from the shared light preview theme', () => {
  const rules = parseRules(appCss)
  const declaration = (selector: string, property: string) =>
    winningDeclaration(rules, new Set([selector]), property)

  expect(declaration('.compare-workspace', 'background')).toBe('var(--preview-surface)')
  expect(declaration('.compare-workspace', 'border-radius')).toBe('10px')
  expect(declaration('.compare-workspace', 'border')).toBe('')
  expect(declaration('.compare-workspace', 'color')).toBe('var(--preview-text)')
  expect(declaration('.compare-toolbar', 'background')).toBe('var(--preview-chrome)')
  expect(declaration('.compare-toolbar', 'border-bottom')).toBe(
    '1px solid var(--preview-border)',
  )
  expect(declaration('.compare-toolbar > span', 'color')).toBe('var(--preview-muted)')
  expect(declaration('.compare-pane-grid', 'background')).toBe('var(--preview-surface)')

  expect(declaration('.compare-pane', 'background')).toBe(
    'var(--preview-panel-surface)',
  )
  expect(declaration('.compare-pane', 'border')).toBe(
    '2px solid var(--preview-border)',
  )
  expect(declaration('.compare-pane.is-active', 'border-color')).toBe(
    'var(--preview-accent)',
  )
  expect(declaration('.compare-pane-header', 'background')).toBe(
    'var(--preview-chrome)',
  )
  expect(declaration('.compare-pane-header', 'border-bottom')).toBe(
    '1px solid var(--preview-border)',
  )
  expect(declaration('.compare-pane-stage', 'background')).toBe('var(--preview-stage)')
  expect(declaration('.compare-pane-stage img', 'outline')).toBe(
    '1px solid var(--preview-border)',
  )
  expect(declaration('.compare-pane-stage img', 'box-shadow')).toBe(
    'var(--preview-image-shadow)',
  )
  expect(declaration('.compare-pane > footer', 'background')).toBe(
    'var(--preview-chrome)',
  )
  expect(declaration('.compare-pane > footer', 'border-top')).toBe(
    '1px solid var(--preview-border)',
  )

  for (const selector of [
    '.compare-toolbar button',
    '.compare-pane button',
    '.compare-invalid > button',
  ]) {
    expect(declaration(selector, 'background')).toBe('var(--preview-control-surface)')
    expect(declaration(selector, 'border')).toBe(
      '1px solid var(--preview-control-border)',
    )
    expect(declaration(selector, 'border-radius')).toBe('5px')
    expect(declaration(selector, 'color')).toBe('var(--preview-text)')
    expect(declaration(selector, 'min-height')).toBe('28px')
  }

  expect(declaration('.compare-pane-header button', 'min-height')).toBe('24px')
  expect(declaration('.compare-pane-header button', 'min-width')).toBe('26px')
  expect(declaration('.compare-pane-header button', 'padding')).toBe('0')
  expect(
    declaration('.compare-pane > footer .marker-buttons button', 'min-height'),
  ).toBe('24px')
  expect(declaration('.compare-pane > footer .marker-buttons button', 'padding')).toBe(
    '2px 5px',
  )

  for (const selector of [
    '.compare-toolbar button:hover:not(:disabled):not([aria-pressed="true"])',
    '.compare-pane button:hover:not(:disabled):not([aria-pressed="true"])',
    '.compare-invalid > button:hover:not(:disabled)',
  ]) {
    expect(declaration(selector, 'background')).toBe(
      'var(--preview-control-hover-surface)',
    )
    expect(declaration(selector, 'border-color')).toBe(
      'var(--preview-control-hover-border)',
    )
  }

  for (const selector of [
    '.compare-toolbar button:focus-visible',
    '.compare-pane button:focus-visible',
    '.compare-invalid > button:focus-visible',
  ]) {
    expect(declaration(selector, 'outline')).toBe('2px solid var(--preview-accent)')
    expect(declaration(selector, 'outline-offset')).toBe('2px')
  }

  expect(
    winningDeclaration(
      rules,
      new Set([
        '.compare-toolbar button[aria-pressed="true"]',
        '.compare-pane .marker-buttons button[aria-pressed="true"]',
      ]),
      'background',
    ),
  ).toBe('var(--preview-accent-surface)')
  expect(
    declaration('.compare-toolbar button[aria-pressed="true"]', 'color'),
  ).toBe('var(--preview-accent-text)')
  expect(
    declaration('.compare-pane .marker-buttons button[aria-pressed="true"]', 'color'),
  ).toBe('var(--preview-accent-text)')

  for (const selector of [
    '.compare-pane > p[role="alert"]',
    '.compare-invalid > p[role="alert"]',
  ]) {
    expect(declaration(selector, 'background')).toBe('var(--preview-danger-surface)')
    expect(declaration(selector, 'color')).toBe('var(--preview-danger)')
  }

  const legacyColors = [
    '#171a1f',
    '#24282f',
    '#0e1013',
    '#20242a',
    '#303640',
    '#4d5663',
    '#275e9d',
    '#4e8ed8',
    '#4e94df',
    '#4b2c22',
    '#ffd5c7',
    '#b8c0ca',
  ]
  for (const rule of rules.filter((candidate) => candidate.selector.startsWith('.compare'))) {
    for (const value of Object.values(rule.declarations)) {
      for (const legacyColor of legacyColors) {
        expect(value, `${rule.selector} ${legacyColor}`).not.toContain(legacyColor)
      }
    }
  }

  expect(contrastRatio('#174f8f', '#d9e8ff')).toBeGreaterThanOrEqual(4.5)
  expect(contrastRatio('#2477d4', '#ffffff')).toBeGreaterThanOrEqual(3)
})
```

The production change this test catches is any comparison surface, state, or control keeping or reintroducing its old dark palette.

- [ ] **Step 2: Run the focused comparison test and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "renders image comparison from the shared light preview theme"
```

Expected: FAIL because `.compare-workspace` still resolves to `#171a1f` instead of `var(--preview-surface)`.

- [ ] **Step 3: Replace the comparison palette with the shared variables**

Change the comparison rules to the following color and state declarations while retaining every existing grid, sizing, overflow, cursor, touch, and transform declaration:

```css
.compare-workspace {
  background: var(--preview-surface);
  color: var(--preview-text);
}

.compare-toolbar {
  background: var(--preview-chrome);
  border-bottom: 1px solid var(--preview-border);
}

.compare-toolbar > span {
  color: var(--preview-muted);
}

.compare-toolbar button,
.compare-pane button,
.compare-invalid > button {
  border: 1px solid var(--preview-control-border);
  border-radius: 5px;
  background: var(--preview-control-surface);
  color: var(--preview-text);
  min-height: 28px;
}

.compare-toolbar button:hover:not(:disabled):not([aria-pressed="true"]),
.compare-pane button:hover:not(:disabled):not([aria-pressed="true"]),
.compare-invalid > button:hover:not(:disabled) {
  border-color: var(--preview-control-hover-border);
  background: var(--preview-control-hover-surface);
}

.compare-toolbar button:focus-visible,
.compare-pane button:focus-visible,
.compare-invalid > button:focus-visible {
  outline: 2px solid var(--preview-accent);
  outline-offset: 2px;
}

.compare-toolbar button[aria-pressed="true"],
.compare-pane .marker-buttons button[aria-pressed="true"] {
  background: var(--preview-accent-surface);
  border-color: var(--preview-accent);
  color: var(--preview-accent-text);
}

.compare-pane-grid {
  background: var(--preview-surface);
}

.compare-pane {
  background: var(--preview-panel-surface);
  border: 2px solid var(--preview-border);
}

.compare-pane.is-active {
  border-color: var(--preview-accent);
}

.compare-pane-header {
  background: var(--preview-chrome);
  border-bottom: 1px solid var(--preview-border);
}

.compare-pane-stage {
  background: var(--preview-stage);
}

.compare-pane-stage img {
  outline: 1px solid var(--preview-border);
  box-shadow: var(--preview-image-shadow);
}

.compare-pane > p[role="alert"],
.compare-invalid > p[role="alert"] {
  background: var(--preview-danger-surface);
  color: var(--preview-danger);
}

.compare-pane > footer {
  background: var(--preview-chrome);
  border-top: 1px solid var(--preview-border);
}

.compare-pane > footer .marker-buttons button {
  border-color: var(--preview-control-border);
}
```

Keep the existing compact overrides on `.compare-pane-header button` and `.compare-pane > footer .marker-buttons button`; they intentionally reduce the 28-pixel comparison-control height to 24 pixels inside pane chrome.

- [ ] **Step 4: Run the focused comparison-theme test and verify green**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "renders image comparison from the shared light preview theme"
```

Expected: PASS.

- [ ] **Step 5: Run all preview and comparison regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/styles/app.test.ts \
  src/components/ImagePreview.test.tsx \
  src/components/TextPreview.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/ComparePane.test.tsx
```

Expected: PASS. Layout selection, synchronized and independent transforms, active-pane focus, zoom, pan, rotation, proxy/original switching, marker states, favorites, pane removal, keyboard exit, single-image preview, and text preview remain unchanged.

- [ ] **Step 6: Commit the light comparison theme**

```bash
git add ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: unify comparison with light preview theme"
```

---

### Task 3: Verify every preview mode in the latest development build

**Files:**

- Verify: `ui/src/styles/app.css`
- Verify: `ui/src/styles/app.test.ts`
- Verify: `ui/src/components/ImagePreview.test.tsx`
- Verify: `ui/src/components/TextPreview.test.tsx`
- Verify: `ui/src/components/CompareWorkspace.test.tsx`
- Verify: `ui/src/components/ComparePane.test.tsx`

**Interfaces:**

- Confirms the complete UI tree compiles and passes.
- Confirms the running macOS application is built from the isolated worktree’s exact source state.
- Confirms single-image, text, and two-, three-, and four-image previews share the approved light visual system.

- [ ] **Step 1: Run static checks**

Run:

```bash
pnpm --dir ui check
```

Expected: exit 0. The repository may continue to report the existing informational Biome configuration deprecation, but it must report no errors and apply no fixes.

- [ ] **Step 2: Run the complete UI test suite**

Run:

```bash
pnpm --dir ui test
```

Expected: all UI test files and tests pass with zero failures.

- [ ] **Step 3: Build the production UI**

Run:

```bash
pnpm --dir ui build
```

Expected: exit 0 with a fresh Vite production bundle.

- [ ] **Step 4: Start the exact development build**

Resolve and stop only an existing Viewer development process whose executable path is verified. From the isolated worktree, run:

```bash
pnpm tauri dev
```

Verify the running executable is the isolated worktree’s `target/debug/viewer-desktop`. Do not launch or install `/Applications/Viewer.app`.

- [ ] **Step 5: Inspect single-image and text previews**

Confirm:

- single-image overlay, toolbar, controls, stage, image boundary, shadow, and footer remain light;
- text document and sticky toolbar remain light;
- default, hover, keyboard-focus, loading, and error states remain readable;
- zoom, fit, original size, rotation, navigation, encoding selection, scrolling, links, and closing still work.

- [ ] **Step 6: Inspect two-, three-, and four-image comparison layouts**

For each layout, confirm:

- outer workspace, main toolbar, grid gaps, pane shells, headers, stages, marker footers, and ordinary controls contain no black or dark-gray surfaces;
- white-background images remain bounded against the light stage;
- the active pane has a blue border;
- synchronized mode has a light-blue pressed state and independent mode returns to a white control;
- active review and favorite buttons retain their pressed state;
- hover and keyboard-focus states are visible;
- alerts use light-red surfaces and dark-red text;
- panes do not clip controls and no unexpected scrollbar appears.

- [ ] **Step 7: Exercise comparison behavior**

Confirm fit, 100%, zoom in/out, rotation, synchronized and independent transforms, active-pane switching, pan, review markers, favorite, pane removal, completion, and Escape behave exactly as before.

- [ ] **Step 8: Record final evidence**

Record the focused red/green test outputs, preview regression pass count, complete-suite pass count, static-check result, build result, exact executable path, and visual observations. If any verification fails, return to the smallest affected task and repeat its red/green cycle before reporting completion.
