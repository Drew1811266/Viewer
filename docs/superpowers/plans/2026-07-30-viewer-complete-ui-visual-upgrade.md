# Viewer Complete UI Visual Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the approved cross-platform, predominantly white Viewer visual system to every current UI surface without changing the established product workflows or backend behavior.

**Architecture:** Introduce one semantic token layer, then migrate the existing React components in dependency order: shell and menus, browsing and search, previews, operation surfaces, and lifecycle states. Keep controller and Tauri boundaries intact; presentation-only coordination is expressed through focused components and typed UI command props, with contract tests protecting the final cascade and interaction tests protecting existing behavior.

**Tech Stack:** React 19, TypeScript 6, CSS, Vitest 4, Testing Library, Biome, Vite 8, pnpm 10, Tauri 2

## Global Constraints

- Treat `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md` as the authoritative visual specification.
- Work from an isolated git worktree created with `superpowers:using-git-worktrees`; the main workspace contains unrelated `tests/fixtures/images/.viewer/` files that must not be staged.
- Do not modify Rust crates, Tauri commands, bridge signatures, persisted project state, filesystem semantics, image safety budgets, search semantics, comparison limits, or file-operation safety behavior.
- Do not add a component library, icon dependency, theme framework, or runtime dependency.
- Viewer-owned surfaces remain light even when the operating system uses a dark appearance; delete conflicting Viewer dark-scheme overrides rather than adding a theme switch.
- Preserve native macOS and future Windows window chrome, file pickers, permission UI, shortcuts, and Trash/Recycle Bin terminology.
- Use the exact approved accent `#5869CF`, four-pixel spacing grid, and semantic token values from the design specification.
- Preserve roles, accessible names, focus traps, focus restoration, Escape handling, keyboard shortcuts, selection behavior, and reduced-motion support.
- Keep the application runnable after every task and stage only the files listed for that task.
- Use TDD: verify each new focused test fails for the expected reason before implementation, then run the focused and neighboring regression suites before committing.

---

## File Map

### New files

- `ui/src/styles/tokens.css` — application-wide semantic colors, typography, spacing, radius, shadow, focus, and motion tokens.
- `ui/src/styles/tokens.test.ts` — exact token values, light-only color-scheme, and no-dark-theme contract.
- `ui/src/components/WorkspaceViewMenu.tsx` — contextual `视图` menu for search layout, descendant aggregation, and select-all scope.
- `ui/src/components/WorkspaceViewMenu.test.tsx` — contextual view-menu behavior and accessible state.
- `ui/src/components/WorkspaceMoreMenu.tsx` — `更多` menu for settings, project access, permission, reselection, and close.
- `ui/src/components/WorkspaceMoreMenu.test.tsx` — command routing, read-only content, closing state, and focus behavior.
- `ui/src/components/GlobalNoticeStack.tsx` — non-modal top-right global recovery and error notices.
- `ui/src/components/GlobalNoticeStack.test.tsx` — notice ordering, roles, actions, and dismissal.
- `ui/src/components/FileContextMenu.tsx` — conventional secondary-click/Control-click fallback that consumes the radial action model.
- `ui/src/components/FileContextMenu.test.tsx` — fallback grouping, checked states, disabled reasons, destructive placement, Escape, and focus restoration.
- `ui/src/components/WorkspaceLoadingState.tsx` — stable sidebar, folder-band, and content skeletons used while project data is unavailable.
- `ui/src/components/WorkspaceLoadingState.test.tsx` — skeleton semantics, final-layout geometry hooks, and reduced-motion contract.
- `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md` — final state matrix, viewport checks, visual comparisons, and command evidence.

### Existing files changed by responsibility

- `ui/src/main.tsx` — import the token layer before component styles.
- `ui/src/App.tsx`, `ui/src/App.test.tsx` — aligned shell, contextual menus, view-command coordination, notice routing, task/results placement, and loading-state composition.
- `ui/src/styles/app.css`, `ui/src/styles/app.test.ts` — shell, toolbar, browsing, search, comparison, menu, dialog, feedback, and lifecycle styles plus cascade contracts.
- `ui/src/styles/filePreviewExtensions.css` — text, Markdown, unsupported-file, and file-info presentation.
- `ui/src/styles/adaptiveOtherFilePanel.css`, `ui/src/styles/adaptiveOtherFilePanel.test.ts` — subordinate expandable other-file panel.
- `ui/src/components/SearchToolbar.tsx`, `ui/src/components/SearchToolbar.test.tsx` — search field and compact filter popover only; contextual view moves out.
- `ui/src/components/FolderOverview.tsx`, `ui/src/components/FolderOverview.test.tsx` — remove repeated workspace heading and retain quiet filmstrip bands.
- `ui/src/components/FolderFilmstripRow.tsx`, `ui/src/components/FolderFilmstripRow.test.tsx` — stable band identity, skeleton, empty, and local error states.
- `ui/src/components/FolderTree.tsx`, `ui/src/components/FolderTree.test.tsx` — compact navigation and drop-target visual hooks.
- `ui/src/components/ContentBrowser.tsx`, `ui/src/components/ContentBrowser.test.tsx` — direct-to-grid content, typed global view commands, selection summary, and drag feedback.
- `ui/src/components/contentBrowser/ImageCell.tsx` — restrained selected/review/favorite presentation hooks.
- `ui/src/components/contentBrowser/OtherFilePanel.tsx`, `ui/src/components/contentBrowser/OtherFilePanel.test.tsx` — bottom expandable panel and compact rows.
- `ui/src/components/contentBrowser/SelectAllChoicePanel.tsx`, `ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx` — remove after its choices move into `WorkspaceViewMenu`.
- `ui/src/components/SearchResults.tsx`, `ui/src/components/SearchResults.test.tsx` — compact grouped/flat result rows, progress, snippets, and pagination.
- `ui/src/components/ImagePreview.tsx`, `ui/src/components/ImagePreview.test.tsx` — grouped 52 px preview toolbar and floating navigation.
- `ui/src/components/CompareWorkspace.tsx`, `ui/src/components/CompareWorkspace.test.tsx` — shared preview chrome and restrained pane boundaries.
- `ui/src/components/ComparePane.tsx`, `ui/src/components/ComparePane.test.tsx` — active-pane, marker, and removal presentation.
- `ui/src/components/TextPreview.tsx`, `ui/src/components/TextPreviewPane.tsx`, `ui/src/components/TextPreview.test.tsx` — shared text preview shell and two-pane reading layout.
- `ui/src/components/UnsupportedFilePreview.tsx`, `ui/src/components/UnsupportedFileState.tsx`, `ui/src/components/UnsupportedFilePreview.test.tsx` — concise unsupported/unavailable state.
- `ui/src/components/InfoOverlay.tsx`, `ui/src/components/InfoOverlay.test.tsx` — grouped trailing non-modal inspector.
- `ui/src/components/RadialFileMenu.tsx`, `ui/src/components/RadialFileMenu.test.tsx` — preserve current geometry and behavior while applying global tokens.
- `ui/src/components/ModalSheet.tsx`, `ui/src/components/ModalSheet.test.tsx` — common dialog chrome and structural slots.
- `ui/src/components/DestinationDialog.tsx`, `ui/src/components/DestinationDialog.test.tsx` — two-column destination and conflict layout.
- `ui/src/components/BatchRenameDialog.tsx`, `ui/src/components/BatchRenameDialog.test.tsx` — rule region, summary, and full preview.
- `ui/src/components/CloseOperationDialog.tsx`, `ui/src/components/CloseOperationDialog.test.tsx` — safe/default/destructive action hierarchy.
- `ui/src/components/RenameDialog.tsx`, `ui/src/components/TrashConfirmation.tsx`, `ui/src/components/SettingsDialog.tsx` and their existing tests — shared modal primitives only.
- `ui/src/components/TaskBar.tsx`, `ui/src/components/TaskBar.test.tsx` — bottom-right floating task stack.
- `ui/src/components/OperationResults.tsx`, `ui/src/components/OperationResults.test.tsx` — trailing non-modal result inspector.
- `ui/src/components/ReadOnlyBanner.tsx`, `ui/src/components/ReadOnlyBanner.test.tsx` — compact 38 px warning-soft strip.
- `ui/src/components/EmptyProject.tsx`, `ui/src/components/EmptyProject.test.tsx` — three-element resting state plus transient drag/open/error states.
- `ui/src/components/AspectThumbnail.tsx`, `ui/src/components/AspectThumbnail.test.tsx` — aspect-preserving skeleton/failure classes.

---

### Task 1: Establish the light-only semantic token layer

**Files:**
- Create: `ui/src/styles/tokens.css`
- Create: `ui/src/styles/tokens.test.ts`
- Modify: `ui/src/main.tsx:1-6`
- Modify: `ui/src/styles/app.css:1-32,1244-1304,1948-2068`
- Modify: `ui/src/styles/app.test.ts:1-76,190-215,286-305,520-548`

**Interfaces:**
- Consumes: no runtime interface; values come directly from the approved design specification.
- Produces: CSS custom properties prefixed `--viewer-`, imported before all component styles.
- Produces: compatibility aliases for existing `--preview-*` variables so preview migration can occur task-by-task.

- [ ] **Step 1: Write the failing token contract**

Create `ui/src/styles/tokens.test.ts`:

```ts
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const tokens = readFileSync('src/styles/tokens.css', 'utf8')
const appCss = readFileSync('src/styles/app.css', 'utf8')

function value(name: string): string {
  const match = tokens.match(new RegExp(`${name}:\\s*([^;]+);`))
  if (match?.[1] === undefined) throw new Error(`Missing token ${name}`)
  return match[1].trim()
}

describe('Viewer visual tokens', () => {
  it('defines the approved light palette and geometry', () => {
    expect(value('--viewer-canvas')).toBe('#f3f3f1')
    expect(value('--viewer-application')).toBe('#fafaf8')
    expect(value('--viewer-surface')).toBe('#ffffff')
    expect(value('--viewer-sidebar')).toBe('#eff0ef')
    expect(value('--viewer-soft-surface')).toBe('#f5f5f3')
    expect(value('--viewer-border')).toBe('#dedfdd')
    expect(value('--viewer-text')).toBe('#23262c')
    expect(value('--viewer-text-secondary')).toBe('#6b7077')
    expect(value('--viewer-text-tertiary')).toBe('#8a8e94')
    expect(value('--viewer-accent')).toBe('#5869cf')
    expect(value('--viewer-accent-soft')).toBe('#eceeff')
    expect(value('--viewer-radius-control')).toBe('8px')
    expect(value('--viewer-radius-popover')).toBe('12px')
    expect(value('--viewer-radius-dialog')).toBe('14px')
    expect(value('--viewer-motion-fast')).toBe('120ms')
    expect(value('--viewer-motion-standard')).toBe('160ms')
    expect(value('--viewer-motion-slow')).toBe('200ms')
  })

  it('keeps Viewer-owned surfaces light without a dark-theme override', () => {
    expect(tokens).toContain('color-scheme: light')
    expect(tokens).not.toContain('prefers-color-scheme: dark')
    expect(appCss).not.toContain('@media (prefers-color-scheme: dark)')
    expect(appCss).toContain('@media (prefers-reduced-motion: reduce)')
  })
})
```

