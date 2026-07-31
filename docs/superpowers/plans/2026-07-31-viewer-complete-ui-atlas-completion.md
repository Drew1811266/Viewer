# Viewer Complete UI Atlas Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the representative Viewer visual atlas into one self-contained HTML file that visibly covers every interface family and all 17 state groups from the approved visual specification.

**Architecture:** Keep one canonical HTML artifact, but replace the twelve coarse scenes with a declarative scene-and-state catalog. Each scene owns a compact state switcher and renders realistic, local mock data. A Vitest/JSDOM contract loads the real HTML, exercises navigation and state changes, and verifies that no runtime asset path is required.

**Tech Stack:** HTML5, CSS, vanilla JavaScript, Vitest 4, JSDOM 29, Browser visual verification.

## Global Constraints

- Preserve the approved white, warm-gray, and indigo A-density direction.
- Do not change Viewer product behavior or file-operation semantics.
- Keep the 40 px workspace toolbar, 24 px directory rows, 52 px preview toolbar, and thumbnail-only selection ring.
- Keep the radial menu as the only file-action surface.
- Do not introduce a runtime dependency, framework, external web font, or external asset request.
- The final deliverable must open correctly from both `file://` and the local preview server.
- Viewer-owned controls remain cross-platform; only copy such as `⌘/Ctrl` and Trash/Recycle Bin may vary by platform.

---

### Task 1: Add an executable atlas coverage contract

**Files:**
- Create: `ui/src/visualAtlas.test.ts`
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`

**Interfaces:**
- Consumes: the canonical HTML file and its `data-screen`, `data-state`, `data-coverage`, and `data-asset-source` attributes.
- Produces: a real-DOM regression contract for scene coverage, state switching, self-contained assets, and core interaction behavior.

- [ ] **Step 1: Write the failing coverage test**

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { JSDOM } from 'jsdom'
import { describe, expect, it } from 'vitest'

const htmlPath = resolve(import.meta.dirname, '../../docs/prototypes/viewer-complete-ui-visual-atlas.html')
const html = readFileSync(htmlPath, 'utf8')

function renderAtlas() {
  return new JSDOM(html, {
    runScripts: 'dangerously',
    resources: 'usable',
    url: `file://${htmlPath}`,
    beforeParse(window) {
      window.ResizeObserver = class {
        observe() {}
        disconnect() {}
        unobserve() {}
      }
    },
  })
}

describe('complete Viewer visual atlas', () => {
  it('publishes every approved state group and no external asset paths', () => {
    const dom = renderAtlas()
    const document = dom.window.document
    expect(document.querySelectorAll('[data-coverage]').length).toBe(17)
    expect(document.querySelectorAll('[data-screen]').length).toBeGreaterThanOrEqual(17)
    expect(document.querySelector('[data-coverage="radial"]')).not.toBeNull()
    expect(document.querySelector('[data-coverage="unsupported-files"]')).not.toBeNull()
    expect(document.querySelector('[data-coverage="operation-results"]')).not.toBeNull()
    expect(html).not.toContain('const assetBase = "/files/"')
    expect(document.querySelectorAll('img[src^="/"], img[src^="http"]').length).toBe(0)
  })
})
```

- [ ] **Step 2: Run the focused test and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: FAIL because the existing atlas exposes only 12 screen entries, has no 17-group coverage attributes, and loads images from `/files/`.

- [ ] **Step 3: Add the catalog contract and self-contained asset slots**

Add `data-coverage` markers for all 17 state groups, expand the navigation model, and replace runtime image paths with data-URI asset slots. Do not change the visual design yet beyond what is necessary for the contract to render.

- [ ] **Step 4: Run the focused test**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit the coverage contract**

```bash
git add ui/src/visualAtlas.test.ts docs/prototypes/viewer-complete-ui-visual-atlas.html
git commit -m "test: define complete visual atlas coverage"
```

---

### Task 2: Complete shell, browsing, selection, drag, search, and filter states

**Files:**
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Modify: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: scene catalog and existing Viewer shell/image fixtures.
- Produces: interactive state controls for sidebar, folder context, thumbnail density/selection, other files/drag, search result, and filter coverage groups.

- [ ] **Step 1: Add failing interaction tests**

Test real clicks on the state switcher and assert visible outcomes:

```ts
const button = [...document.querySelectorAll<HTMLButtonElement>('[data-state]')]
  .find((item) => item.dataset.state === 'sidebar-collapsed')
button?.click()
expect(document.querySelector('[data-viewer-sidebar]')?.getAttribute('data-mode')).toBe('collapsed')
```

Add equivalent assertions for:

- standard/compact/large thumbnail density;
- no selection/single/multiple/focus;
- other-files expanded and organization drag;
- grouped/flat/indexing/paging/empty search;
- zero/one/multiple/advanced filters.

- [ ] **Step 2: Run the focused test and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts -t "shell and browsing states"
```

Expected: FAIL because those state buttons and visible outcomes do not exist.

- [ ] **Step 3: Implement the shell and browsing state renderers**

Add bounded renderers that preserve the approved geometry:

- `shellStatesScreen`
- `contentStatesScreen`
- `otherFilesAndDragScreen`
- `searchStatesScreen`
- `filterStatesScreen`

Each renderer must expose all states listed above without duplicating product logic or adding permanent local toolbars.

- [ ] **Step 4: Run the focused and full atlas tests**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit shell and browsing coverage**

```bash
git add ui/src/visualAtlas.test.ts docs/prototypes/viewer-complete-ui-visual-atlas.html
git commit -m "feat: complete atlas browsing and search states"
```

---

### Task 3: Complete radial, preview, compare, text, unsupported-file, and information states

**Files:**
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Modify: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: approved radial raster reference, preview shell, fixture images, and shared inspector language.
- Produces: state surfaces for radial invocation modes, preview transforms/errors, compare counts, text variants, unsupported/unavailable files, and single/multi information.

- [ ] **Step 1: Add failing state tests**

Exercise state buttons and assert:

- radial click/gesture/mark/organize/disabled/read-only/keyboard labels and menu semantics;
- preview fit/100/zoom/rotate/loading/error/navigation states;
- compare 2/3/4/large-count layouts;
- Markdown/plain/encoding/truncated/dual/unsupported/unavailable states;
- single and multiple information inspector states.

- [ ] **Step 2: Run and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts -t "viewing and radial states"
```

Expected: FAIL because the current radial view is one static reference image and the viewing surfaces expose only one state each.

- [ ] **Step 3: Implement viewing and radial state renderers**

Reuse the approved radial raster as the visual truth and add semantic state panels and real menu-role controls around it; do not redraw its geometry with CSS, SVG, canvas, gradients, emoji, or text-glyph art. Add:

- `radialStatesScreen`
- `previewStatesScreen`
- `compareStatesScreen`
- `documentStatesScreen`
- `informationStatesScreen`

Keep the approved `完成` action and three-part toolbar.

- [ ] **Step 4: Run the full atlas test**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit viewing coverage**

```bash
git add ui/src/visualAtlas.test.ts docs/prototypes/viewer-complete-ui-visual-atlas.html
git commit -m "feat: complete atlas viewing and radial states"
```

---

### Task 4: Complete dialogs, tasks, results, notices, empty, loading, recovery, and accessibility states

**Files:**
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Modify: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: shared dialog, task, notice, error, and loading components.
- Produces: settings/single rename/remaining dialog states, task outcomes, results inspector, scoped feedback, empty/recovery surfaces, and accessibility review states.

- [ ] **Step 1: Add failing completion tests**

Assert visible states for:

- settings, single rename, batch rename, destination/conflict, trash, close-task dialogs;
- running/success/failure/cancelled/result tasks;
- operation results, global notice, local error, read-only, and recovery;
- no-project/drag/invalid-drag/opening/loading/thumbnail/empty/error/recovery;
- focus restoration, reduced motion, forced colors, and 200% zoom review examples.

- [ ] **Step 2: Run and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts -t "dialogs feedback and recovery states"
```

Expected: FAIL because the current atlas covers only four dialogs, one running task, and four launch states.

- [ ] **Step 3: Implement the remaining state renderers**

Add:

- `dialogStatesScreen`
- `taskStatesScreen`
- `feedbackStatesScreen`
- `launchAndRecoveryScreen`
- `accessibilityStatesScreen`

Keep unsafe actions restrained, show task outcomes separately, and preserve exact three-element no-project resting content.

- [ ] **Step 4: Run the full atlas test**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit remaining state coverage**

```bash
git add ui/src/visualAtlas.test.ts docs/prototypes/viewer-complete-ui-visual-atlas.html
git commit -m "feat: complete atlas dialogs and feedback states"
```

---

### Task 5: Embed optimized assets, perform visual QA, and correct the coverage claim

**Files:**
- Modify: `docs/prototypes/viewer-complete-ui-visual-atlas.html`
- Modify: `design-qa.md`
- Test: `ui/src/visualAtlas.test.ts`

**Interfaces:**
- Consumes: final state catalog and optimized raster assets.
- Produces: one portable HTML artifact, a clean browser run, exact viewport screenshots, combined source/implementation comparison, and a truthful final QA result.

- [ ] **Step 1: Run the standalone contract**

Run:

```bash
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS with no external asset paths.

- [ ] **Step 2: Verify both direct-file and preview-server rendering**

Open the canonical HTML from `file://` and through the visual companion. Exercise every navigation entry and every state tab at 1024×720 and 1440×900. Confirm no browser console errors.

- [ ] **Step 3: Run design QA**

Capture source/reference and implementation at the same state and viewport, place them in one combined comparison image, fix all P0/P1/P2 findings, and update `design-qa.md` with exact evidence paths and `final result: passed`.

- [ ] **Step 4: Run final repository checks**

Run:

```bash
git diff --check
pnpm --dir ui exec vitest run src/visualAtlas.test.ts
```

Expected: PASS with no warnings or failures.

- [ ] **Step 5: Commit the complete standalone atlas**

```bash
git add docs/prototypes/viewer-complete-ui-visual-atlas.html ui/src/visualAtlas.test.ts design-qa.md
git commit -m "docs: complete standalone Viewer UI visual atlas"
```

