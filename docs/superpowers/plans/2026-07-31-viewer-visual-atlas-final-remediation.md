# Viewer Visual Atlas Final Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct every P1/P2 issue from the final Viewer visual-atlas audit, verify the corrected atlas visually and behaviorally, and place the accepted evidence into a Figma audit board.

**Architecture:** Keep the standalone HTML atlas and its Vitest/JSDOM contract. Replace duplicated state strings with derived state helpers, render radial and accessibility states as real Viewer surfaces, and keep atlas controls in reserved prototype chrome. Browser verification captures fresh evidence only after the automated contract is green.

**Tech Stack:** HTML5, CSS, vanilla JavaScript, Vitest 4, JSDOM 29, Browser visual verification, Figma.

## Global Constraints

- Preserve the approved white, warm-gray, and indigo A-density direction.
- Do not change Viewer product behavior or file-operation semantics.
- Keep the 40 px workspace toolbar, 24 px directory rows, 52 px preview toolbar, and thumbnail-only selection ring.
- Keep the radial menu as the only visible file-action surface.
- Do not add runtime dependencies, external web fonts, or external asset requests.
- Keep direct `file://` and local preview-server rendering functional.
- Use a minimum 32 px effective target except for the approved 24 px continuous directory rows.
- Keep information and result inspectors between 320 px and 340 px.

---

### Task 1: Lock the audit findings into failing regression tests

**Files:**
- Modify: `ui/src/visualAtlas.test.ts`
- Test: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: the real standalone atlas through `renderAtlas()`.
- Produces: behavioral contracts for shell, filter, radial, accessibility, dialog, and sizing corrections.

- [ ] **Step 1: Add helpers that exercise real user behavior**

Add:

```ts
function clickToolbarControl(document: Document, label: string) {
  const button = [...document.querySelectorAll<HTMLButtonElement>('.viewer-toolbar button')].find(
    (candidate) => candidate.textContent?.trim().startsWith(label),
  )
  expect(button).not.toBeNull()
  button?.click()
}

function keydown(window: Window, target: Element, key: string) {
  target.dispatchEvent(new window.KeyboardEvent('keydown', { bubbles: true, key }))
}
```

- [ ] **Step 2: Add one failing test per audited behavior**

Add tests that assert:

```ts
it('opens filters in place and derives one consistent condition count', () => {
  const dom = renderAtlas()
  const document = dom.window.document
  clickScreen(document, 'filters')
  clickState(document, 'filters-advanced')
  const screenBefore = document.querySelector('[data-filter-state]')
  clickToolbarControl(document, '筛选')
  expect(document.querySelector('[data-filter-popover]')).not.toBeNull()
  expect(document.querySelector('[data-filter-state]')).toBe(screenBefore)
  expect(document.querySelector('[data-filter-trigger-count]')?.textContent).toBe(
    document.querySelector('[data-filter-panel-count]')?.textContent,
  )
})
```

```ts
it('renders a compact accessible collapsed sidebar header', () => {
  const dom = renderAtlas()
  const document = dom.window.document
  clickScreen(document, 'sidebar')
  clickState(document, 'sidebar-collapsed')
  const header = document.querySelector('[data-project-head="collapsed"]')
  expect(header?.querySelector('[aria-label="展开项目目录"]')).not.toBeNull()
  expect(header?.textContent?.trim()).toBe('')
})
```

```ts
it('renders distinct real radial geometry for every visual state', () => {
  const dom = renderAtlas()
  const document = dom.window.document
  clickScreen(document, 'radial')
  const states = ['mark', 'organize', 'disabled', 'readonly', 'keyboard']
  for (const value of states) {
    clickState(document, `radial-${value}`)
    expect(document.querySelector(`[data-radial-visual-state="${value}"]`)).not.toBeNull()
    expect(document.querySelector('.radial-demo-menu [data-level="primary"]')).not.toBeNull()
  }
  clickState(document, 'radial-mark')
  expect(document.querySelector('[data-secondary-kind="mark"]')).not.toBeNull()
  clickState(document, 'radial-organize')
  expect(document.querySelector('[data-secondary-kind="organize"]')).not.toBeNull()
})
```