- [ ] **Step 2: Run the token test and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/tokens.test.ts
```

Expected: FAIL because `src/styles/tokens.css` does not exist.

- [ ] **Step 3: Create the token stylesheet**

Create `ui/src/styles/tokens.css` with this complete root contract:

```css
:root {
  color-scheme: light;
  --viewer-canvas: #f3f3f1;
  --viewer-application: #fafaf8;
  --viewer-surface: #ffffff;
  --viewer-sidebar: #eff0ef;
  --viewer-soft-surface: #f5f5f3;
  --viewer-border: #dedfdd;
  --viewer-text: #23262c;
  --viewer-text-secondary: #6b7077;
  --viewer-text-tertiary: #8a8e94;
  --viewer-accent: #5869cf;
  --viewer-accent-soft: #eceeff;
  --viewer-success: #397158;
  --viewer-success-soft: #eff7f2;
  --viewer-warning: #76591d;
  --viewer-warning-soft: #faf6eb;
  --viewer-danger: #b7463f;
  --viewer-danger-soft: #fff7f6;
  --viewer-space-1: 4px;
  --viewer-space-2: 8px;
  --viewer-space-3: 12px;
  --viewer-space-4: 16px;
  --viewer-space-6: 24px;
  --viewer-space-8: 32px;
  --viewer-radius-row: 6px;
  --viewer-radius-control: 8px;
  --viewer-radius-popover: 12px;
  --viewer-radius-dialog: 14px;
  --viewer-shadow-popover: 0 18px 48px rgb(31 35 42 / 17%);
  --viewer-shadow-dialog: 0 22px 60px rgb(31 35 42 / 19%);
  --viewer-shadow-image: 0 8px 28px rgb(34 42 53 / 14%);
  --viewer-focus-ring: 0 0 0 2px var(--viewer-accent);
  --viewer-motion-fast: 120ms;
  --viewer-motion-standard: 160ms;
  --viewer-motion-slow: 200ms;
  --viewer-easing: cubic-bezier(.2, 0, 0, 1);
  --preview-surface: var(--viewer-application);
  --preview-chrome: var(--viewer-surface);
  --preview-stage: #f0f1ef;
  --preview-document-surface: var(--viewer-surface);
  --preview-panel-surface: var(--viewer-surface);
  --preview-text: var(--viewer-text);
  --preview-muted: var(--viewer-text-secondary);
  --preview-border: var(--viewer-border);
  --preview-control-border: var(--viewer-border);
  --preview-control-hover-border: #c8cac7;
  --preview-control-surface: var(--viewer-surface);
  --preview-control-hover-surface: var(--viewer-soft-surface);
  --preview-accent: var(--viewer-accent);
  --preview-accent-surface: var(--viewer-accent-soft);
  --preview-accent-text: #4152b4;
  --preview-danger: var(--viewer-danger);
  --preview-danger-surface: var(--viewer-danger-soft);
  --preview-image-shadow: var(--viewer-shadow-image);
}
```

- [ ] **Step 4: Import tokens first and remove the obsolete root palette**

Update `ui/src/main.tsx` imports to:

```ts
import './styles/tokens.css'
import './styles/app.css'
import './styles/filePreviewExtensions.css'
import './styles/adaptiveOtherFilePanel.css'
```

In `ui/src/styles/app.css`, keep the font and document-level declarations but replace literal root colors with:

```css
:root {
  color: var(--viewer-text);
  background: var(--viewer-application);
  font-family:
    Inter,
    "SF Pro Text",
    "Segoe UI Variable Text",
    "PingFang SC",
    "Microsoft YaHei UI",
    sans-serif;
  font-synthesis: none;
}
```

Delete both `@media (prefers-color-scheme: dark)` blocks. Replace the four dark-mode-specific tests in `app.test.ts` with light-token cascade assertions using `winningDeclaration`.

Add motion only to state changes and floating-surface entrances:

```css
button,
input,
select,
summary,
[role="menuitem"],
[role="menuitemcheckbox"] {
  transition:
    color var(--viewer-motion-fast) var(--viewer-easing),
    border-color var(--viewer-motion-fast) var(--viewer-easing),
    background-color var(--viewer-motion-fast) var(--viewer-easing);
}
.search-options-popover,
.workspace-menu-popover,
.file-context-menu {
  animation: viewer-popover-in var(--viewer-motion-standard) var(--viewer-easing);
}
.modal-sheet,
.preview-overlay {
  animation: viewer-fade-in var(--viewer-motion-slow) var(--viewer-easing);
}
@keyframes viewer-popover-in {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes viewer-fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
@media (prefers-reduced-motion: reduce) {
  button,
  input,
  select,
  summary,
  [role="menuitem"],
  [role="menuitemcheckbox"],
  .search-options-popover,
  .workspace-menu-popover,
  .file-context-menu,
  .modal-sheet,
  .preview-overlay {
    transition: none;
    animation: none;
  }
}
```

Do not animate image-grid layout, folder bands, scrolling, zoom, pan, or comparison geometry.

- [ ] **Step 5: Run token and style contracts**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/tokens.test.ts src/styles/app.test.ts
```

Expected: PASS with no dark-mode contract remaining.

- [ ] **Step 6: Run formatting and type checks**

Run:

```bash
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 7: Commit the foundation**

```bash
git add ui/src/main.tsx ui/src/styles/tokens.css ui/src/styles/tokens.test.ts ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: add Viewer visual token foundation"
```

---

### Task 2: Align the application shell and consolidate the top toolbar

**Files:**
- Create: `ui/src/components/WorkspaceViewMenu.tsx`
- Create: `ui/src/components/WorkspaceViewMenu.test.tsx`
- Create: `ui/src/components/WorkspaceMoreMenu.tsx`
- Create: `ui/src/components/WorkspaceMoreMenu.test.tsx`
- Modify: `ui/src/components/SearchToolbar.tsx:1-286`
- Modify: `ui/src/components/SearchToolbar.test.tsx:18-190`
- Modify: `ui/src/App.tsx:76-145,862-928,995-1062`
- Modify: `ui/src/App.test.tsx:141-245`
- Modify: `ui/src/styles/app.css:49-230,397-620`
- Modify: `ui/src/styles/app.test.ts:210-365`

**Interfaces:**
- Consumes: `ProjectAccess`, `SearchLayout`, existing App callbacks for settings, permission settings, directory reselection, close, result layout, descendant aggregation, and return-to-folder.
- Produces:

```ts
export type WorkspaceViewContext =
  | { kind: 'search'; layout: SearchLayout; onLayoutChange(layout: SearchLayout): void }
  | {
      kind: 'content'
      showingAggregate: boolean
      selectAllRequest: SelectAllRequest
      onSelectAll(scope: SelectAllScope): void
      onShowAllDescendants(): void
      onReturnToFolder(): void
    }
  | { kind: 'category'; onShowAllDescendants(): void }
  | { kind: 'none' }

export interface WorkspaceMoreMenuProps {
  access: ProjectAccess
  closing: boolean
  onOpenSettings(): void
  onOpenPermissionSettings(): void
  onReselectProject(): void
  onCloseProject(): void
}
```

- [ ] **Step 1: Write failing shell and menu tests**

Add this contract to `WorkspaceMoreMenu.test.tsx`:

```tsx
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import WorkspaceMoreMenu from './WorkspaceMoreMenu'

describe('WorkspaceMoreMenu', () => {
  it('contains settings and project commands in one named menu', () => {
    const openSettings = vi.fn()
    const closeProject = vi.fn()
    render(
      <WorkspaceMoreMenu
        access="read_write"
        closing={false}
        onOpenSettings={openSettings}
        onOpenPermissionSettings={vi.fn()}
        onReselectProject={vi.fn()}
        onCloseProject={closeProject}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '更多' }))
    fireEvent.click(screen.getByRole('button', { name: '软件设置' }))
    expect(openSettings).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '更多' }))
    fireEvent.click(screen.getByRole('button', { name: '关闭项目' }))
    expect(closeProject).toHaveBeenCalledOnce()
  })
})
```

Add an App assertion after opening a project:

```tsx
const toolbar = screen.getByRole('toolbar', { name: 'Viewer 工具栏' })
expect(within(toolbar).getByRole('button', { name: /^筛选/ })).toBeVisible()
expect(within(toolbar).getByRole('button', { name: '视图' })).toBeVisible()
expect(within(toolbar).getByRole('button', { name: '更多' })).toBeVisible()
expect(within(toolbar).queryByRole('button', { name: '软件设置' })).not.toBeInTheDocument()
expect(within(toolbar).queryByRole('button', { name: '项目菜单' })).not.toBeInTheDocument()
```

- [ ] **Step 2: Run the focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/WorkspaceMoreMenu.test.tsx \
  src/components/SearchToolbar.test.tsx \
  src/App.test.tsx -t "settings and project commands|筛选|persistent"
```

Expected: FAIL because the new menu does not exist and the header still renders separate settings and project controls.

- [ ] **Step 3: Implement the contextual view and more menus**

Implement both menus as controlled `<details>` components with button-role summaries, explicit `aria-expanded`, Escape close, and focus restoration. The `WorkspaceViewMenu` content must use this exact contextual mapping:

```tsx
{context.kind === 'search' && (
  <>
    <button aria-pressed={context.layout === 'grouped'} onClick={() => context.onLayoutChange('grouped')}>
      按文件夹分组
    </button>
    <button aria-pressed={context.layout === 'flat'} onClick={() => context.onLayoutChange('flat')}>
      展平结果
    </button>
  </>
)}
{context.kind === 'category' && (
  <button type="button" onClick={context.onShowAllDescendants}>显示全部后代文件</button>
)}
{context.kind === 'content' && (
  <>
    {context.showingAggregate ? (
      <button type="button" onClick={context.onReturnToFolder}>返回当前文件夹</button>
    ) : (
      <button type="button" onClick={context.onShowAllDescendants}>显示全部后代文件</button>
    )}
    {selectAllButtons(context.selectAllRequest, context.onSelectAll)}
  </>
)}
```

`selectAllButtons` returns no button for `none`, one direct button for a direct scope, and `全选图片` / `全选其它文件` / `全选全部文件` for a choice.
Use `.workspace-menu-popover` on both menu-content containers so they share the global floating
surface and motion contract.

The `WorkspaceMoreMenu` order is:

```tsx
<button type="button" onClick={onOpenSettings}>软件设置</button>
{access === 'read_only' && <p className="project-access-status">访问权限：只读</p>}
{access === 'read_only' && <button type="button" onClick={onOpenPermissionSettings}>权限设置</button>}
{access === 'read_only' && <button type="button" onClick={onReselectProject}>重新选择目录</button>}
<hr />
<button type="button" disabled={closing} onClick={onCloseProject}>
  {closing ? '正在关闭…' : '关闭项目'}
</button>
```

- [ ] **Step 4: Reduce SearchToolbar to search plus filter**

Remove `viewOpen` and the `search-view-panel` JSX from `SearchToolbar`. Rename the filter summary to `筛选`, compute `chips.length`, and render:

```tsx
<summary role="button" aria-label={chips.length > 0 ? `筛选，${chips.length} 项已启用` : '筛选'}>
  <span>筛选</span>
  {chips.length > 0 && <span className="filter-count">{chips.length}</span>}
</summary>
```

Wrap orientation, size, and time fields in:

```tsx
<details className="advanced-filter-group">
  <summary>高级条件</summary>
  <div className="advanced-filter-grid">
    <fieldset aria-label="方向">{orientationControls}</fieldset>
    <fieldset aria-label="像素尺寸">{dimensionControls}</fieldset>
    <fieldset aria-label="文件大小">{fileSizeControls}</fieldset>
    <fieldset aria-label="修改时间">{modifiedTimeControls}</fieldset>
  </div>
</details>
```

Extract the current orientation, dimension, file-size, and modification-time JSX into the four
named local constants shown above without changing their inputs or callbacks. Keep file type and
review state visible, retain every current control and callback, and place scope/sort above the
common filters.

- [ ] **Step 5: Align App header and remove duplicate workspace headings**

Use the existing `sidebarWidth` and collapse state as a CSS variable:

```tsx
<main
  className="viewer-shell"
  style={{ '--viewer-sidebar-width': `${sidebarCollapsed ? 44 : sidebarWidth}px` } as CSSProperties}
>
  <header className="workspace-header">
    <div className="project-identity"><h1>{state.project.displayName}</h1></div>
    <div className="workspace-header-main" role="toolbar" aria-label="Viewer 工具栏">
      <SearchToolbar
        query={state.search.query}
        folders={state.folders}
        focusRequest={state.search.focusRequest}
        onTextChange={setSearchText}
        onScopeChange={setSearchScope}
        onFiltersChange={setSearchFilters}
        onSortChange={setSearchSort}
        onRemoveFilter={removeSearchFilter}
        onClearFilters={clearSearchFilters}
      />
      <WorkspaceViewMenu context={viewContext} />
      <WorkspaceMoreMenu
        access={state.project.access}
        closing={state.status === 'closing'}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenPermissionSettings={() => void openPermissionSettings()}
        onReselectProject={() => void reselectProject()}
        onCloseProject={() => void closeProject()}
      />
    </div>
  </header>
```

Delete the standalone settings trigger and old project menu. Remove the `currentPath` heading from
`ContentBrowser`, remove the repeated heading from `FolderOverview`, and keep an aggregate state
label only inside the content surface when active.

- [ ] **Step 6: Apply exact shell geometry**

Use the following structural declarations in `app.css`:

```css
.workspace-header {
  display: grid;
  grid-template-columns: var(--viewer-sidebar-width) minmax(0, 1fr);
  min-height: 52px;
  padding: 0;
  background: var(--viewer-surface);
  border-bottom: 1px solid var(--viewer-border);
}
.project-identity {
  display: flex;
  align-items: center;
  min-width: 0;
  padding: 0 16px;
  background: var(--viewer-sidebar);
  border-right: 1px solid var(--viewer-border);
}
.workspace-header-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 0 12px;
}
.search-field {
  min-height: 34px;
  background: var(--viewer-soft-surface);
  border: 1px solid transparent;
  border-radius: 10px;
}
.filter-count {
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--viewer-accent);
  color: #fff;
}
```

Menus use 12 px radius, `var(--viewer-shadow-popover)`, and no width larger than the available viewport. Controls use 28–32 px visual height and the shared focus ring.

- [ ] **Step 7: Run shell regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/SearchToolbar.test.tsx \
  src/components/WorkspaceViewMenu.test.tsx \
  src/components/WorkspaceMoreMenu.test.tsx \
  src/components/FolderOverview.test.tsx \
  src/App.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

- [ ] **Step 8: Commit the aligned shell**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/SearchToolbar.tsx ui/src/components/SearchToolbar.test.tsx ui/src/components/WorkspaceViewMenu.tsx ui/src/components/WorkspaceViewMenu.test.tsx ui/src/components/WorkspaceMoreMenu.tsx ui/src/components/WorkspaceMoreMenu.test.tsx ui/src/components/FolderOverview.tsx ui/src/components/FolderOverview.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: align Viewer shell and simplify toolbar"
```

---

### Task 3: Restyle content browsing, selection, other files, and drag feedback

**Files:**
- Modify: `ui/src/App.tsx:995-1087`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx:1-640`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/contentBrowser/ImageCell.tsx`
- Modify: `ui/src/components/contentBrowser/OtherFilePanel.tsx`
- Modify: `ui/src/components/contentBrowser/OtherFilePanel.test.tsx`
- Delete: `ui/src/components/contentBrowser/SelectAllChoicePanel.tsx`
- Delete: `ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.test.tsx`
- Modify: `ui/src/components/FolderTree.tsx`
- Modify: `ui/src/components/FolderTree.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/adaptiveOtherFilePanel.css`
- Modify: `ui/src/styles/adaptiveOtherFilePanel.test.ts`

**Interfaces:**
- Consumes: `SelectAllScope`, `SelectAllRequest`, `resolveSelectAllRequest`, existing `onSelectionChange`, Finder drag, organization drag, preview, radial-menu, and other-file preference callbacks.
- Produces:

```ts
export interface ContentViewCommand {
  requestId: number
  scope: SelectAllScope
}

viewCommand?: ContentViewCommand | null
onViewStateChange?(request: SelectAllRequest): void
onRequestViewMenu?(): void
```

Add the final three declarations in that block to the current `ContentBrowserProps` interface.

- Produces a bottom floating `.selection-action-bar` with selected count only; file actions remain in the existing radial/context menu and shortcuts.

- [ ] **Step 1: Write failing content-view and selection tests**

Add to `ContentBrowser.test.tsx`:

```tsx
it('starts directly with images and consumes select-all commands from the global view menu', () => {
  const selection = vi.fn()
  const viewState = vi.fn()
  const rendered = render(
    <ContentBrowser
      workspace={workspace(3)}
      density="standard"
      viewCommand={null}
      onViewStateChange={viewState}
      onRequestViewMenu={vi.fn()}
      onSelectionChange={selection}
      otherFilePanelExpanded={false}
      onOtherFilePanelExpandedChange={vi.fn()}
    />,
  )
  expect(screen.queryByText('当前文件夹')).not.toBeInTheDocument()
  expect(screen.queryByRole('button', { name: '视图' })).not.toBeInTheDocument()
  expect(viewState).toHaveBeenCalledWith({ kind: 'direct', scope: 'images' })

  rendered.rerender(
    <ContentBrowser
      workspace={workspace(3)}
      density="standard"
      viewCommand={{ requestId: 1, scope: 'images' }}
      onViewStateChange={viewState}
      onRequestViewMenu={vi.fn()}
      onSelectionChange={selection}
      otherFilePanelExpanded={false}
      onOtherFilePanelExpandedChange={vi.fn()}
    />,
  )
  expect(selection).toHaveBeenLastCalledWith(expect.arrayContaining(workspace(3).images))
  expect(screen.getByRole('status', { name: '选择摘要' })).toHaveTextContent('已选择 3 项')
})
```

Add an App test that presses `Meta+A` in a mixed folder and asserts `WorkspaceViewMenu` opens with `全选图片`, `全选其它文件`, and `全选全部文件`.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ContentBrowser.test.tsx \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/App.test.tsx -t "select-all|global view menu|starts directly"
```

Expected: FAIL because `ContentBrowser` still owns a local content toolbar and has no typed view command.

- [ ] **Step 3: Move select-all coordination to the global view menu**

In `ContentBrowser`:

```tsx
useEffect(() => {
  onViewStateChange?.(selectAllRequest)
}, [onViewStateChange, selectAllRequest])

const consumedViewCommand = useRef(0)
useEffect(() => {
  if (viewCommand == null || viewCommand.requestId <= consumedViewCommand.current) return
  consumedViewCommand.current = viewCommand.requestId
  commitSelectAll(viewCommand.scope)
}, [commitSelectAll, viewCommand])
```

Replace mixed-content `Meta+A` behavior with:

```tsx
if (event.metaKey && event.key.toLowerCase() === 'a') {
  event.preventDefault()
  if (selectAllRequest.kind === 'choice') onRequestViewMenu?.()
  else if (selectAllRequest.kind === 'direct') commitSelectAll(selectAllRequest.scope)
}
```

Wrap `commitSelection` in `useCallback` with `[allFiles, onSelectionChange]`, then wrap
`commitSelectAll` in `useCallback` with `[commitSelection, workspace]` so the command effect has a
complete, stable dependency list.

Remove `content-toolbar`, `content-view-menu`, the local disclosure state, and `SelectAllChoicePanel`. Delete the now-unused component and test. In `App`, keep `contentSelectAllRequest`, `contentViewCommand`, and a monotonic request id; pass exact scopes selected in `WorkspaceViewMenu` to `ContentBrowser`.

- [ ] **Step 4: Add the floating selection summary**

Render after `content-browser-body`:

```tsx
{selected.size > 0 && (
  <div className="selection-action-bar" role="status" aria-label="选择摘要">
    <strong>已选择 {selected.size} 项</strong>
    <span>右键或使用快捷键进行操作</span>
  </div>
)}
```

Do not add new rename, copy, move, delete, or marker callbacks. The summary communicates selection while the existing radial menu, keyboard shortcuts, and command flows remain authoritative.

- [ ] **Step 5: Apply browsing and drag visual contracts**

Use these exact structural rules:

```css
.content-browser {
  position: relative;
  height: 100%;
  background: var(--viewer-surface);
}
.content-browser-body {
  height: 100%;
  padding: 12px;
}
.image-cell {
  border: 1px solid transparent;
  border-radius: 8px;
  background: #f8f8f6;
}
.image-cell[aria-selected="true"] {
  border-color: var(--viewer-accent);
  box-shadow: 0 0 0 2px rgb(88 105 207 / 14%);
}
.selection-action-bar {
  position: absolute;
  bottom: 16px;
  left: 50%;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 38px;
  padding: 0 14px;
  border: 1px solid var(--viewer-border);
  border-radius: 12px;
  background: var(--viewer-surface);
  box-shadow: var(--viewer-shadow-popover);
  transform: translateX(-50%);
}
.organization-drag-preview {
  min-height: 32px;
  padding: 0 10px;
  border: 1px solid var(--viewer-accent);
  border-radius: 8px;
  background: var(--viewer-surface);
  color: #4152b4;
  box-shadow: var(--viewer-shadow-popover);
}
.folder-tree [data-organization-drop-target="true"] {
  background: var(--viewer-accent-soft);
  box-shadow: inset 2px 0 var(--viewer-accent);
  color: #4152b4;
}
```

Keep the Finder drag representation native. Do not alter `beginFinderDrag`.

- [ ] **Step 6: Make other files a subordinate bottom panel**

Keep the existing expansion preference and adaptive model. Style the disclosure as one bottom hairline-separated row, remove card-like borders around the whole panel, and use compact list rows. The contract must resolve to:

```css
.other-file-panel {
  border-top: 1px solid var(--viewer-border);
  background: var(--viewer-surface);
  box-shadow: none;
}
.other-file-panel > summary {
  min-height: 38px;
  padding: 0 12px;
  color: var(--viewer-text-secondary);
}
.other-file-row {
  min-height: 34px;
  border-bottom: 1px solid #ededeb;
  border-radius: 0;
}
```

- [ ] **Step 7: Run browsing regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ContentBrowser.test.tsx \
  src/components/contentBrowser/OtherFilePanel.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/components/FolderTree.test.tsx \
  src/state/useOrganizationPointerDrag.test.tsx \
  src/App.test.tsx \
  src/styles/app.test.ts \
  src/styles/adaptiveOtherFilePanel.test.ts
