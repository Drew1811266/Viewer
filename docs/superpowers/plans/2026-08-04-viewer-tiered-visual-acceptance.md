# Viewer Tiered Visual Acceptance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 89 × 2 all-native visual capture bottleneck with a release-isolated formal-component harness and one-session Playwright batch, while retaining a small PID-bound Tauri smoke matrix for genuine desktop boundaries.

**Architecture:** A separate Vite entry renders deterministic acceptance scenes made only from Viewer production components, icons and styles. A Node/Playwright runner reuses one Chromium process, captures exact dual-viewport product images, combines them with hash-bound atlas references and records bounded manifests; the existing native controller is narrowed to 15 desktop integration journeys instead of every visual state.

**Tech Stack:** React 19, TypeScript 6, Vite 8, Vitest 4, Playwright Chromium, Node.js ESM, existing Viewer production components and CSS, existing PNG evidence codec, existing Swift/macOS native controller.

## Global Constraints

- Work only in `/Users/abc/Project/Viewer/.worktrees/viewer-atlas-product-migration` on `codex/viewer-atlas-product-migration`; never implement on `main`.
- The approved design is `docs/superpowers/specs/2026-08-04-viewer-tiered-visual-acceptance-design.md` and supersedes the old 89-state all-native requirement.
- The visual harness must import formal Viewer components, formal icon assets and the exact production style entries; it must not copy atlas JSX/HTML/CSS as product implementation.
- `pnpm build:ui` and the Tauri release bundle must not contain `visual-acceptance.html`, `src/acceptance`, acceptance fixtures, Playwright or acceptance query handling.
- Playwright is a root development dependency only and may install Chromium only for local/CI acceptance.
- The approved product viewports are exactly `1024 × 720` and `1440 × 900` CSS pixels.
- State entry must be bounded and observable: no unlimited polling, recursive retry or fixed long sleep.
- Generated files remain ignored below `target/viewer-visual-acceptance/`; source fixtures and catalog code are committed.
- An automated screenshot starts as `pending-visual-review`; only a same-viewport reference/product combined inspection can mark visual acceptance passed.
- Browser visual evidence does not prove native window chrome, Finder, keyboard synthesis, drag-and-drop or platform accessibility behavior; those stay in the native smoke matrix.
- Use TDD for every behavior change: write a focused failing test, observe the expected failure, implement the minimum behavior, rerun focused tests, then run the relevant broader gate.
- Preserve existing user changes and generated evidence. Never stage `target/` artifacts.
- Keep exactly one Viewer development instance when native smoke is run; do not run browser visual capture and native smoke simultaneously.

## File Structure

```text
ui/visual-acceptance.html
  Non-release HTML entry containing only the acceptance root.

ui/vite.visual-acceptance.config.ts
  Dedicated Vite dev/build config; output is target/viewer-visual-acceptance/site.

ui/src/acceptance/
  acceptanceStateCatalog.ts       89 IDs, wave/reference/component coverage metadata.
  acceptanceStateCatalog.test.ts  Ledger equality, scene coverage and public contract tests.
  acceptanceRequest.ts            Strict id/viewport query parsing.
  acceptanceRequest.test.ts       Query and exact viewport tests.
  acceptanceFixtures.ts           Deterministic product files, images, text and operation data.
  acceptanceBridge.ts             Complete ViewerBridge fixture with fail-fast unused calls.
  AcceptanceApp.tsx               Scene dispatch and pending/ready/error protocol.
  AcceptanceApp.test.tsx          One-state render and status protocol tests.
  main.tsx                         Acceptance-only React entry and formal CSS imports.
  scenes/workspaceScenes.tsx      Wave 1 formal shell/browsing states.
  scenes/viewingScenes.tsx        Wave 2 radial/preview/compare/document/info states.
  scenes/dialogScenes.tsx         Wave 3 dialog states.
  scenes/feedbackScenes.tsx       Wave 3 task/result plus Wave 4 accessibility states.
  scenes/*.test.tsx               State-specific visible assertions.

scripts/viewer-acceptance-evidence.mjs
  Shared PNG read/write/combine and hash-bound evidence manifest helpers.

scripts/viewer-visual-acceptance.mjs
  CLI, Vite lifecycle, one-browser Playwright capture, state batching and summaries.

scripts/viewer-visual-acceptance.test.mjs
  CLI selection, process reuse, timeout, cleanup, evidence and production isolation tests.

docs/reviews/viewer-native-smoke-matrix.md
  Fifteen approved native journeys and evidence meaning.
```

---

### Task 1: Establish the 89-State Catalog Contract

**Files:**
- Create: `ui/src/acceptance/acceptanceStateCatalog.ts`
- Create: `ui/src/acceptance/acceptanceStateCatalog.test.ts`
- Modify: `ui/src/visualMigrationCoverage.test.ts`

**Interfaces:**
- Consumes: the 89 rows in `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`.
- Produces:
  - `AcceptanceId`
  - `AcceptanceWave = 1 | 2 | 3 | 4`
  - `AcceptanceSceneGroup = 'workspace' | 'viewing' | 'dialog' | 'feedback'`
  - `AcceptanceStateDefinition`
  - `ACCEPTANCE_STATE_DEFINITIONS`
  - `acceptanceDefinition(id: string): AcceptanceStateDefinition`

