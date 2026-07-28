# Viewer Light Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to execute this plan.

**Goal:** Make image and text previews visually continuous with Viewer’s default light interface while preserving every existing preview interaction.

**Architecture:** Keep the existing `ImagePreview` and `TextPreview` components and their shared `.preview-overlay` / `.preview-toolbar` structure. Express the approved light appearance entirely in the shared stylesheet, then protect the final cascade with focused CSS contract tests and the existing component interaction suites.

**Tech Stack:** React 19, TypeScript, CSS, Vitest, Testing Library, pnpm, Tauri 2

## Global Constraints

- Work from an isolated git worktree because the main workspace already contains unrelated user changes.
- Do not modify `ImagePreview.tsx`, `TextPreview.tsx`, preview state, keyboard shortcuts, zoom behavior, navigation, text encoding, or dialog behavior.
- Do not introduce a theme system or change the surrounding workspace.
- Preserve the current light text-body surface and sticky text toolbar.
- Stage and commit only the files listed in this plan.

---

### Task 1: Lock the approved light-preview contract

**Files:**

- Modify: `ui/src/styles/app.test.ts`
- Test: `ui/src/styles/app.test.ts`

**Interfaces:**

- Consumes the existing CSS parsing helpers `parseRules`, `winningDeclaration`, and `contrastRatio`.
- Verifies the final declarations for `.preview-overlay`, `.preview-toolbar`, `.image-preview-stage`, `.image-preview-stage img`, `.image-preview > footer`, `.text-preview`, and `.text-preview .preview-toolbar`.
- Verifies normal, hover, and keyboard-focus states for toolbar and footer controls.

**Step 1: Add a failing CSS contract test**

Append one focused test beside the existing application-style contract tests:

```ts
it('renders image and text previews as shared light surfaces', () => {
  const rules = parseRules(appCss)
  const declaration = (selector: string, property: string) =>
    winningDeclaration(rules, new Set([selector]), property)

  expect(declaration('.preview-overlay', 'background')).toBe('#f5f6f8')
  expect(declaration('.preview-overlay', 'color')).toBe('#1f2328')

  expect(declaration('.preview-toolbar', 'background')).toBe('#fbfcfd')
  expect(declaration('.preview-toolbar', 'border-bottom')).toBe('1px solid #d8dce2')
  expect(declaration('.preview-toolbar', 'color')).toBe('#1f2328')

  expect(declaration('.image-preview-stage', 'background')).toBe('#edf0f3')
  expect(declaration('.image-preview-stage img', 'border')).toBe('1px solid #d8dce2')
  expect(declaration('.image-preview-stage img', 'box-shadow')).toBe(
    '0 8px 28px rgb(34 42 53 / 14%)',
  )

  expect(declaration('.image-preview > footer', 'background')).toBe('#fbfcfd')
  expect(declaration('.image-preview > footer', 'border-top')).toBe('1px solid #d8dce2')
  expect(declaration('.image-preview > footer', 'color')).toBe('#1f2328')
  expect(declaration('.preview-overlay [role="alert"]', 'color')).toBe('#9d1c13')

  expect(declaration('.text-preview', 'background')).toBe('#f7f7f8')
  expect(declaration('.text-preview', 'color')).toBe('#1f2328')
  expect(declaration('.text-preview .preview-toolbar', 'position')).toBe('sticky')
  expect(declaration('.text-preview .preview-toolbar', 'top')).toBe('0')
  expect(declaration('.text-preview .preview-toolbar', 'color')).toBe('#1f2328')

  for (const selector of [
    '.preview-toolbar button',
    '.preview-toolbar select',
    '.image-preview > footer button',
  ]) {
    expect(declaration(selector, 'background')).toBe('#fff')
    expect(declaration(selector, 'border')).toBe('1px solid #8a94a3')
    expect(declaration(selector, 'border-radius')).toBe('6px')
    expect(declaration(selector, 'color')).toBe('#1f2328')
    expect(declaration(selector, 'min-height')).toBe('30px')
  }

  expect(contrastRatio('#8a94a3', '#ffffff')).toBeGreaterThanOrEqual(3)

  for (const selector of [
    '.preview-toolbar button:hover:not(:disabled)',
    '.preview-toolbar select:hover',
    '.image-preview > footer button:hover:not(:disabled)',
  ]) {
    expect(declaration(selector, 'background')).toBe('#f3f5f7')
    expect(declaration(selector, 'border-color')).toBe('#747f8e')
  }

  expect(contrastRatio('#747f8e', '#f3f5f7')).toBeGreaterThanOrEqual(3)

  for (const selector of [
    '.preview-toolbar button:focus-visible',
    '.preview-toolbar select:focus-visible',
    '.image-preview > footer button:focus-visible',
  ]) {
    expect(declaration(selector, 'outline')).toBe('2px solid #2477d4')
    expect(declaration(selector, 'outline-offset')).toBe('2px')
  }
})
```

If the test file uses a different variable name for the imported stylesheet, use that existing name instead of introducing a duplicate fixture.

**Step 2: Run the focused test and confirm the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "renders image and text previews"
```

Expected: FAIL because the current overlay and toolbar still resolve to the dark colors `rgba(18, 20, 24, 0.97)` and `#20242a`, and the approved control and image-boundary declarations do not exist.

**Step 3: Commit the red test**

```bash
git add ui/src/styles/app.test.ts
git commit -m "test: define light preview styling contract"
```

---

### Task 2: Apply the shared light-preview surfaces

**Files:**