```

Expected: PASS.

- [ ] **Step 8: Commit browsing and selection**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx ui/src/components/contentBrowser/ImageCell.tsx ui/src/components/contentBrowser/OtherFilePanel.tsx ui/src/components/contentBrowser/OtherFilePanel.test.tsx ui/src/components/FolderFilmstripRow.tsx ui/src/components/FolderFilmstripRow.test.tsx ui/src/components/FolderTree.tsx ui/src/components/FolderTree.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts ui/src/styles/adaptiveOtherFilePanel.css ui/src/styles/adaptiveOtherFilePanel.test.ts
git add -u ui/src/components/contentBrowser/SelectAllChoicePanel.tsx ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx
git commit -m "feat: refine content browsing and selection"
```

---

### Task 4: Rebuild search filters and result presentation

**Files:**
- Modify: `ui/src/components/SearchToolbar.tsx`
- Modify: `ui/src/components/SearchToolbar.test.tsx`
- Modify: `ui/src/components/SearchResults.tsx`
- Modify: `ui/src/components/SearchResults.test.tsx`
- Modify: `ui/src/App.tsx:995-1020`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: existing `SearchQueryModel`, `SearchResultPage`, snippets, offset, paging, layout, filter, sort, and scope callbacks.
- Produces: no new domain or bridge interface.
- Produces: `.search-results-summary`, `.search-result-group`, `.search-result-row`, `.search-result-context`, and `.search-pagination` presentation hooks.

- [ ] **Step 1: Write failing filter hierarchy and result-row tests**

Update `SearchToolbar.test.tsx` with:

```tsx
fireEvent.click(screen.getByRole('button', { name: /筛选/ }))
expect(screen.getByRole('group', { name: '文件类型' })).toBeVisible()
expect(screen.getByRole('group', { name: '审阅状态' })).toBeVisible()
expect(screen.getByLabelText('最小宽度')).not.toBeVisible()
fireEvent.click(screen.getByRole('button', { name: '高级条件' }))
expect(screen.getByLabelText('最小宽度')).toBeVisible()
expect(screen.getByLabelText('最晚修改时间')).toBeVisible()
```

Update `SearchResults.test.tsx` with:

```tsx
const region = screen.getByRole('region', { name: '搜索结果区域' })
expect(within(region).getByText(/128 个结果/)).toBeVisible()
expect(within(region).getByRole('group', { name: '衣服 / A01' })).toBeVisible()
expect(within(region).getByRole('navigation', { name: '搜索结果分页' })).toBeVisible()
expect(within(region).getByText('文件名与路径匹配')).toHaveClass('search-result-context')
```

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/SearchToolbar.test.tsx \
  src/components/SearchResults.test.tsx
```

Expected: FAIL because advanced filters are always expanded and result groups do not expose the approved structure/classes.

- [ ] **Step 3: Implement the filter hierarchy without changing capability**

Keep every existing filter input and callback. The popover order is:

```tsx
<header className="filter-popover-header">
  <h2>筛选</h2>
  <button type="button" aria-label="关闭筛选" onClick={() => setOptionsOpen(false)}>关闭</button>
</header>
<div className="filter-scope-sort">{scopeAndSortControls}</div>
<div className="common-filter-grid">{fileKindAndReviewControls}</div>
<details className="advanced-filter-group">
  <summary role="button">高级条件</summary>
  <div className="advanced-filter-grid">
    {orientationControls}
    {dimensionControls}
    {fileSizeControls}
    {modifiedTimeControls}
  </div>
</details>
<footer className="active-filter-summary">{activeFilterChipsAndClearAction}</footer>
```

Extract the current JSX into the six named local constants without changing values, labels,
disabled states, or callbacks.

Constrain the popover with:

```css
.search-options-popover {
  width: min(580px, calc(100vw - 24px));
  max-height: calc(100vh - 72px);
  overflow: auto;
  border: 1px solid var(--viewer-border);
  border-radius: var(--viewer-radius-popover);
  background: var(--viewer-surface);
  box-shadow: var(--viewer-shadow-popover);
}
```

- [ ] **Step 4: Give grouped and flat results one row model**

Keep `resultRows()` and virtualization. For group rows, render:

```tsx
<div className="search-result-group" role="group" aria-label={row.path}>
  <strong>{row.path}</strong>
  <span>{row.count} 项</span>
</div>
```

For hits, retain the thumbnail/file-kind preview, title, path, matched snippet, marker, and click behavior. Add a `search-result-context` span around metadata or bounded snippet, and keep the row order stable in both layouts.

At the top render total and live status:

```tsx
<header className="search-results-summary">
  <div><strong>{page.total} 个结果</strong>{searching && <span role="status">结果仍在更新</span>}</div>
  <button type="button" onClick={onReturnToFolder}>返回文件夹内容</button>
</header>
```

Pass `searching={state.search.status === 'searching'}` from App.

- [ ] **Step 5: Apply compact list geometry**

Use a 42–48 px result row, 28 px group header, 40 px summary, and 40 px pagination footer. Selected and keyboard-focused rows use the accent tokens; snippets remain secondary text and matched ranges use a soft accent background, not yellow.

- [ ] **Step 6: Run search and App regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/SearchToolbar.test.tsx \
  src/components/SearchResults.test.tsx \
  src/state/useViewerController.test.tsx \
  src/App.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS with search semantics and paging unchanged.

- [ ] **Step 7: Commit search presentation**

```bash
git add ui/src/components/SearchToolbar.tsx ui/src/components/SearchToolbar.test.tsx ui/src/components/SearchResults.tsx ui/src/components/SearchResults.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: refine search filters and results"
```

---

### Task 5: Unify image preview and comparison chrome

**Files:**
- Modify: `ui/src/components/ImagePreview.tsx:199-292`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/components/CompareWorkspace.tsx:282-379`
- Modify: `ui/src/components/CompareWorkspace.test.tsx`
- Modify: `ui/src/components/ComparePane.tsx`
- Modify: `ui/src/components/ComparePane.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: all existing image request, transform, navigation, marker, pane removal, synchronization, and read-only callbacks.
- Produces: `.preview-toolbar-leading`, `.preview-toolbar-transform`, `.preview-toolbar-actions`, `.preview-navigation-float`, and matching compare-toolbar groups.
- Does not change preview/compare props or state models.

- [ ] **Step 1: Write failing structure and accessibility tests**

Add to `ImagePreview.test.tsx`:

```tsx
const dialog = screen.getByRole('dialog', { name: /front\.jpg/ })
expect(within(dialog).getByRole('toolbar', { name: '图片显示控制' })).toBeVisible()
expect(within(dialog).getByRole('navigation', { name: '图片导航' })).toHaveClass(
  'preview-navigation-float',
)
expect(within(dialog).queryByText('front.jpg').closest('footer')).toBeNull()
```

Add to `CompareWorkspace.test.tsx`:

```tsx
const workspace = screen.getByRole('region', { name: '图片对比' })
expect(within(workspace).getByRole('toolbar', { name: '对比工具' })).toBeVisible()
expect(within(workspace).getByRole('button', { name: '完成对比' })).toBeVisible()
expect(within(workspace).getAllByRole('group', { name: /图片/ })[0]).toHaveAttribute(
  'data-active',
  'true',
)
```

- [ ] **Step 2: Run focused preview tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ImagePreview.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/ComparePane.test.tsx
```

Expected: FAIL because image navigation is still a full-width footer and toolbar groups/classes are absent.

- [ ] **Step 3: Regroup ImagePreview controls**

Keep every current button, accessible name, handler, transform, and disabled state. Move the
filename/dimension nodes into `preview-toolbar-leading`, move fit/100%/zoom-out/percentage/zoom-in
into the named toolbar, move rotate and close into `preview-toolbar-actions`, and move
previous/count/next into the navigation element:

```tsx
<header className="preview-toolbar">
  <div className="preview-toolbar-leading">{previewIdentity}</div>
  <div className="preview-toolbar-transform" role="toolbar" aria-label="图片显示控制">
    {displayControls}
  </div>
  <div className="preview-toolbar-actions">{previewActions}</div>
</header>
<div className="image-preview-stage">{previewStage}</div>
<nav className="preview-navigation-float" aria-label="图片导航">
  {previewNavigation}
</nav>
```

Use local `ReactNode` constants for the five named groups so the extraction is mechanical and the
current callback expressions remain unchanged. The preview root remains modal and
application-covering.

- [ ] **Step 4: Regroup CompareWorkspace controls and pane chrome**

Use the same three toolbar groups. The trailing action accessible name is `完成对比`, while visible copy remains `完成`. Retain every synchronization, zoom, rotation, fit, removal, active-pane, marker, and original-image behavior.

Set `role="toolbar"` and `aria-label="对比工具"` on the compare toolbar. Set `role="group"`, set
`aria-label` with the current `对比 ${file.name}` template, and set `data-active={active}` on the
current pane root in `ComparePane`. Do not add shadows to ordinary panes.

- [ ] **Step 5: Apply shared preview geometry**

```css
.preview-overlay,
.compare-workspace {
  background: #f0f1ef;
  color: var(--viewer-text);
}
.preview-toolbar,
.compare-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  min-height: 52px;
  padding: 0 14px;
  background: var(--viewer-surface);
  border-bottom: 1px solid var(--viewer-border);
}
.preview-toolbar-actions,
.compare-toolbar-actions {
  justify-self: end;
}
.preview-navigation-float {
  position: absolute;
  bottom: 18px;
  left: 50%;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 38px;
  padding: 0 8px;
  border: 1px solid var(--viewer-border);
  border-radius: 12px;
  background: var(--viewer-surface);
  box-shadow: var(--viewer-shadow-popover);
  transform: translateX(-50%);
}
.compare-fit-layout {
  gap: 10px;
}
.compare-pane {
  border: 1px solid var(--viewer-border);
  background: var(--viewer-surface);
  box-shadow: none;
}
.compare-pane[data-active="true"] {
  border-color: var(--viewer-accent);
  box-shadow: 0 0 0 2px rgb(88 105 207 / 14%);
}
```

- [ ] **Step 6: Run preview, comparison, and layout regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/ImagePreview.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/ComparePane.test.tsx \
  src/components/CompareVirtualViewport.test.tsx \
  src/components/useCompareLayout.test.tsx \
  src/components/compareLayoutEngine.test.ts \
  src/state/compareModel.test.ts \
  src/styles/app.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit image workspaces**

```bash
git add ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx ui/src/components/CompareWorkspace.tsx ui/src/components/CompareWorkspace.test.tsx ui/src/components/ComparePane.tsx ui/src/components/ComparePane.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: unify image preview and comparison chrome"
```

---

### Task 6: Unify text, Markdown, unsupported-file, and information surfaces

**Files:**
- Modify: `ui/src/components/TextPreview.tsx:1-129`
- Modify: `ui/src/components/TextPreviewPane.tsx:1-145`
- Modify: `ui/src/components/TextPreview.test.tsx`
- Modify: `ui/src/components/UnsupportedFilePreview.tsx:1-50`
- Modify: `ui/src/components/UnsupportedFileState.tsx:1-34`
- Modify: `ui/src/components/UnsupportedFilePreview.test.tsx`
- Modify: `ui/src/components/InfoOverlay.tsx:1-169`
- Modify: `ui/src/components/InfoOverlay.test.tsx`
- Modify: `ui/src/styles/filePreviewExtensions.css`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: the current `TextPreviewProps`, `TextPreviewPaneProps`, `UnsupportedFileStateProps`,
  `InfoOverlayProps`, text encodings, Markdown HTML, unavailable-file state, and selection
  information.