```ts
it('supports keyboard selection and exposes progress and dialog semantics', () => {
  const dom = renderAtlas()
  const document = dom.window.document
  clickScreen(document, 'browser')
  const card = document.querySelector<HTMLElement>('.image-card')
  expect(card).not.toBeNull()
  keydown(dom.window, card as Element, 'Enter')
  expect(card?.getAttribute('aria-selected')).toBe('true')
  clickScreen(document, 'launch')
  clickState(document, 'launch-scanning')
  expect(document.querySelector('[role="progressbar"][aria-valuenow]')).not.toBeNull()
  expect(document.querySelector('[role="status"][aria-live]')).not.toBeNull()
  clickScreen(document, 'dialogs')
  expect(document.querySelector('[role="dialog"][aria-modal="true"]')).not.toBeNull()
})
```

```ts
it('associates field labels and exposes real accessibility shell states', () => {
  const dom = renderAtlas()
  const document = dom.window.document
  clickScreen(document, 'filters')
  clickState(document, 'filters-advanced')
  for (const label of document.querySelectorAll<HTMLLabelElement>('label[for]')) {
    expect(document.getElementById(label.htmlFor)).not.toBeNull()
  }
  clickScreen(document, 'accessibility')
  for (const value of ['keyboard', 'restore', 'reduced', 'forced', 'zoom']) {
    clickState(document, `accessibility-${value}`)
    expect(document.querySelector(`[data-accessible-viewer-state="${value}"]`)).not.toBeNull()
    expect(document.querySelector('[data-accessible-viewer-state] .viewer-shell')).not.toBeNull()
  }
})
```

- [ ] **Step 3: Run the focused test and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: FAIL on the new filter popover, collapsed header, radial visual,
keyboard selection, progress semantics, dialog semantics, label association,
and accessibility-shell assertions.

- [ ] **Step 4: Commit the failing contract**

```bash
git add ui/src/visualAtlas.test.ts
git commit -m "test: capture final visual atlas defects"
```

---

### Task 2: Correct shell, filter, menu, and sizing behavior

**Files:**
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Test: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: `state.screen`, `state.openMenu`, `state.filters`, and shell options.
- Produces: `filterConditionModel()`, a real filter popover, compact collapsed project header, global disabled menu treatment, and approved sizing tokens.

- [ ] **Step 1: Implement one derived filter model**

Add a helper returning selected file types, review states, advanced rules, and
`count`. Render both `data-filter-trigger-count` and
`data-filter-panel-count` from that `count`.

- [ ] **Step 2: Keep the filter trigger on the current screen**

Change the `data-open="filter"` handler to toggle `state.openMenu`. Render the
filter panel with `data-filter-popover` from the shared toolbar menu layer.

- [ ] **Step 3: Replace collapsed project-head text with one icon control**

Render:

```html
<header class="project-head collapsed" data-project-head="collapsed">
  <button class="icon-button" type="button" aria-label="展开项目目录"></button>
</header>
```

Use the project’s existing chevron icon asset or existing icon font/library;
do not add a text label inside the 52 px cell.

- [ ] **Step 4: Apply approved global styles**

Set:

- `.viewer-button` and `.segmented button` to a minimum effective height of
  32 px;
- `--tertiary` to a value that reaches 4.5:1 on white and sidebar surfaces;
- `.information-layout` inspector column to `334px`;
- global `[aria-disabled="true"]` menu-row styling;
- state-switcher positioning inside reserved atlas chrome.

- [ ] **Step 5: Run the focused test**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts -t "filters|collapsed|approved"
```

Expected: PASS for shell, filter, menu, and sizing contracts.

- [ ] **Step 6: Commit shell and toolbar corrections**

```bash
git add docs/prototypes/viewer-complete-ui-visual-atlas.html ui/src/visualAtlas.test.ts
git commit -m "fix: correct atlas shell and filter states"
```

---

### Task 3: Replace radial explanations with real radial state visuals

**Files:**
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Test: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: production radial geometry and the approved radial command model.
- Produces: `radialDemoMarkup(radialState)` with primary sectors, local secondary sectors, center context, disabled cues, gesture trace, and focus state.

- [ ] **Step 1: Add a standalone radial renderer based on production geometry**

Use the same 336 px view box, 42/108 px primary radii, 112/168 px secondary
radii, primary order, center content, labels, and semantic menu roles as
`RadialFileMenu.tsx`. Generate sector paths from one
`annularSectorPath()` helper.

- [ ] **Step 2: Map each atlas state to visible radial data**

Implement literal state data for:

- click;
- gesture;
- mark secondary;
- organize secondary;
- disabled;
- read-only;
- keyboard focus.

Every state adds `data-radial-visual-state`. Secondary rings add
`data-secondary-kind`; disabled sectors use both `aria-disabled` and a visible
non-color cue.

- [ ] **Step 3: Make the live radial the dominant artifact**

Place the live radial at reviewable size in the main workspace. Keep the
approved raster only as a small reference thumbnail and remove the statement
that alternate radial states are not redrawn.

- [ ] **Step 4: Run radial tests**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts -t "radial"
```