- [ ] **Step 1: Write the failing catalog equality test**

Create `acceptanceStateCatalog.test.ts` with a ledger parser local to the test and these assertions:

```ts
const expected = ledgerRows().map(({ id, wave, referenceState }) => ({
  id,
  wave,
  referenceState,
}))
const actual = ACCEPTANCE_STATE_DEFINITIONS.map(({ id, wave, referenceState }) => ({
  id,
  wave,
  referenceState,
}))

expect(actual).toHaveLength(89)
expect(new Set(actual.map(({ id }) => id)).size).toBe(89)
expect(actual).toEqual(expected)
expect(() => acceptanceDefinition('PRE-99')).toThrow('Unknown Viewer acceptance state: PRE-99')
```

The parser must accept only rows matching the existing ledger shape and fail when a row has no numeric wave or backticked reference state.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
pnpm --dir ui test -- src/acceptance/acceptanceStateCatalog.test.ts
```

Expected: FAIL because `acceptanceStateCatalog.ts` does not exist.

- [ ] **Step 3: Implement the complete typed catalog**

Add all IDs in ledger order with these fixed group boundaries:

```ts
export type AcceptanceSceneGroup = 'workspace' | 'viewing' | 'dialog' | 'feedback'

export interface AcceptanceStateDefinition {
  id: string
  wave: 1 | 2 | 3 | 4
  referenceState: string
  sceneGroup: AcceptanceSceneGroup
  components: readonly string[]
}
```

Use `workspace` for LAU/SID/STR/THU/OTH/SEA/FIL/MEN, `viewing` for RAD/PRE/COM/DOC/INF, `dialog` for DIA, and `feedback` for TAS/RES/A11Y. Copy each row's exact wave and reference state from the ledger. Populate `components` from the ledger's formal-component column, using source filenames without extensions, for example PRE-01 includes `ImagePreview`, PRE-07 includes `ImagePreview`, COM-04 includes `CompareWorkspace` and `CompareVirtualViewport`, and A11Y-05 includes all named responsive modules.

Implement lookup through a module-level `Map`; throw on unknown IDs rather than returning `undefined`.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run the Step 2 command. Expected: 89 definitions pass exact ledger equality.

- [ ] **Step 5: Make the existing migration coverage test consume the catalog**

Change `visualMigrationCoverage.test.ts` so its 89-ID assertion compares the audit, ledger and `ACCEPTANCE_STATE_DEFINITIONS`. Keep the existing automated-evidence row check intact.

- [ ] **Step 6: Run both coverage tests**

```bash
pnpm --dir ui test -- src/acceptance/acceptanceStateCatalog.test.ts src/visualMigrationCoverage.test.ts
```

Expected: PASS with 89 unique IDs in the same order.

- [ ] **Step 7: Commit the catalog contract**

```bash
git add ui/src/acceptance/acceptanceStateCatalog.ts ui/src/acceptance/acceptanceStateCatalog.test.ts ui/src/visualMigrationCoverage.test.ts
git commit -m "test: define Viewer visual acceptance catalog"
```

---

### Task 2: Build a Release-Isolated Acceptance Entry

**Files:**
- Create: `ui/visual-acceptance.html`
- Create: `ui/vite.visual-acceptance.config.ts`
- Create: `ui/src/acceptance/acceptanceRequest.ts`
- Create: `ui/src/acceptance/acceptanceRequest.test.ts`
- Create: `ui/src/acceptance/AcceptanceApp.tsx`
- Create: `ui/src/acceptance/AcceptanceApp.test.tsx`
- Create: `ui/src/acceptance/main.tsx`
- Modify: `ui/tsconfig.node.json`
- Modify: `package.json`
- Create: `scripts/viewer-visual-acceptance.test.mjs`

**Interfaces:**
- Consumes: `AcceptanceId`, exact query string, formal CSS entries.
- Produces:
  - `AcceptanceViewport = '1024x720' | '1440x900'`
  - `parseAcceptanceRequest(search: string)`
  - `<AcceptanceApp request sceneRegistry />`
  - `pnpm build:visual-acceptance`

- [ ] **Step 1: Write failing request parser tests**

```ts
expect(parseAcceptanceRequest('?id=PRE-01&viewport=1024x720')).toEqual({
  id: 'PRE-01',
  viewport: '1024x720',
  width: 1024,
  height: 720,
})
expect(() => parseAcceptanceRequest('?id=PRE-99&viewport=1024x720')).toThrow(
  'Unknown Viewer acceptance state: PRE-99',
)
expect(() => parseAcceptanceRequest('?id=PRE-01&viewport=1024')).toThrow(
  'Unsupported Viewer acceptance viewport: 1024',
)
expect(() => parseAcceptanceRequest('?viewport=1024x720')).toThrow(
  'Missing Viewer acceptance state ID',
)
```

- [ ] **Step 2: Run parser tests and verify RED**

```bash
pnpm --dir ui test -- src/acceptance/acceptanceRequest.test.ts
```

Expected: FAIL because the parser is missing.

- [ ] **Step 3: Implement strict request parsing**

Use `URLSearchParams`, the catalog lookup and an exact viewport record. Reject repeated `id` or `viewport` parameters and every unrecognized parameter so debug state cannot silently vary.

- [ ] **Step 4: Write failing acceptance root protocol tests**

Render `<AcceptanceApp>` with a one-entry test registry. Assert pending is present on the first commit, then after two mocked animation frames assert:

```ts
expect(root).toHaveAttribute('data-acceptance-id', 'PRE-01')
expect(root).toHaveAttribute('data-acceptance-viewport', '1024x720')
expect(root).toHaveAttribute('data-acceptance-status', 'ready')
expect(root).toHaveStyle({ width: '1024px', height: '720px' })
```

Add a throwing scene test that sets `data-acceptance-status="error"` and renders a plain diagnostic outside the screenshot root. It must not retry the scene.

- [ ] **Step 5: Run root protocol tests and verify RED**

```bash
pnpm --dir ui test -- src/acceptance/AcceptanceApp.test.tsx
```

Expected: FAIL because the app and status boundary are missing.

- [ ] **Step 6: Implement the separate HTML, React entry and Vite config**

`visual-acceptance.html` must contain only a UTF-8 head, viewport meta, `<div id="root"></div>` and `/src/acceptance/main.tsx`.

`main.tsx` imports these exact production styles in the same order as `ui/src/main.tsx`:

```ts
import '../styles/tokens.css'
import '../styles/primitives.css'
import '../styles/app.css'
import '../styles/filePreviewExtensions.css'
import '../styles/adaptiveOtherFilePanel.css'
```

The acceptance config uses the React plugin, `visual-acceptance.html` as the only Rollup input and `../target/viewer-visual-acceptance/site` as `outDir`. It must set `emptyOutDir: true` only for that exact target path. Add `vite.visual-acceptance.config.ts` to `tsconfig.node.json`.

Add root scripts:

```json
"dev:visual-acceptance": "pnpm --dir ui exec vite --config vite.visual-acceptance.config.ts --host 127.0.0.1",
"build:visual-acceptance": "pnpm --dir ui exec vite build --config vite.visual-acceptance.config.ts"
```

- [ ] **Step 7: Add and verify the release-isolation test**

In `viewer-visual-acceptance.test.mjs`, read `ui/src/main.tsx`, `ui/vite.config.ts`, `src-tauri/tauri.conf.json` and `src-tauri/capabilities/*.json`. Assert none reference `visual-acceptance`, `src/acceptance` or Playwright. Run `pnpm build:ui`, recursively inspect `ui/dist`, and assert no emitted filename or UTF-8 asset contains `data-acceptance-id` or `visual-acceptance.html`.

Run:

```bash
node --test --test-name-pattern='release isolation' scripts/viewer-visual-acceptance.test.mjs
pnpm build:ui
pnpm build:visual-acceptance
```

Expected: production build contains only the existing product entry; acceptance build contains `visual-acceptance.html` only under `target/viewer-visual-acceptance/site`.

- [ ] **Step 8: Commit the isolated entry**

```bash
git add package.json ui/visual-acceptance.html ui/vite.visual-acceptance.config.ts ui/tsconfig.node.json ui/src/acceptance/acceptanceRequest.ts ui/src/acceptance/acceptanceRequest.test.ts ui/src/acceptance/AcceptanceApp.tsx ui/src/acceptance/AcceptanceApp.test.tsx ui/src/acceptance/main.tsx scripts/viewer-visual-acceptance.test.mjs
git commit -m "feat: isolate Viewer visual acceptance entry"
```

---

### Task 3: Prove One Formal PRE-01 Scene End to End

**Files:**
- Create: `ui/src/acceptance/acceptanceFixtures.ts`
- Create: `ui/src/acceptance/acceptanceBridge.ts`
- Create: `ui/src/acceptance/acceptanceBridge.test.ts`
- Create: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Create: `ui/src/acceptance/scenes/viewingScenes.test.tsx`
- Modify: `ui/src/acceptance/AcceptanceApp.tsx`

**Interfaces:**
- Consumes: formal `ImagePreview`, `ViewerBridge`, existing product image assets or deterministic repository fixture files.
- Produces:
  - `ACCEPTANCE_FILES`
  - `imageRepresentation(file, kind)`
  - `createAcceptanceBridge(overrides?)`
  - `VIEWING_SCENES`

- [ ] **Step 1: Write the failing fail-fast bridge test**

Assert `createAcceptanceBridge()` satisfies every `ViewerBridge` member, returns deterministic settings and no-op unlisten functions, and rejects an unexpected business call with `Unexpected acceptance bridge call: executeFileCommand`. Override `requestImage` in a second test and assert the override result is returned.

- [ ] **Step 2: Run the bridge test and verify RED**

```bash
pnpm --dir ui test -- src/acceptance/acceptanceBridge.test.ts
```

Expected: FAIL because fixtures and bridge do not exist.

- [ ] **Step 3: Implement deterministic files and the complete bridge**

Create at least 30 image `BrowserFile` records named `商品-01.jpg` through `商品-30.jpg`, two text files, one unsupported file and three folders. Use stable entity IDs, relative paths, sizes, `modifiedNs`, marker data and image dimensions. Use real repository media URLs for visible images; do not generate placeholder rectangles.

The base bridge returns a writable `测试图` snapshot, standard density, stable tree/workspace/search/selection results and no-op listeners. Methods not intentionally used by a scene reject through one named helper. Scene overrides replace exact methods without type casts.

- [ ] **Step 4: Write the failing PRE-01 formal-component test**

Render `VIEWING_SCENES['PRE-01']` and assert:

```ts
expect(screen.getByRole('dialog', { name: /图片预览 商品-02\.jpg/ })).toBeVisible()
expect(screen.getByRole('button', { name: '适应窗口' })).toHaveAttribute('aria-pressed', 'true')
expect(screen.getByRole('button', { name: '按 100% 显示' })).toBeVisible()
expect(screen.getByRole('button', { name: '顺时针旋转' })).toBeVisible()
expect(screen.getByRole('button', { name: '关闭预览' })).toBeVisible()
expect(screen.getByRole('img', { name: '商品-02.jpg' })).toBeVisible()
```

Also assert the rendered dialog is the exported formal `ImagePreview` behavior by clicking zoom and observing the percentage change; do not assert a copied acceptance-only class.

- [ ] **Step 5: Run PRE-01 test and verify RED**

```bash
pnpm --dir ui test -- src/acceptance/scenes/viewingScenes.test.tsx
```

Expected: FAIL because the scene is missing.

- [ ] **Step 6: Implement PRE-01 with formal ImagePreview**

Use `<ImagePreview>` directly with `商品-02.jpg`, the 30 image records, stable real-image representations, no-op close/navigation callbacks and the existing production CSS. Register the scene in `AcceptanceApp`; the acceptance wrapper may size the viewport but must not style ImagePreview internals.

- [ ] **Step 7: Run the focused UI tests and typecheck**

```bash
pnpm --dir ui test -- src/acceptance/acceptanceBridge.test.ts src/acceptance/scenes/viewingScenes.test.tsx src/acceptance/AcceptanceApp.test.tsx
pnpm --dir ui check
```

Expected: PASS with no duplicate React key, act or accessibility warnings.

- [ ] **Step 8: Commit the first formal scene**

```bash
git add ui/src/acceptance/acceptanceFixtures.ts ui/src/acceptance/acceptanceBridge.ts ui/src/acceptance/acceptanceBridge.test.ts ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx ui/src/acceptance/AcceptanceApp.tsx
git commit -m "feat: render formal preview acceptance scene"
```

---

### Task 4: Add the One-Browser Playwright Capture Engine

**Files:**
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Create: `scripts/viewer-acceptance-evidence.mjs`
- Create: `scripts/viewer-visual-acceptance.mjs`
- Modify: `scripts/viewer-visual-acceptance.test.mjs`

**Interfaces:**
- Consumes: `ACCEPTANCE_STATE_DEFINITIONS`, built acceptance page, atlas hash-bound references.
- Produces:
  - `parseVisualAcceptanceCli(argv)`
  - `selectVisualAcceptanceIds(options, changedFiles?)`
  - `waitForAcceptanceReady(page, request, timeoutMs)`
  - `captureVisualAcceptanceState(context)`
  - `runVisualAcceptanceBatch(options, dependencies?)`
  - `pnpm accept:visual`

- [ ] **Step 1: Install the approved development dependency**

Run:

```bash
pnpm add --workspace-root --save-dev playwright@latest
pnpm exec playwright install chromium
```

Do not import Playwright from `ui/src`, Tauri or Rust.

- [ ] **Step 2: Write failing CLI selection tests**

Assert these exact behaviors:

```js
assert.deepEqual(parseVisualAcceptanceCli(['--id', 'PRE-01']).ids, ['PRE-01'])
assert.equal(parseVisualAcceptanceCli(['--wave', '2']).wave, 2)
assert.equal(parseVisualAcceptanceCli(['--all']).mode, 'all')
assert.equal(parseVisualAcceptanceCli(['--changed']).mode, 'changed')
assert.throws(() => parseVisualAcceptanceCli(['--all', '--wave', '2']), {
  code: 'CLI_ARGUMENT',
})
assert.throws(() => parseVisualAcceptanceCli(['--id', 'PRE-99']), {
  code: 'CLI_ARGUMENT',
})
```

- [ ] **Step 3: Run CLI tests and verify RED**

```bash
node --test --test-name-pattern='visual acceptance CLI' scripts/viewer-visual-acceptance.test.mjs
```

Expected: FAIL because the runner is missing.

- [ ] **Step 4: Implement bounded CLI and process dependency injection**

Use one selector only. Default viewports are both approved sizes; `--viewport` can narrow to one. Add `--output-root` only when its resolved path remains below `target/viewer-visual-acceptance`. Expose injected `startServer`, `launchBrowser`, `now` and filesystem dependencies so unit tests do not launch a real browser.

- [ ] **Step 5: Write failing browser-reuse and failure-isolation tests**

Use fake browser/page objects and assert one `launchBrowser` call for three IDs, one page reused within each viewport, two animation frames plus ready/font/image conditions, exact `setViewportSize`, and screenshot calls in stable catalog order. Make the second state return `data-acceptance-status="error"`; assert the third still runs, the summary is nonzero and no state is silently retried.

- [ ] **Step 6: Run reuse tests and verify RED**

```bash
node --test --test-name-pattern='reuses one browser|isolates state failure|bounded ready' scripts/viewer-visual-acceptance.test.mjs
```

Expected: FAIL because batch execution is incomplete.

- [ ] **Step 7: Implement Playwright lifecycle and ready protocol**

Build the acceptance page once, serve it on a random loopback port, launch `chromium` once, reuse a context/page per viewport and close page, context, browser and server in `finally`. Set a 10-second per-state ready timeout and a 20-minute full-batch budget. Capture console `error` and unhandled page errors. Use `page.addStyleTag` to disable only animation, transition and caret rendering; do not override production geometry or colors.

Add root scripts:

```json
"test:visual-acceptance": "node --test scripts/viewer-visual-acceptance.test.mjs",
"accept:visual": "node scripts/viewer-visual-acceptance.mjs"
```

- [ ] **Step 8: Extract shared lossless evidence functions**

Move `readRgbaPng`, `writeRgbaPng`, `combinePngEvidence` and SHA-256 helpers from `viewer-native-acceptance.mjs` into `viewer-acceptance-evidence.mjs`. Re-export or import them from the native controller without changing native evidence format. Add tests that combine two 2 × 2 fixtures into one 4 × 2 PNG and reject unequal heights.

- [ ] **Step 9: Run one real PRE-01 capture**

```bash
pnpm accept:visual -- --id PRE-01 --viewport 1024x720
```

Expected: one Chromium process, no Finder, exact 1024 × 720 `product.png`, current atlas `reference.png`, 2048 × 720 `combined.png`, and manifest verdict `pending-visual-review`.

- [ ] **Step 10: Open the current combined PRE-01 image and inspect it**

Inspect the generated combined image as one visual input. Record every visible mismatch in a local diagnostic note; do not fix product CSS in this infrastructure task and do not mark the state passed.

- [ ] **Step 11: Run focused gates and commit**

```bash
pnpm test:visual-acceptance
pnpm test:native-acceptance
pnpm --dir ui check
```

Then:

```bash
git add package.json pnpm-lock.yaml scripts/viewer-acceptance-evidence.mjs scripts/viewer-visual-acceptance.mjs scripts/viewer-visual-acceptance.test.mjs scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs
git commit -m "feat: batch Viewer visual evidence in one browser"
```

---

### Task 5: Complete All 27 Viewing States

**Files:**
- Modify: `ui/src/acceptance/acceptanceFixtures.ts`
- Modify: `ui/src/acceptance/scenes/viewingScenes.tsx`
- Modify: `ui/src/acceptance/scenes/viewingScenes.test.tsx`

**Interfaces:**
- Consumes: formal `RadialFileMenu`, `ImagePreview`, `CompareWorkspace`, `TextPreview`, `UnsupportedFilePreview`, `InfoOverlay`.
- Produces: scene entries for RAD-01–07, PRE-01–07, COM-01–04, DOC-01–07 and INF-01–02.

- [ ] **Step 1: Write the failing scene-completeness test**

Assert the viewing registry keys exactly equal the 27 Wave 2 catalog IDs. For each ID render the scene and require the following formal landmarks:

| IDs | Required landmark |
|---|---|
| RAD-01–07 | menu named `文件操作` and at least one formal menu item |
| PRE-01–07 | dialog matching `图片预览` and image, loading heading, error heading or navigation required by the row |
| COM-01–04 | region named `图片对比` with exactly 2, 3, 4 or virtualized-many source files |
| DOC-01–05 | dialog named `文本预览` with Markdown, plain text, encoding, truncation or two panes |
| DOC-06–07 | unsupported preview/state with `暂不支持预览` or `文件已不可用` |
| INF-01–02 | complementary inspector named `文件信息` with single or aggregate metadata |

- [ ] **Step 2: Run the viewing tests and verify RED**

```bash
pnpm --dir ui test -- src/acceptance/scenes/viewingScenes.test.tsx
```

Expected: FAIL listing the 26 unimplemented IDs after PRE-01.

- [ ] **Step 3: Implement RAD states with the formal radial model**

Use `buildRadialMenuModel` and formal `RadialFileMenu`. Set deterministic origins inside both viewports. RAD-02 uses the component's gesture-visible state, RAD-03 and RAD-04 open the formal secondary ring through component input, RAD-05 supplies one non-image/one-image compare-disabled context, RAD-06 supplies read-only context, and RAD-07 sets keyboard focus without executing an action. Do not reproduce the circle in acceptance CSS.

- [ ] **Step 4: Implement PRE states through formal ImagePreview interactions**

Use the same 30-file fixture. PRE-02 clicks `按 100% 显示`; PRE-03 clicks `放大` twice; PRE-04 clicks rotation once; PRE-05 supplies a never-settling image Promise while the acceptance wrapper itself becomes ready; PRE-06 rejects the requested active image with the production error message; PRE-07 opens item 2/30 so both arrows are enabled. Use Testing Library actions in scene setup only when the target component owns the state and has no public prop for it.

- [ ] **Step 5: Implement COM, DOC and INF states**

COM uses 2, 3, 4 and 20 real `BrowserFile` records with stable representations and the formal layout engine. DOC fixtures return sanitized Markdown, selectable plain text, an encoding-required rejection followed by the formal selector state, a `truncated: true` 10 MiB result, two independent panes, unsupported and unavailable states. INF uses one image for INF-01 and a `SelectionInfo` containing image/text/folder types for INF-02.

- [ ] **Step 6: Run the complete viewing tests**

```bash
pnpm --dir ui test -- src/acceptance/scenes/viewingScenes.test.tsx src/components/ImagePreview.test.tsx src/components/RadialFileMenu.test.tsx src/components/CompareWorkspace.test.tsx src/components/TextPreview.test.tsx src/components/InfoOverlay.test.tsx
```

Expected: all formal component and scene tests PASS.

- [ ] **Step 7: Run one bounded Wave 2 browser batch**

```bash
pnpm accept:visual -- --wave 2 --viewport 1024x720
```

Expected: 27 states captured in one browser run, no Finder, no Viewer native process restart, and a complete summary even if a state reports a visual diagnostic.

- [ ] **Step 8: Commit viewing coverage**

```bash
git add ui/src/acceptance/acceptanceFixtures.ts ui/src/acceptance/scenes/viewingScenes.tsx ui/src/acceptance/scenes/viewingScenes.test.tsx
git commit -m "test: cover Viewer viewing acceptance states"
```

---

### Task 6: Complete All 40 Workspace States

**Files:**
- Modify: `ui/src/acceptance/acceptanceFixtures.ts`
- Modify: `ui/src/acceptance/acceptanceBridge.ts`
- Create: `ui/src/acceptance/scenes/workspaceScenes.tsx`
- Create: `ui/src/acceptance/scenes/workspaceScenes.test.tsx`

**Interfaces:**
- Consumes: formal `App`, `EmptyProject`, `FolderTree`, `FolderOverview`, `ContentBrowser`, `SearchResults`, `SearchToolbar`, workspace menus and read-only banner.
- Produces: LAU-01–09, SID-01–04, STR-01–05, THU-01–07, OTH-01–03, SEA-01–05, FIL-01–04 and MEN-01–03.

- [ ] **Step 1: Write the failing Wave 1 completeness test**

Assert the registry keys exactly equal all 40 Wave 1 IDs and render each at both widths in jsdom with stable `ResizeObserver` geometry. Require the row-specific landmark from the ledger: launch feedback, sidebar state, overview/content mode, density/selection state, other-file panel/drag state, search state, filter popover or workspace menu.

- [ ] **Step 2: Run the workspace test and verify RED**

```bash
pnpm --dir ui test -- src/acceptance/scenes/workspaceScenes.test.tsx
```

Expected: FAIL because the registry is missing.

- [ ] **Step 3: Implement LAU and SID states**

Use formal `EmptyProject` for no-project, drag-valid, drag-invalid, opening, scan, thumbnail, permission, error and recovery surfaces. Use formal `App` plus the acceptance bridge for expanded, 320 px resized, collapsed and drag-target sidebars. Expose transient promises/events through bridge overrides; do not add sleep.

- [ ] **Step 4: Implement STR, THU and OTH states**

Use formal folder/workspace records and existing selection APIs. Provide compact, standard and large densities; zero, one, three and keyboard-focused selections; collapsed and expanded three-file other panels; and the formal three-item organization drag over `目标/Destination`. Acceptance-only code supplies input events and fixture data, not duplicate card markup.

- [ ] **Step 5: Implement SEA, FIL and MEN states**

Use formal search page fixtures for grouped, flat, indexing, page two and empty results. Open the real SearchToolbar popover with zero/one/four/advanced conditions. Open formal view, more and read-only menus through their public callbacks/props, retaining production focus and disabled semantics.

- [ ] **Step 6: Run workspace and affected product tests**

```bash
pnpm --dir ui test -- src/acceptance/scenes/workspaceScenes.test.tsx src/components/EmptyProject.test.tsx src/components/ContentBrowser.test.tsx src/components/SearchResults.test.tsx src/components/SearchToolbar.test.tsx src/App.test.tsx
```

Expected: 40 acceptance scenes and existing product tests PASS without warnings.

- [ ] **Step 7: Run one bounded Wave 1 browser batch**

```bash
pnpm accept:visual -- --wave 1 --viewport 1024x720
```

Expected: 40 product captures in one browser process, no Finder or native Viewer automation.

- [ ] **Step 8: Commit workspace coverage**

```bash
git add ui/src/acceptance/acceptanceFixtures.ts ui/src/acceptance/acceptanceBridge.ts ui/src/acceptance/scenes/workspaceScenes.tsx ui/src/acceptance/scenes/workspaceScenes.test.tsx
git commit -m "test: cover Viewer workspace acceptance states"
```

---

### Task 7: Complete Dialog, Feedback and Accessibility States

**Files:**
- Modify: `ui/src/acceptance/acceptanceFixtures.ts`
- Create: `ui/src/acceptance/scenes/dialogScenes.tsx`
- Create: `ui/src/acceptance/scenes/dialogScenes.test.tsx`
- Create: `ui/src/acceptance/scenes/feedbackScenes.tsx`
- Create: `ui/src/acceptance/scenes/feedbackScenes.test.tsx`

**Interfaces:**
- Produces: DIA-01–07, TAS-01–05, RES-01–05 and A11Y-01–05.

- [ ] **Step 1: Write failing Wave 3/4 completeness tests**

Assert dialog registry equals seven DIA IDs and feedback registry equals TAS/RES/A11Y IDs. Require named formal dialogs, task states, result/notice/banner states and accessibility mode attributes. Assert no scene contains an acceptance-only replacement button, dialog, progress bar or inspector.

- [ ] **Step 2: Run the tests and verify RED**

```bash
pnpm --dir ui test -- src/acceptance/scenes/dialogScenes.test.tsx src/acceptance/scenes/feedbackScenes.test.tsx
```

Expected: FAIL with all 22 IDs absent.

- [ ] **Step 3: Implement the seven formal dialog scenes**

Render `SettingsDialog`, `RenameDialog`, `BatchRenameDialog`, `DestinationDialog`, conflict-stage destination, `TrashConfirmation` and `CloseOperationDialog` with the exact stable fixtures named by the ledger. Use real validation/preflight/result props and public callbacks. DIA-03 supplies three selected files and a valid prefix; DIA-04 selects `目标/Destination`; DIA-05 returns a same-name conflict; DIA-07 supplies a running multi-file operation.

- [ ] **Step 4: Implement task and result scenes**

Render formal `TaskBar`, `OperationResults`, `GlobalNoticeStack`, `ViewerLocalFeedback` and `ReadOnlyBanner`. Supply running, success-before-dismissal, retained failure, cancelled and result-bearing tasks; operation results, recoverable context notice, local preview error, read-only capability strip and interrupted-operation recovery.

- [ ] **Step 5: Implement accessibility scenes without fake platform claims**

A11Y-01 and A11Y-02 render the formal focused/restore states needed for visual inspection and retain existing keyboard unit evidence. A11Y-03 applies `prefers-reduced-motion` through Playwright media emulation. A11Y-04 applies browser `forced-colors` automated coverage and labels native Increase Contrast evidence separately. A11Y-05 sets the acceptance page to 200% browser zoom/minimum layout state and asserts all named controls remain present. Do not label browser forced colors as Windows-native or macOS-native proof.

- [ ] **Step 6: Run all scene and formal component tests**

```bash
pnpm --dir ui test -- src/acceptance/scenes/dialogScenes.test.tsx src/acceptance/scenes/feedbackScenes.test.tsx src/components/SettingsDialog.test.tsx src/components/TaskBar.test.tsx src/components/OperationResults.test.tsx src/styles/visualAccessibility.test.ts
```

Expected: 22 scenes and affected product tests PASS.

- [ ] **Step 7: Run Wave 3 and Wave 4 at 1024**

```bash
pnpm accept:visual -- --wave 3 --viewport 1024x720
pnpm accept:visual -- --wave 4 --viewport 1024x720
```

Expected: 17 and 5 state summaries, each using one browser process per command and no native automation.

- [ ] **Step 8: Commit the remaining scenes**

```bash
git add ui/src/acceptance/acceptanceFixtures.ts ui/src/acceptance/scenes/dialogScenes.tsx ui/src/acceptance/scenes/dialogScenes.test.tsx ui/src/acceptance/scenes/feedbackScenes.tsx ui/src/acceptance/scenes/feedbackScenes.test.tsx
git commit -m "test: complete Viewer visual acceptance scenes"
```

---

### Task 8: Add Changed-State Selection and Narrow Native Acceptance

**Files:**
- Modify: `scripts/viewer-visual-acceptance.mjs`
- Modify: `scripts/viewer-visual-acceptance.test.mjs`
- Modify: `scripts/viewer-native-acceptance.mjs`
- Modify: `scripts/viewer-native-acceptance.test.mjs`
- Create: `docs/reviews/viewer-native-smoke-matrix.md`
- Modify: `package.json`

**Interfaces:**
- Produces:
  - `affectedAcceptanceIds(changedFiles, definitions)`
  - `NATIVE_SMOKE_IDS`
  - `pnpm accept:native-smoke`

- [ ] **Step 1: Write failing changed-state tests**

Assert:

- `ui/src/components/ImagePreview.tsx` selects PRE-01–PRE-07;
- `ui/src/components/RadialFileMenu.tsx` selects RAD-01–RAD-07 and A11Y keyboard/restore states;
- `ui/src/styles/tokens.css`, `primitives.css` or `app.css` selects all 89 states;
- `ui/src/components/ui/ViewerButton.tsx` selects every catalog row whose `components` contains a dependent formal surface, with fallback to all 89 when the dependency is ambiguous;
- docs-only changes select zero states unless the atlas, ledger or acceptance spec changed;
- an unknown file below `ui/src/components` safely selects all 89.

- [ ] **Step 2: Run changed-state tests and verify RED**

```bash
node --test --test-name-pattern='changed-state' scripts/viewer-visual-acceptance.test.mjs
```

Expected: FAIL because the selector is missing.

- [ ] **Step 3: Implement conservative changed-state mapping**

Read changed paths from `git diff --name-only <base>...HEAD` or explicit test input. Use catalog `components` for named files and fixed global-style/build lists. Never return a partial guess for unknown production component or style changes.

- [ ] **Step 4: Write the failing native-smoke matrix test**

Define these 15 stable smoke IDs in the review document and controller: `launch-single-instance`, `launch-empty`, `open-project-picker`, `workspace-scan`, `sidebar-window`, `thumbnail-keyboard`, `radial-native`, `preview-native`, `compare-native`, `text-native`, `info-shortcut`, `rename-dialog-native`, `finder-drop-valid`, `finder-drop-invalid`, `close-project-native`.

Assert the controller exposes exactly those IDs under `--smoke`, does not expand them to 89 catalog states, reuses one fixture/project session after `open-project-picker`, and resets only when a destructive fixture mutation requires it.

- [ ] **Step 5: Run native matrix tests and verify RED**

```bash
pnpm test:native-acceptance -- --test-name-pattern='native smoke matrix'
```

Expected: FAIL because `--smoke` and the matrix are missing.

- [ ] **Step 6: Implement the bounded native smoke command**

Keep existing PID, path, window, coordinate and evidence guards. Add `--smoke` as a separate selector. Reuse one copied fixture and opened project across non-destructive journeys; only Finder picker/drop journeys invoke Finder. Leave legacy per-ID commands available for diagnostics but remove `--all` from the migration completion command and documentation.

Add:

```json
"accept:native-smoke": "node scripts/viewer-native-acceptance.mjs --smoke"
```

- [ ] **Step 7: Run controller unit tests without driving the UI**

```bash
pnpm test:native-acceptance
pnpm test:visual-acceptance
```

Expected: all protocol, safety, visual runner and smoke-matrix tests PASS.

- [ ] **Step 8: Commit changed selection and native narrowing**

```bash
git add package.json scripts/viewer-visual-acceptance.mjs scripts/viewer-visual-acceptance.test.mjs scripts/viewer-native-acceptance.mjs scripts/viewer-native-acceptance.test.mjs docs/reviews/viewer-native-smoke-matrix.md
git commit -m "test: tier Viewer visual and native acceptance"
```

---

### Task 9: Run the New Gates and Resume Product Visual Correction

**Files:**
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`
- Modify: `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md`
- Modify: `design-qa.md`
- Modify: product component/style files only after a failing component or visual contract test names the mismatch.

**Interfaces:**
- Consumes: complete 89-state catalog, dual-viewport browser evidence, 15 native smoke journeys.
- Produces: current-commit visual evidence, native smoke report and truthful migration ledger.

- [ ] **Step 1: Run fast automated gates**

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm test:visual-acceptance
pnpm test:native-acceptance
```

Expected: all commands exit 0 with no unexpected warnings.

- [ ] **Step 2: Create a fresh evidence commit boundary**

Ensure the worktree is clean and create a small metadata commit only if the last implementation commit does not already represent all product and acceptance source. Evidence must bind to this exact clean HEAD; do not modify source between the two viewport batches.

- [ ] **Step 3: Run the complete fast dual-viewport batch once**

```bash
pnpm accept:visual -- --all
```

Expected: 178 product screenshots and 178 combined images from one bounded run, two viewport contexts, no Finder, no native Viewer restart, no missing ID and no ready timeout.

- [ ] **Step 4: Inspect joint evidence wave by wave**

Open the current combined images in these bounded groups: Wave 1 (40), Wave 2 (27), Wave 3 (17), Wave 4 (5). For each image record typography, spacing, alignment, radius, border, opacity, crop, control hierarchy, selection/focus and compact-viewport defects. Mark no state passed until its combined image is inspected.

- [ ] **Step 5: Correct product defects with a new TDD cycle**

For each P0/P1/P2 mismatch, first add or change the affected formal component/style test so it fails for the approved rule, run the focused test to observe RED, change only formal product code, rerun GREEN, then run `pnpm accept:visual -- --changed` on a new clean commit. Never patch acceptance JSX/CSS to hide a product mismatch.

- [ ] **Step 6: Run the 15 native journeys once after browser visual closure**

Start exactly one current-worktree Viewer process at the approved viewport and run:

```bash
pnpm accept:native-smoke -- --viewport 1024x720
pnpm accept:native-smoke -- --viewport 1440x900
```

Expected: both bounded smoke runs pass without recreating the project for each journey. Stop immediately for a safety/precondition failure; retain diagnostics for a state failure.

- [ ] **Step 7: Update the ledger and verification documents**

Record browser visual evidence and native smoke evidence in separate fields/sections. Mark the obsolete 89-state native columns as historical rather than pretending browser evidence is native. Every current row must name the current commit, both combined paths and visual verdict. State explicitly that Windows-native behavior remains pending until Windows development starts.

- [ ] **Step 8: Run final repository gates**

```bash
pnpm verify
git status --short
```

Expected: verify exits 0 and the worktree contains only the intended documentation updates before commit; `target/` evidence remains ignored.

- [ ] **Step 9: Commit the truthful final acceptance record**

```bash
git add docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md design-qa.md
git commit -m "docs: record tiered Viewer visual acceptance"
```

## Plan Self-Review

- Spec coverage: every requirement in the approved tiered design maps to Tasks 1–9.
- Release isolation: Task 2 tests both source references and built output.
- State coverage: Tasks 5–7 cover 27 + 40 + 22 = 89 states.
- Capture efficiency: Task 4 launches one browser per command and Task 9 runs one complete dual-viewport batch.
- Native scope: Task 8 names exactly 15 desktop journeys and Task 9 runs them only after browser visual closure.
- Evidence truth: browser and native evidence remain separately labeled; combined visual inspection is still mandatory.
- Safety: target paths remain bounded, generated evidence remains ignored and native PID/path/window guards are preserved.