- Produces: `.text-preview-toolbar`, `.text-preview-stage`, `.text-preview-pane-toolbar`,
  `.markdown-reading-surface`, `.info-section`, and `.unsupported-file-message`.
- Does not add text diffing, merging, external-open actions, preview formats, bridge calls, or modal
  behavior to the information inspector.

- [ ] **Step 1: Write failing shared-shell and inspector tests**

Add to `TextPreview.test.tsx`:

```tsx
const dialog = screen.getByRole('dialog', { name: /plain\.txt.*notes\.md/ })
expect(within(dialog).getByRole('toolbar', { name: '文本预览控制' })).toBeVisible()
expect(within(dialog).getByRole('button', { name: '关闭预览' })).toHaveTextContent('完成')
expect(within(dialog).getAllByRole('region')).toHaveLength(2)
expect(within(dialog).queryByText(/差异|合并/)).not.toBeInTheDocument()
```

Add to `InfoOverlay.test.tsx` for a single image:

```tsx
const inspector = screen.getByRole('complementary', { name: '文件信息' })
expect(inspector).toHaveClass('info-overlay')
expect(within(inspector).getByRole('group', { name: '身份与位置' })).toBeVisible()
expect(within(inspector).getByRole('group', { name: '审阅信息' })).toBeVisible()
expect(within(inspector).getByRole('group', { name: '技术信息' })).toBeVisible()
expect(inspector).not.toHaveAttribute('aria-modal')
```

Add to `UnsupportedFilePreview.test.tsx`:

```tsx
const dialog = screen.getByRole('dialog', { name: 'archive.zip' })
expect(within(dialog).getByRole('toolbar', { name: '文件预览控制' })).toBeVisible()
expect(within(dialog).getByText('暂不支持预览')).toHaveClass('unsupported-file-message')
expect(within(dialog).queryByRole('button', { name: /外部|其它应用/ })).not.toBeInTheDocument()
```

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/TextPreview.test.tsx \
  src/components/UnsupportedFilePreview.test.tsx \
  src/components/InfoOverlay.test.tsx
```

Expected: FAIL because the shared toolbar roles, reading-surface class, and grouped inspector
structure are not present.

- [ ] **Step 3: Apply the shared preview shell to text and unsupported files**

In `TextPreview`, keep the dialog root, focus trap, task reporting, and pane loop. Replace the toolbar
contents with:

```tsx
<header className="preview-toolbar text-preview-toolbar">
  <div className="preview-toolbar-leading">
    <strong>{files.map((file) => file.name).join(' · ')}</strong>
    <span>{files.map((file) => textFormatLabel(file.kind)).join(' · ')}</span>
  </div>
  <div className="preview-toolbar-actions" role="toolbar" aria-label="文本预览控制">
    <button type="button" aria-label="关闭预览" onClick={onClose}>完成</button>
  </div>
</header>
<div className="text-preview-stage">
  <div className="text-preview-panes" data-pane-count={files.length}>{paneNodes}</div>
</div>
```

Define `paneNodes` without changing pane props:

```tsx
const paneNodes = files.map((file) => (
  <TextPreviewPane
    key={`${file.entityId}:${file.modifiedNs}`}
    file={file}
    unavailable={unavailableEntityIds.has(file.entityId)}
    requestPreview={requestPreview}
    openExternalLink={openExternalLink}
    onStatusChange={recordStatus}
  />
))

function textFormatLabel(kind: BrowserFile['kind']): string {
  return kind === 'markdown' ? 'Markdown' : '纯文本'
}
```

Use the same header groups in `UnsupportedFilePreview`, with `role="toolbar"`
and `aria-label="文件预览控制"`, and render visible `完成` copy while preserving the accessible name
`关闭预览`. Wrap `UnsupportedFileState` in `<div className="preview-stage">` so it shares the
neutral application-covering stage.

- [ ] **Step 4: Make each text pane a quiet reading region**

Add `role="region"` to the current named `TextPreviewPane` section. Keep filename and encoding in
the pane sub-toolbar. Add `markdown-reading-surface` to the Markdown element:

```tsx
<div
  className="markdown-preview markdown-reading-surface"
  onClick={markdownClicked}
  dangerouslySetInnerHTML={{ __html: preview.markdownHtml ?? '' }}
/>
```

Do not add line numbers, diff markers, merge controls, or cross-pane synchronization. Keep current
loading, encoding-required, error, truncation, link-opening, request generation, and status
reporting logic unchanged.

- [ ] **Step 5: Group the information inspector without changing its data**

Keep `InfoOverlay` as an `<aside>`. For one file, split the current definition list into three named
groups:

```tsx
<section className="info-section" role="group" aria-labelledby="info-identity">
  <h3 id="info-identity">身份与位置</h3>
  <dl>{identityRows}</dl>
</section>
<section className="info-section" role="group" aria-labelledby="info-review">
  <h3 id="info-review">审阅信息</h3>
  <dl>{reviewRows}</dl>
</section>
<section className="info-section" role="group" aria-labelledby="info-technical">
  <h3 id="info-technical">技术信息</h3>
  <dl>{technicalRows}</dl>
</section>
```

Assign name/path/type rows to `identityRows`, review/favorite rows to `reviewRows`, and
dimensions/size/modified-time rows to `technicalRows`. For aggregate selection, use the same three
headings and place current aggregate values under the closest matching section. Do not change
formatting functions or introduce file actions.

- [ ] **Step 6: Apply exact preview and inspector geometry**

Add:

```css
.text-preview-stage,
.unsupported-file-preview .preview-stage {
  min-height: 0;
  height: calc(100% - 52px);
  padding: 16px;
  overflow: auto;
  background: #f0f1ef;
}
.text-preview-panes {
  display: grid;
  grid-template-columns: repeat(var(--text-pane-count, 1), minmax(0, 1fr));
  min-height: 100%;
  border: 1px solid var(--viewer-border);
  background: var(--viewer-surface);
}
.text-preview-panes[data-pane-count="1"] {
  --text-pane-count: 1;
}
.text-preview-panes[data-pane-count="2"] {
  --text-pane-count: 2;
}
.text-preview-pane + .text-preview-pane {
  border-left: 1px solid var(--viewer-border);
}
.text-preview-pane-toolbar {
  position: sticky;
  top: 0;
  min-height: 40px;
  background: var(--viewer-surface);
  border-bottom: 1px solid var(--viewer-border);
}
.markdown-reading-surface {
  width: min(100%, 800px);
  margin: 0 auto;
  padding: 32px;
  color: var(--viewer-text);
  background: var(--viewer-surface);
}
.plain-text-preview {
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
  font-size: 12px;
  line-height: 1.65;
}
.info-overlay {
  position: fixed;
  z-index: 40;
  top: 52px;
  right: 0;
  bottom: 0;
  width: clamp(320px, 24vw, 340px);
  overflow: auto;
  border-left: 1px solid var(--viewer-border);
  background: var(--viewer-surface);
  box-shadow: -12px 0 32px rgb(31 35 42 / 8%);
}
.info-section + .info-section {
  border-top: 1px solid var(--viewer-border);
}
.unsupported-file-state {
  max-width: 420px;
  margin: auto;
  border: 0;
  background: transparent;
  box-shadow: none;
}
.unsupported-file-message {
  color: var(--viewer-text-secondary);
}
```

Add the `unsupported-file-message` class to the status span. Preserve compact unsupported-file
geometry inside other-file rows.

- [ ] **Step 7: Run preview and information regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/TextPreview.test.tsx \
  src/components/UnsupportedFilePreview.test.tsx \
  src/components/InfoOverlay.test.tsx \
  src/components/TaskBar.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

- [ ] **Step 8: Commit text and information surfaces**

```bash
git add ui/src/components/TextPreview.tsx ui/src/components/TextPreviewPane.tsx ui/src/components/TextPreview.test.tsx ui/src/components/UnsupportedFilePreview.tsx ui/src/components/UnsupportedFileState.tsx ui/src/components/UnsupportedFilePreview.test.tsx ui/src/components/InfoOverlay.tsx ui/src/components/InfoOverlay.test.tsx ui/src/styles/filePreviewExtensions.css ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: unify text preview and file information"
```

---

### Task 7: Preserve the radial menu, add its conventional fallback, and standardize dialogs

**Files:**
- Create: `ui/src/components/FileContextMenu.tsx`
- Create: `ui/src/components/FileContextMenu.test.tsx`
- Modify: `ui/src/App.tsx:318-340,1100-1114`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/RadialFileMenu.tsx:1-455`
- Modify: `ui/src/components/RadialFileMenu.test.tsx`
- Modify: `ui/src/components/radialMenuGeometry.test.ts`
- Modify: `ui/src/components/ModalSheet.tsx:1-77`
- Modify: `ui/src/components/ModalSheet.test.tsx`
- Modify: `ui/src/components/DestinationDialog.tsx:110-208`
- Modify: `ui/src/components/DestinationDialog.test.tsx`
- Modify: `ui/src/components/BatchRenameDialog.tsx:66-200`
- Modify: `ui/src/components/BatchRenameDialog.test.tsx`
- Modify: `ui/src/components/CloseOperationDialog.tsx:18-48`
- Modify: `ui/src/components/CloseOperationDialog.test.tsx`
- Modify: `ui/src/components/RenameDialog.tsx`
- Modify: `ui/src/components/RenameDialog.test.tsx`
- Modify: `ui/src/components/TrashConfirmation.tsx`
- Modify: `ui/src/components/TrashConfirmation.test.tsx`
- Modify: `ui/src/components/SettingsDialog.tsx`
- Modify: `ui/src/components/SettingsDialog.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: `RadialMenuItem[]`, `RadialLeafAction`, `Point`, the active radial-menu session,
  `ModalSheet` focus behavior, all current dialog callbacks, and current conflict/rename models.
- Produces:

```ts
export interface FileContextMenuProps {
  origin: Point
  selectionCount: number
  readOnly?: boolean
  returnFocusTarget?: HTMLElement | null
  model: RadialMenuItem[]
  viewport?: Viewport
  onAction(action: RadialLeafAction): void
  onClose(returnFocusTarget: HTMLElement | null): void
}
```

- Keeps exact radial geometry: primary inner radius 42, primary outer radius 108, six 60-degree
  sectors; secondary inner radius 112, secondary outer radius 168, 30-degree sectors.

- [ ] **Step 1: Write failing fallback, radial-token, and dialog-structure tests**

Create `FileContextMenu.test.tsx` with:

```tsx
it('presents the radial action model as one conventional menu', () => {
  const action = vi.fn()
  render(
    <FileContextMenu
      origin={{ x: 320, y: 240 }}
      selectionCount={2}
      model={model}
      onAction={action}
      onClose={vi.fn()}
    />,
  )
  const menu = screen.getByRole('menu', { name: '文件操作' })
  expect(within(menu).getAllByRole('menuitem')).toHaveLength(6)
  fireEvent.click(within(menu).getByRole('menuitem', { name: '标记' }))
  expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toBeVisible()
  expect(screen.getByRole('menuitem', { name: '移到废纸篓' })).toHaveAttribute(
    'data-tone',
    'destructive',
  )
})
```

Add to `App.test.tsx`: secondary-click produces a compact menu and a held right-button gesture
produces `.radial-file-menu`.

Update `RadialFileMenu.test.tsx` to assert:

```tsx
expect(container.querySelectorAll('.radial-primary-shape')).toHaveLength(6)
expect(primarySectors(container)).toEqual([
  { start: '-120', end: '-60' },
  { start: '-60', end: '0' },
  { start: '0', end: '60' },
  { start: '60', end: '120' },
  { start: '120', end: '180' },
  { start: '180', end: '240' },
])
expect(screen.getByRole('button', { name: '关闭文件操作' })).toHaveTextContent('回到中心取消')
```

Add modal assertions:

```tsx
const dialog = screen.getByRole('dialog', { name: '安全操作' })
expect(dialog.querySelector('.modal-sheet-header')).not.toBeNull()
expect(dialog.querySelector('.modal-sheet-body')).not.toBeNull()
```

Add Destination and Batch Rename assertions for `.destination-dialog-layout`,
`.destination-dialog-main`, `.batch-rename-rule-region`, and `.rename-preview-summary`.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/FileContextMenu.test.tsx \
  src/components/RadialFileMenu.test.tsx \
  src/components/radialMenuGeometry.test.ts \
  src/components/ModalSheet.test.tsx \
  src/components/DestinationDialog.test.tsx \
  src/components/BatchRenameDialog.test.tsx \
  src/components/CloseOperationDialog.test.tsx \
  src/App.test.tsx -t "context menu|right-button|dialog"
```