Expected: PASS for distinct primary, secondary, disabled, read-only, and
keyboard visuals.

- [ ] **Step 5: Commit radial state corrections**

```bash
git add docs/prototypes/viewer-complete-ui-visual-atlas.html ui/src/visualAtlas.test.ts
git commit -m "fix: render complete radial menu states"
```

---

### Task 4: Implement real accessibility, keyboard, progress, and modal states

**Files:**
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Test: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: shell renderer, image-card selection, launch progress, dialogs,
  field renderers, and accessibility scene state.
- Produces: keyboard-operable cards, associated fields, live progress,
  shell-wide modal semantics, and real Viewer accessibility demonstrations.

- [ ] **Step 1: Add keyboard selection behavior**

In the shared keydown handler, toggle the focused image card for Enter and
Space, update `aria-selected`, and update the selection summary.

- [ ] **Step 2: Associate labels and controls**

Give each select/input a stable scene-prefixed id and every label a matching
`for`. Preserve the visible field layout.

- [ ] **Step 3: Add progress and live-status semantics**

Render scan bars with `role="progressbar"`, `aria-valuemin="0"`,
`aria-valuemax="100"`, and the current `aria-valuenow`. Add a concise
`role="status" aria-live="polite"` progress sentence.

- [ ] **Step 4: Make dialogs shell-wide and modal**

Place the scrim above the complete Viewer shell. Add `role="dialog"`,
`aria-modal="true"`, and labelled title ids. Escape closes the dialog and
restores focus to the triggering control.

- [ ] **Step 5: Render actual Viewer accessibility states**

Wrap the real shell in `data-accessible-viewer-state` and apply:

- keyboard: visible focus and selection on separate layers;
- restore: returned focus on the prior image card;
- reduced: zero non-essential transition duration;
- forced: system color tokens and explicit borders;
- zoom: a 200% content reflow simulation without horizontal loss of the
  primary action.

- [ ] **Step 6: Run the focused and full atlas tests**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts -t "keyboard|progress|dialog|accessibility"
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS with no JSDOM errors or warnings.

- [ ] **Step 7: Commit accessibility corrections**

```bash
git add docs/prototypes/viewer-complete-ui-visual-atlas.html ui/src/visualAtlas.test.ts
git commit -m "fix: complete atlas accessibility states"
```

---

### Task 5: Run final visual acceptance and build the Figma audit board

**Files:**
- Modify: `design-qa.md`
- Create: `target/final-design-acceptance-2026-07-31/`
- Test: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: the corrected standalone atlas and the approved visual
  specification.
- Produces: fresh accepted screenshots, inline audit notes, and one Figma
  section containing the same screenshots and notes.

- [ ] **Step 1: Run repository verification**

Run:

```bash
git diff --check
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
pnpm --dir ui check
pnpm --dir ui build
```

Expected: all commands exit 0 with no failures.

- [ ] **Step 2: Capture corrected states at 1024×720**

Capture and inspect:

- collapsed sidebar;
- advanced filter open;
- read-only more menu;
- radial click, mark, organize, disabled, read-only, and keyboard;
- keyboard image selection;
- scanning progress;
- modal dialog;
- forced-colors and 200% reflow.

- [ ] **Step 3: Capture representative states at 1440×900**

Capture and inspect the main browser, filter, radial, preview, dialog, results,
and accessibility surfaces. Reject any screenshot with overlap, clipping,
loading artifacts, or atlas controls covering Viewer controls.

- [ ] **Step 4: Update the final QA record**

Record each accepted screenshot, viewport, corrected finding, remaining
evidence limit, and final result in `design-qa.md`. Do not claim full WCAG
compliance from screenshots.

- [ ] **Step 5: Create the Figma audit board**

Create a Figma design file if no approved destination file exists. Place the
accepted screenshots left-to-right with 200 px gaps, start a new row after 15
screens with 600 px row separation, add step number/name/health/notes below
each screenshot, and wrap all added content in a section titled
`Viewer Final UI Visual Acceptance — 2026-07-31`.

- [ ] **Step 6: Inspect the Figma output**

Render or inspect the section and confirm every accepted screenshot is visible
in the correct card with its corresponding notes.

- [ ] **Step 7: Commit the verified QA record**

```bash
git add design-qa.md
git commit -m "docs: record final visual atlas acceptance"
```