- Modify: `ui/src/styles/app.css`
- Test: `ui/src/styles/app.test.ts`

**Interfaces:**

- `.preview-overlay` provides the common preview canvas and foreground color.
- `.preview-toolbar` and `.image-preview > footer` provide matching light chrome.
- Toolbar buttons, text-encoding select, and footer navigation buttons share one control treatment.
- `.image-preview-stage` separates the image from the surrounding interface without returning to a dark canvas.
- `.text-preview` keeps its existing reading surface; its sticky toolbar inherits the shared light chrome.

**Step 1: Replace the dark shared preview colors**

Update the existing preview rules in `ui/src/styles/app.css` so the declarations resolve to:

```css
.preview-overlay {
  background: #f5f6f8;
  color: #1f2328;
}

.preview-toolbar {
  background: #fbfcfd;
  border-bottom: 1px solid #d8dce2;
  color: #1f2328;
}
```

Keep all existing layout, positioning, spacing, and overflow declarations on these selectors.

**Step 2: Give every preview control a consistent button shape**

Add the shared control rules:

```css
.preview-toolbar button,
.preview-toolbar select,
.image-preview > footer button {
  min-height: 30px;
  padding: 4px 8px;
  border: 1px solid #8a94a3;
  border-radius: 6px;
  background: #fff;
  color: #1f2328;
}

.preview-toolbar button:hover:not(:disabled),
.preview-toolbar select:hover,
.image-preview > footer button:hover:not(:disabled) {
  border-color: #747f8e;
  background: #f3f5f7;
}

.preview-toolbar button:focus-visible,
.preview-toolbar select:focus-visible,
.image-preview > footer button:focus-visible {
  outline: 2px solid #2477d4;
  outline-offset: 2px;
}
```

Retain native disabled behavior and the existing component semantics. Do not replace buttons or selects with custom elements.

**Step 3: Finish the image-stage and footer treatment**

Update or add the relevant declarations:

```css
.preview-overlay [role="alert"] {
  color: #9d1c13;
}

.image-preview-stage {
  background: #edf0f3;
}

.image-preview-stage img {
  border: 1px solid #d8dce2;
  box-shadow: 0 8px 28px rgb(34 42 53 / 14%);
}

.image-preview > footer {
  background: #fbfcfd;
  border-top: 1px solid #d8dce2;
  color: #1f2328;
}
```

Keep image sizing, object fitting, transformations, zoom, and stage alignment unchanged.

**Step 4: Preserve the light text-reading surface**

Keep the existing text preview body colors:

```css
.text-preview {
  background: #f7f7f8;
  color: #1f2328;
}

.text-preview .preview-toolbar {
  position: sticky;
  top: 0;
  color: #1f2328;
}
```

Remove only obsolete dark-toolbar color overrides; keep the sticky behavior and all other text-preview layout declarations.

**Step 5: Run the focused contract test**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "renders image and text previews"
```

Expected: PASS.

**Step 6: Run preview regression tests**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/styles/app.test.ts \
  src/components/ImagePreview.test.tsx \
  src/components/TextPreview.test.tsx
```

Expected: PASS. Image zoom, fit, rotate, close, previous/next navigation, text encoding, and text close behavior remain unchanged.

**Step 7: Commit the implementation**

```bash
git add ui/src/styles/app.css
git commit -m "fix: align previews with light workspace"
```

---

### Task 3: Verify the latest development build end to end

**Files:**

- Verify: `ui/src/styles/app.css`
- Verify: `ui/src/styles/app.test.ts`
- Verify: `ui/src/components/ImagePreview.test.tsx`
- Verify: `ui/src/components/TextPreview.test.tsx`

**Interfaces:**

- Confirms code quality and production compilation.
- Confirms that the app launched from the isolated worktree is the new development build, not an installed application bundle.
- Confirms image and text preview appearance against the approved visual design.

**Step 1: Run static checks**

Run:

```bash
pnpm --dir ui check
```

Expected: PASS with no TypeScript or lint errors.

**Step 2: Run the complete UI test suite**

Run:

```bash
pnpm --dir ui test
```

Expected: PASS with all UI test files and tests green.

**Step 3: Build the UI**

Run:

```bash
pnpm --dir ui build
```

Expected: PASS and a fresh production UI bundle.

**Step 4: Start the exact development binary**

Stop only an existing Viewer development process whose executable path has first been verified. From the isolated worktree, run:

```bash
pnpm tauri dev
```

Verify the launched process points to that worktree’s `target/debug/viewer-desktop`. Do not launch `/Applications/Viewer.app` or any other packaged copy.

**Step 5: Inspect image preview**

Open a populated image folder and double-click an image. Confirm:

- The full-window preview canvas is light gray, not black.
- The top toolbar and bottom navigation bar use the same light surface and border language as the main Viewer window.
- Buttons and the zoom readout are clearly shaped, readable, and remain usable.
- The image has a subtle boundary and shadow against the light stage.
- Zoom, fit-to-window, rotate, previous, next, close, and keyboard focus still work.
- No clipping or unwanted scrollbar appears at the tested window size.

**Step 6: Inspect text preview**

Double-click a text file. Confirm:

- The toolbar matches the image preview’s light toolbar.
- The text body remains a comfortable light reading surface.
- The encoding selector and close button share the approved control style.
- The toolbar remains sticky while the text body scrolls.

**Step 7: Record verification evidence**

Capture the commands run, their pass counts, the exact development executable path, and any visual discrepancy. If a check fails, return to the smallest affected task and repeat its red/green cycle before claiming completion.