Expected: FAIL because the conventional component and common dialog slots do not exist.

- [ ] **Step 3: Implement the conventional context-menu fallback**

`FileContextMenu` uses the same model without rebuilding or relabeling it. Its root positioning is:

```tsx
const left = Math.min(Math.max(8, origin.x), Math.max(8, viewport.width - 260))
const top = Math.min(Math.max(8, origin.y), Math.max(8, viewport.height - 360))

<div
  ref={rootRef}
  className="file-context-menu"
  style={{ left, top }}
  onKeyDown={handleMenuKeyDown}
>
  <div role="menu" aria-label="文件操作">
    {model.map((item, index) => renderMenuItem(item, index))}
  </div>
  <p className="file-context-summary">{selectionCount} 个文件{readOnly ? ' · 只读' : ''}</p>
</div>
```

`renderMenuItem` must use `menuitemcheckbox` when `checked` is defined, expose `aria-checked`,
`aria-disabled`, `aria-haspopup`, `aria-expanded`, `title={disabledReason}`, and `data-tone`.
Primary items with children open one nested sibling `role="menu"`; leaf items call `onAction`.
Implement roving focus for ArrowUp/ArrowDown, ArrowRight to open a child group, ArrowLeft to return,
Enter/Space to activate, Escape/outside pointer to close, and restore `returnFocusTarget`. Show the
disabled reason in a `<small>` with its own id, and connect it with `aria-describedby` so the
visible reason does not replace the item's accessible name.

In `App`, route only session requests with `pointerId === null` to `FileContextMenu`. Route non-null
requests to `RadialFileMenu`:

```tsx
{activeRadialMenu?.pointerId === null && (
  <FileContextMenu
    key={activeRadialMenu.requestId}
    origin={activeRadialMenu.origin}
    selectionCount={activeRadialMenu.files.length}
    readOnly={state.project.access === 'read_only'}
    returnFocusTarget={activeRadialMenu.returnFocusTarget}
    model={radialModel}
    onAction={runRadialAction}
    onClose={finishRadialSession}
  />
)}
{activeRadialMenu?.pointerId !== null && activeRadialMenu !== null && (
  <RadialFileMenu
    key={activeRadialMenu.requestId}
    origin={activeRadialMenu.origin}
    pointerId={activeRadialMenu.pointerId}
    selectionCount={activeRadialMenu.files.length}
    readOnly={state.project.access === 'read_only'}
    returnFocusTarget={activeRadialMenu.returnFocusTarget}
    model={radialModel}
    onAction={runRadialAction}
    onClose={finishRadialSession}
  />
)}
```

- [ ] **Step 4: Restyle the radial sectors without changing interaction geometry**

Do not change the geometry constants, pointer thresholds, 120 ms dwell, fitting, action order,
center cancellation, keyboard navigation, disabled reasons, focus restoration, or outward secondary
fan. Replace literal cool grays in radial tests and CSS with token assertions, then apply:

```css
.radial-primary-shape,
.radial-secondary-shape {
  fill: var(--viewer-surface);
  stroke: var(--viewer-border);
  stroke-width: 1;
}
.radial-primary-shape[data-active="true"],
.radial-secondary-shape[data-active="true"] {
  fill: var(--viewer-accent-soft);
  stroke: var(--viewer-accent);
}
.radial-primary-shape[data-disabled="true"],
.radial-secondary-shape[data-disabled="true"] {
  fill: var(--viewer-soft-surface);
  opacity: .62;
}
.radial-menu-center {
  border: 1px solid var(--viewer-border);
  background: var(--viewer-surface);
  box-shadow: var(--viewer-shadow-popover);
}
.radial-menu-button[data-tone="destructive"] {
  color: var(--viewer-danger);
}
```

Keep labels upright and aligned over their SVG sectors. Use a restrained shared drop shadow on the
shape layer, not six independent card shadows.

- [ ] **Step 5: Add common modal structure**

In `ModalSheet`, wrap the title and children while preserving the dialog element and focus logic:

```tsx
<div
  ref={dialogRef}
  className="modal-sheet"
  role="dialog"
  aria-modal="true"
  aria-labelledby={titleId}
  tabIndex={-1}
  onKeyDown={containFocus}
>
  <header className="modal-sheet-header"><h2 id={titleId}>{title}</h2></header>
  <div className="modal-sheet-body">{children}</div>
</div>
```

Use 14 px radius, 20 px body padding, one neutral border, `var(--viewer-shadow-dialog)`, and a neutral
backdrop. `data-destructive` may style the final destructive button only; it must not tint the whole
dialog.

- [ ] **Step 6: Recompose complex dialog interiors**

In `DestinationDialog`, wrap the current folder fieldset in
`.destination-dialog-sidebar`; wrap check-conflicts, messages, and preflight results in
`.destination-dialog-main`; place both inside `.destination-dialog-layout`. Use
`grid-template-columns: minmax(220px, 34%) minmax(360px, 1fr)`. Keep the current choose → check →
resolve → execute sequence and all preflight logic unchanged.

In `BatchRenameDialog`, wrap the current rule and sequence controls in
`.batch-rename-rule-region`. Before the virtual list, render:

```tsx
<div className="rename-preview-summary">
  <strong>{preview.rows.length} 项</strong>
  <span>{preview.rows.filter((row) => row.errors.length > 0).length} 项无效</span>
</div>
```

Keep the full virtualized list, first-invalid focus behavior, request revision handling, and execute
gate unchanged. Style source and proposed paths as stable two-column rows.

Keep CloseOperationDialog button order `保持打开`, `等待完成后关闭`,
`取消待处理项目并关闭`. Apply the primary class only to the wait action and the destructive class
only to cancel-pending. Rename, Trash, and Settings keep current callbacks and receive only the
common modal fields, errors, and action styling.

- [ ] **Step 7: Run menu and dialog regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/FileContextMenu.test.tsx \
  src/components/RadialFileMenu.test.tsx \
  src/components/radialMenuGeometry.test.ts \
  src/components/radialMenuModel.test.ts \
  src/components/ModalSheet.test.tsx \
  src/components/DestinationDialog.test.tsx \
  src/components/BatchRenameDialog.test.tsx \
  src/components/CloseOperationDialog.test.tsx \
  src/components/RenameDialog.test.tsx \
  src/components/TrashConfirmation.test.tsx \
  src/components/SettingsDialog.test.tsx \
  src/app/appSessionCoordinators.test.tsx \
  src/App.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

- [ ] **Step 8: Commit menus and dialogs**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/FileContextMenu.tsx ui/src/components/FileContextMenu.test.tsx ui/src/components/RadialFileMenu.tsx ui/src/components/RadialFileMenu.test.tsx ui/src/components/radialMenuGeometry.test.ts ui/src/components/ModalSheet.tsx ui/src/components/ModalSheet.test.tsx ui/src/components/DestinationDialog.tsx ui/src/components/DestinationDialog.test.tsx ui/src/components/BatchRenameDialog.tsx ui/src/components/BatchRenameDialog.test.tsx ui/src/components/CloseOperationDialog.tsx ui/src/components/CloseOperationDialog.test.tsx ui/src/components/RenameDialog.tsx ui/src/components/RenameDialog.test.tsx ui/src/components/TrashConfirmation.tsx ui/src/components/TrashConfirmation.test.tsx ui/src/components/SettingsDialog.tsx ui/src/components/SettingsDialog.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: refine menus and operation dialogs"
```

---

### Task 8: Place tasks, results, notices, errors, and read-only feedback by scope

**Files:**
- Create: `ui/src/components/GlobalNoticeStack.tsx`
- Create: `ui/src/components/GlobalNoticeStack.test.tsx`
- Modify: `ui/src/App.tsx:146-159,928-950,1091-1100,1224-1237`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/TaskBar.tsx:1-188`
- Modify: `ui/src/components/TaskBar.test.tsx`
- Modify: `ui/src/components/OperationResults.tsx:1-89`
- Modify: `ui/src/components/OperationResults.test.tsx`
- Modify: `ui/src/components/ReadOnlyBanner.tsx:1-24`
- Modify: `ui/src/components/ReadOnlyBanner.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: recovery counts, `state.errorMessage`, Finder-drag failure text, current task models,
  operation result pages, read-only callbacks, and current contextual status text.
- Produces:

```ts
export type GlobalNoticeTone = 'info' | 'warning' | 'danger'

export interface GlobalNotice {
  id: string
  title: string
  message: string
  tone: GlobalNoticeTone
  action?: {
    label: string
    onAction(): void
  }
}

export interface GlobalNoticeStackProps {
  notices: GlobalNotice[]
}
```

- Does not promote preview repair, comparison status, search status, selection feedback, or row
  failures into the global stack.

- [ ] **Step 1: Write failing scope and presentation tests**

Create `GlobalNoticeStack.test.tsx`:

```tsx
it('orders global notices and preserves action and dismissal semantics', () => {
  const showResults = vi.fn()
  render(
    <GlobalNoticeStack
      notices={[
        {
          id: 'recovery',
          title: '项目恢复完成',
          message: '已恢复 2 项操作；1 项需要检查。',
          tone: 'warning',
          action: { label: '查看结果', onAction: showResults },
        },
        {
          id: 'fatal',
          title: 'Viewer 无法继续',
          message: '项目状态不可用。',
          tone: 'danger',
        },
      ]}
    />,
  )
  expect(screen.getByRole('region', { name: '全局通知' })).toBeVisible()
  expect(screen.getByRole('alert')).toHaveTextContent('Viewer 无法继续')
  fireEvent.click(screen.getByRole('button', { name: '查看结果' }))
  expect(showResults).toHaveBeenCalledOnce()
  fireEvent.click(screen.getByRole('button', { name: '关闭 Viewer 无法继续' }))
  expect(screen.queryByText('项目状态不可用。')).not.toBeInTheDocument()
})
```

Add to `TaskBar.test.tsx`:

```tsx
const stack = screen.getByRole('complementary', { name: '后台任务' })
expect(stack).toHaveClass('task-bar')
expect(within(stack).getByRole('progressbar', { name: '扫描项目进度' })).toHaveAttribute(
  'value',
  '12',
)
```

Add to `OperationResults.test.tsx`:

```tsx
expect(screen.getByRole('complementary', { name: '文件操作结果' })).toHaveClass(
  'operation-results',
)
expect(screen.getByRole('navigation', { name: '文件操作结果分页' })).toBeVisible()
```

Update `ReadOnlyBanner.test.tsx` to query visible actions `权限设置` and `重新选择目录`.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/GlobalNoticeStack.test.tsx \
  src/components/TaskBar.test.tsx \
  src/components/OperationResults.test.tsx \
  src/components/ReadOnlyBanner.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/App.test.tsx -t "notice|task|result|read-only|local error"
```

Expected: FAIL because global notices and progress semantics are missing and result pagination is
not a named navigation region.

- [ ] **Step 3: Implement the global notice stack**

`GlobalNoticeStack` owns only dismissal memory for currently supplied notice ids:

```tsx
const [dismissed, setDismissed] = useState<Set<string>>(() => new Set())
const visible = notices.filter((notice) => !dismissed.has(notice.id))
if (visible.length === 0) return null

return (
  <section className="global-notice-stack" aria-label="全局通知">
    {visible.map((notice) => (
      <article
        key={notice.id}
        className="global-notice"
        data-tone={notice.tone}
        role={notice.tone === 'danger' ? 'alert' : 'status'}
      >
        <div>
          <strong>{notice.title}</strong>
          <p>{notice.message}</p>
        </div>
        {notice.action && (
          <button type="button" onClick={notice.action.onAction}>{notice.action.label}</button>
        )}
        <button
          type="button"
          aria-label={`关闭 ${notice.title}`}
          onClick={() => setDismissed((current) => new Set([...current, notice.id]))}
        >
          关闭
        </button>
      </article>
    ))}
  </section>
)
```

When a notice id leaves `notices`, remove that id from `dismissed` so a future independent
occurrence can appear again.

- [ ] **Step 4: Route only global feedback from App**

Build `globalNotices` with stable ids:

```tsx
const globalNotices: GlobalNotice[] = []
if (state.recoveryReport &&
    (state.recoveryReport.recovered > 0 || state.recoveryReport.needsUserReview > 0)) {
  const activeBatchId = state.operation.results === null ? null : state.operation.active?.batchId
  globalNotices.push({
    id: `recovery:${projectSessionId}`,
    title: '项目恢复完成',
    message: `已恢复 ${state.recoveryReport.recovered} 项操作；${state.recoveryReport.needsUserReview} 项需要检查。`,
    tone: state.recoveryReport.needsUserReview > 0 ? 'warning' : 'info',
    action: activeBatchId
      ? { label: '查看结果', onAction: () => setResultsBatchId(activeBatchId) }
      : undefined,
  })
}
if (state.errorMessage) {
  globalNotices.push({
    id: `application:${state.errorMessage}`,
    title: 'Viewer 出现问题',
    message: state.errorMessage,
    tone: 'danger',
  })
}
if (finderDragMessage) {
  globalNotices.push({
    id: `finder-drag:${finderDragMessage}`,
    title: '无法拖出文件',
    message: finderDragMessage,
    tone: 'danger',
  })
}
```

Render `<GlobalNoticeStack notices={globalNotices} />` after the shell columns. Remove the old
recovery, top-level error, and Finder-drag paragraphs.

Move `contextRepairMessage` into the `.workspace` section before its affected content. Render
`compareStatus` immediately after `CompareWorkspace` inside the comparison branch. Keep search
status inside the search region and selection status in the selection summary. Keep folder-row and
text-pane errors local.

- [ ] **Step 5: Turn TaskBar into floating progress cards**

After computing `finished`, add:

```tsx
<progress
  aria-label={`${currentTask.label}进度`}
  max={Math.max(1, currentTask.requested)}
  value={finished}
/>
```

Keep live-status isolation, automatic clean-success dismissal, failure detail expansion,
cancellation, result access, and explicit dismissal unchanged. Apply:

```css
.task-bar {
  position: fixed;
  z-index: 35;
  right: 16px;
  bottom: 16px;
  display: grid;
  width: min(340px, calc(100vw - 32px));
  gap: 8px;
  pointer-events: none;
}
.task-row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid var(--viewer-border);
  border-radius: var(--viewer-radius-popover);
  background: var(--viewer-surface);
  box-shadow: var(--viewer-shadow-popover);
  pointer-events: auto;
}
.task-row progress {
  grid-column: 1 / -1;
  width: 100%;
  height: 3px;
  accent-color: var(--viewer-accent);
}
```

- [ ] **Step 6: Make operation results a trailing inspector**

Wrap the current result footer controls in:

```tsx
<nav className="operation-results-pagination" aria-label="文件操作结果分页">
  {paginationNodes}
</nav>
```

Assign the current count and previous/next buttons to `paginationNodes`; preserve the 200-item page
size and offsets. Apply the same 320–340 px trailing-panel width as file information, compact
34–38 px rows, status text plus non-color labels, and fixed header/footer. Do not make the panel
modal and do not block browsing.

- [ ] **Step 7: Reduce read-only feedback and normalize local errors**

Change visible `只读项目` to `只读模式` and `打开权限设置` to `权限设置`, preserving callbacks and
disabled states. Apply:

```css
.read-only-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 38px;
  padding: 0 12px;
  border-bottom: 1px solid #eadfbf;
  background: var(--viewer-warning-soft);
  color: var(--viewer-warning);
}
.local-error,
.folder-filmstrip-error,
.form-error {
  border: 1px solid color-mix(in srgb, var(--viewer-danger) 28%, transparent);
  background: var(--viewer-danger-soft);
  color: var(--viewer-danger);
}
```

Add `.local-error` only to affected in-workspace errors. Keep failed folder-filmstrip height stable
and preserve its local retry action.

- [ ] **Step 8: Run feedback regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/GlobalNoticeStack.test.tsx \
  src/components/TaskBar.test.tsx \
  src/components/OperationResults.test.tsx \
  src/components/ReadOnlyBanner.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/components/TextPreview.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/App.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

- [ ] **Step 9: Commit scoped feedback**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/GlobalNoticeStack.tsx ui/src/components/GlobalNoticeStack.test.tsx ui/src/components/TaskBar.tsx ui/src/components/TaskBar.test.tsx ui/src/components/OperationResults.tsx ui/src/components/OperationResults.test.tsx ui/src/components/ReadOnlyBanner.tsx ui/src/components/ReadOnlyBanner.test.tsx ui/src/components/FolderFilmstripRow.tsx ui/src/components/FolderFilmstripRow.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: place Viewer feedback by scope"
```

---

### Task 9: Finish the no-project, opening, loading, thumbnail, empty, and recovery states

**Files:**
- Create: `ui/src/components/WorkspaceLoadingState.tsx`
- Create: `ui/src/components/WorkspaceLoadingState.test.tsx`
- Modify: `ui/src/components/EmptyProject.tsx:1-81`
- Modify: `ui/src/components/EmptyProject.test.tsx`
- Modify: `ui/src/components/FolderTree.tsx`
- Modify: `ui/src/components/FolderTree.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.test.tsx`
- Modify: `ui/src/components/AspectThumbnail.tsx`
- Modify: `ui/src/components/AspectThumbnail.test.tsx`
- Modify: `ui/src/App.tsx:826-838,965-1030`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: current directory chooser/drop bridge calls, `busy`, `errorMessage`, project identity,
  workspace readiness, folder-tree data, thumbnail dimensions, and reduced-motion preference.
- Produces:

```ts
export interface WorkspaceLoadingStateProps {
  bandCount?: number
}
```

- Extends current `FolderTreeProps` with `loading?: boolean`.
- Does not fabricate scan percentages, remember recent projects, change project opening, or replace
  already available content with a blocking spinner.

- [ ] **Step 1: Write failing minimal-entry and stable-loading tests**

Update `EmptyProject.test.tsx`:

```tsx
const entry = screen.getByTestId('project-drop-zone')
expect(within(entry).getByRole('heading', { name: 'Viewer' })).toBeVisible()
expect(within(entry).getByText('选择或拖入一个项目文件夹')).toBeVisible()
expect(within(entry).getByRole('button', { name: '选择项目文件夹' })).toBeVisible()
expect(within(entry).queryByText(/支持 JPG|不会记住|最近/)).not.toBeInTheDocument()
```

Add a folder-drag state test:

```tsx
fireEvent.dragEnter(entry, {
  dataTransfer: { items: [{ webkitGetAsEntry: () => ({ isDirectory: true }) }] },
})
expect(screen.getByText('松开以打开项目')).toBeVisible()
fireEvent.dragLeave(entry)
expect(screen.queryByText('松开以打开项目')).not.toBeInTheDocument()
```

Create `WorkspaceLoadingState.test.tsx`:

```tsx
render(<WorkspaceLoadingState bandCount={3} />)
const status = screen.getByRole('status', { name: '项目内容加载中' })
expect(status).toHaveAttribute('aria-busy', 'true')
expect(status.querySelectorAll('.workspace-loading-band')).toHaveLength(3)
expect(status.querySelectorAll('.workspace-loading-thumbnail').length).toBeGreaterThan(3)
```

Add an `AspectThumbnail` assertion that known width/height remain on both loading and failed
placeholders.

- [ ] **Step 2: Run focused tests and verify the red state**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/EmptyProject.test.tsx \
  src/components/WorkspaceLoadingState.test.tsx \
  src/components/FolderTree.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/components/AspectThumbnail.test.tsx \
  src/App.test.tsx -t "opening|loading|empty project|skeleton|thumbnail"
```

Expected: FAIL because the resting entry copy is verbose, drag state is not represented, and the
workspace loading component does not exist.

- [ ] **Step 3: Reduce EmptyProject to the approved resting state**

Add `dragDepth`, `dragActive`, and `openingName` presentation state. Resting markup is exactly:

```tsx
<main
  className="empty-project"
  data-drag-active={dragActive || undefined}
  data-testid="project-drop-zone"
  onDragEnter={enterProjectDrag}
  onDragOver={(event) => event.preventDefault()}
  onDragLeave={leaveProjectDrag}
  onDrop={dropProject}
>
  <h1>Viewer</h1>
  <p>选择或拖入一个项目文件夹</p>
  <button type="button" disabled={disabled} onClick={() => void chooseProject()}>
    选择项目文件夹
  </button>
  {dragActive && <div className="project-drop-feedback">松开以打开项目</div>}
  {(localError ?? errorMessage) && <p className="empty-project-error" role="alert">{localError ?? errorMessage}</p>}
</main>
```

Increment `dragDepth.current` on entry, decrement it on leave, and clear it on drop so nested targets
do not flicker. Keep the whole window as the drop target and preserve current directory validation
and safe error messages.

- [ ] **Step 4: Replace entry controls with an in-place opening state**

Before awaiting the chosen path, set:

```ts
setOpeningName(path.split(/[\\/]/).filter(Boolean).at(-1) ?? 'Viewer 项目')
```

While `working` is true, render:

```tsx
<main className="project-opening-state" aria-busy="true">
  <h1>{openingName}</h1>
  <p>正在验证项目…</p>
  <div className="project-opening-progress" role="progressbar" aria-label="正在打开项目" />
</main>
```

Do not leave the disabled chooser button on screen and do not set `aria-valuenow` on this unknown
duration progress indicator.

- [ ] **Step 5: Implement stable workspace and sidebar skeletons**

Create `WorkspaceLoadingState`:

```tsx
export default function WorkspaceLoadingState({ bandCount = 3 }: WorkspaceLoadingStateProps) {
  const bands = ['loading-band-1', 'loading-band-2', 'loading-band-3'].slice(0, bandCount)
  const thumbnails = ['loading-image-1', 'loading-image-2', 'loading-image-3', 'loading-image-4']
  return (
    <section className="workspace-loading" role="status" aria-label="项目内容加载中" aria-busy="true">
      <span className="visually-hidden">正在读取项目…</span>
      {bands.map((bandId) => (
        <div className="workspace-loading-band" aria-hidden="true" key={bandId}>
          <div className="workspace-loading-identity" />
          <div className="workspace-loading-track">
            {thumbnails.map((thumbnailId) => (
              <div className="workspace-loading-thumbnail" key={`${bandId}:${thumbnailId}`} />
            ))}
          </div>
        </div>
      ))}
    </section>
  )
}
```

Pass `loading={state.workspace === null}` to `FolderTree`. When loading and no folder rows have been
published, render five `folder-tree-skeleton-row` elements in the current tree list; once real
folders exist, render those folders immediately and omit skeleton rows.

Replace the App paragraph `正在读取项目…` with `<WorkspaceLoadingState />`. Keep the final shell,
project identity, header, sidebar, read-only strip, and task stack visible.

- [ ] **Step 6: Normalize boundary and thumbnail states**

Keep the current empty category/content/search recovery actions. Give each empty state one short
heading, one sentence, and only current recovery callbacks. Do not add illustrations or new routes.

Keep `AspectThumbnail` width and height on its root and placeholder in every status. Apply:

```css
.aspect-thumbnail-placeholder,
.workspace-loading-thumbnail,
.folder-tree-skeleton-row {
  background:
    linear-gradient(90deg, transparent, rgb(255 255 255 / 58%), transparent),
    var(--viewer-soft-surface);
  background-size: 200% 100%;
  animation: viewer-skeleton 1.4s linear infinite;
}
@keyframes viewer-skeleton {
  from { background-position: 200% 0; }
  to { background-position: -200% 0; }
}
@media (prefers-reduced-motion: reduce) {
  .aspect-thumbnail-placeholder,
  .workspace-loading-thumbnail,
  .folder-tree-skeleton-row {
    animation: none;
    background: var(--viewer-soft-surface);
  }
}
```

This reduced-motion query is allowed; only dark-scheme media queries are forbidden.

- [ ] **Step 7: Apply exact no-project and loading geometry**

```css
.empty-project,
.project-opening-state {
  display: grid;
  place-content: center;
  justify-items: center;
  min-height: 100vh;
  gap: 12px;
  padding: 32px;
  background: var(--viewer-application);
}
.empty-project h1,
.project-opening-state h1 {
  margin: 0;
  font-size: 20px;
}
.project-drop-feedback {
  position: fixed;
  inset: 12px;
  display: grid;
  place-items: center;
  border: 2px solid var(--viewer-accent);
  border-radius: 14px;
  background: rgb(236 238 255 / 82%);
  color: #4152b4;
  pointer-events: none;
}
.project-opening-progress {
  width: min(280px, 60vw);
  height: 2px;
  overflow: hidden;
  background: var(--viewer-border);
}
.workspace-loading-band {
  display: grid;
  grid-template-columns: 228px minmax(0, 1fr);
  min-height: 170px;
  border-bottom: 1px solid var(--viewer-border);
}
```

- [ ] **Step 8: Run lifecycle regressions**

Run:

```bash
pnpm --dir ui exec vitest run \
  src/components/EmptyProject.test.tsx \
  src/components/WorkspaceLoadingState.test.tsx \
  src/components/FolderTree.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/components/AspectThumbnail.test.tsx \
  src/components/GlobalNoticeStack.test.tsx \
  src/state/useViewerController.test.tsx \
  src/App.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS.

- [ ] **Step 9: Commit lifecycle states**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/WorkspaceLoadingState.tsx ui/src/components/WorkspaceLoadingState.test.tsx ui/src/components/EmptyProject.tsx ui/src/components/EmptyProject.test.tsx ui/src/components/FolderTree.tsx ui/src/components/FolderTree.test.tsx ui/src/components/FolderFilmstripRow.tsx ui/src/components/FolderFilmstripRow.test.tsx ui/src/components/AspectThumbnail.tsx ui/src/components/AspectThumbnail.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: finish Viewer lifecycle states"
```

---

### Task 10: Remove obsolete presentation rules and complete automated and visual verification

**Files:**
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`
- Modify: `ui/src/styles/filePreviewExtensions.css`
- Modify: `ui/src/styles/adaptiveOtherFilePanel.css`
- Modify: any Task 1–9 UI file only when a verification failure proves a visual-contract defect.
- Create: `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md`

**Interfaces:**
- Consumes: all approved specification sections, the complete UI suite, current fixture project,
  current macOS development build, and the approved local visual references.
- Produces: no new runtime interface.
- Produces: one evidence document containing the exact commit, commands, state matrix, viewport
  matrix, accessibility checks, visual comparison findings, and resolved defects.

- [ ] **Step 1: Scan for obsolete palette and component remnants**

Run:

```bash
rg -n \
  'prefers-color-scheme: dark|content-toolbar|content-view-menu|settings-trigger|project-menu|SelectAllChoicePanel|#f3f4f6|#2563eb|#1d4ed8' \
  ui/src
```

Expected: no obsolete component selectors/imports and no superseded blue/cool-gray palette. Any
intentional match must be replaced with a semantic token or documented in the verification report.

Run:

```bash
rg -n 'box-shadow:' ui/src/styles
```

Expected: shadows only on popovers, menus, dialogs, preview images, floating navigation, task cards,
and trailing inspectors.

- [ ] **Step 2: Run static, type, component, build, policy, Rust, and security verification**

Run:

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm verify
```

Expected: every command exits 0. Fix only failures introduced by Tasks 1–9; rerun the smallest
failing command first, then all four commands.

- [ ] **Step 3: Launch the latest macOS development build**

Run:

```bash
pnpm start:viewer
```

Expected: the launcher replaces an older Viewer development process and opens the current worktree
build. Open `tests/fixtures/images` as the test project. Do not install, bundle, or test a Windows
version in this iteration.

- [ ] **Step 4: Verify the complete state and viewport matrix**

Inspect at both 1440×900 and 1024×720:

1. no-project resting, valid folder drag, invalid drop, and opening;
2. shell alignment with expanded, resized, and collapsed sidebar;
3. category bands, content grid, mixed select-all, selection summary, other-file panel, and drag
   target;
4. filter common/advanced states, grouped/flat results, empty search, progress, and pagination;
5. image preview at fit/100%/zoom, floating navigation, and unavailable image;
6. two-, four-, and twenty-image comparison, active pane, synchronized/independent transforms, and
   read-only controls;
7. Markdown, plain text, two-text side-by-side, encoding error, truncation, unsupported file, and
   information inspector;
8. held-pointer radial primary/secondary sectors plus secondary-click conventional fallback;
9. rename, batch rename with invalid preview, destination ready/conflict/blocked, Trash, settings,
   and close-operation dialogs;
10. scan/thumbnail/file-operation tasks, operation results, global recovery/error notices, local row
    error, and read-only strip.

For keyboard coverage, verify Tab order, `:focus-visible`, Escape, arrow navigation in both file
menus, Meta+F, Meta+A, Meta+I, preview navigation, and focus restoration. At 1024×720 verify that
filter popovers, context menus, dialogs, task cards, and inspectors remain inside the viewport
without covering their own critical actions.

- [ ] **Step 5: Compare implementation and references in one visual input**

Use the in-app browser already used for Viewer design review. Capture the approved reference and
implemented state at the same viewport, then create a two-column browser contact sheet and inspect
that single combined screenshot. Use these authoritative local references:

```text
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/visual-density-05-all-revised.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-content-browser-17.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-search-tasks-18.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-preview-compare-14.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-text-info-19.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-radial-reference-21.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-menus-dialogs-20.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-states-dialogs-15.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-launch-loading-22.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/integrated-empty-project-minimal-23.html
/Users/abc/Project/Viewer/.superpowers/brainstorm/76998-1785391406/content/visual-system-motion-16.html
```

Store temporary screenshots and contact sheets under `target/visual-qa/`; do not commit them. Check
column alignment, image cropping, text weight, padding, margins, borders, radii, shadows, control
density, menu geometry, dialog containment, and state hierarchy. Fix each visible mismatch, capture
again at the same viewport, and repeat the combined comparison until no material mismatch remains.

- [ ] **Step 6: Run the final full verification after visual fixes**

Run:

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm verify
```

Expected: every command exits 0 after the final visual correction.

- [ ] **Step 7: Commit any visual corrections**

```bash
git add ui/src
git diff --cached --check
git diff --cached --stat
```

If `git diff --cached --stat` lists UI changes, confirm it contains no `.superpowers`, `target`,
screenshots, generated build output, or unrelated fixture metadata, then run:

```bash
git commit -m "fix: resolve Viewer visual verification findings"
```

If the staged diff is empty, do not create an empty commit.

- [ ] **Step 8: Write the verification report against the verified UI commit**

Run:

```bash
git rev-parse HEAD
```

Create `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md`. Put the exact command
output after `Implementation commit:` and include these sections:

```markdown
# Viewer UI Visual Upgrade Verification

- Specification: `docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`
- Platform: macOS development build

## Automated verification
## State matrix
## Accessibility
## Visual comparison
## Remaining differences
```

Under automated verification, record each exact command, exit code, and final test count. Under the
state matrix, record Pass/Fail for every state in Task 10 Step 4 and the viewport inspected. Under
accessibility, record keyboard, focus, roles/names, reduced-motion, non-color cues, and
compact-target results. Under visual comparison, record every reference/implementation pair,
viewport, visible mismatch found, and corrective commit. Under remaining differences, write `None`
only after every material difference is fixed; otherwise list the exact approved exception and its
reason. Do not leave unresolved template markers in the saved document.

- [ ] **Step 9: Commit verification evidence**

```bash
git add docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md
git diff --cached --check
git commit -m "test: verify complete Viewer visual upgrade"
```

- [ ] **Step 10: Verify the finished branch is clean**

Run:

```bash
pnpm verify:clean
git status --short
```

Expected: `pnpm verify:clean` exits 0 and `git status --short` prints nothing.

---

## Plan Self-Review Checklist

- [ ] Every one of the ten approved visual-coverage sections maps to at least one implementation
  task and one verification state.
- [ ] Every new presentation component has a focused failing test before implementation.
- [ ] All bridge, controller, filesystem, search, selection, comparison, and operation safety
  semantics remain unchanged.
- [ ] The conventional context menu and radial menu consume the same `RadialMenuItem[]` model.
- [ ] No dark-theme override, component library, icon dependency, runtime dependency, decorative
  illustration, new workflow, or Windows implementation was introduced.
- [ ] The no-project resting state contains exactly the three approved elements.
- [ ] All test commands reference files that exist when the command is run.
- [ ] Every commit command stages only files owned by its task.
- [ ] The final visual QA uses same-viewport reference/implementation contact sheets, not isolated
  screenshots.
- [ ] The verification report contains no unresolved template marker and the branch is clean.
