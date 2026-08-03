# Viewer Atlas-to-Product Complete Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将已批准的 Viewer 完整视觉图谱迁移为正式 React/Tauri 产品组件，使审计中的 17 组、89 个状态全部使用同一套简约白色视觉系统，同时保留现有业务能力和即时交互语义。

**Architecture:** 先建立可测试的正式 UI primitives、静态真实图标资产和单一受控顶栏弹层状态，再按主工作区、查看内容、操作表面和可访问性四个产品波次迁移现有组件。业务 controller、hook、Tauri 命令和文件事务层保持原位；每波通过行为测试、组件测试、构建检查和两个规定视口的原生联合视觉对照后才进入下一波。

**Tech Stack:** React 19.2、TypeScript 6、CSS、Vitest 4、Testing Library、Biome 2、Vite 8、pnpm 10、Tauri 2、macOS 原生开发进程

## Global Constraints

- 视觉裁决顺序固定为：`docs/superpowers/specs/2026-07-30-viewer-complete-ui-visual-upgrade-design.md`、`docs/prototypes/viewer-complete-ui-visual-atlas.html`、现有业务规范与安全流程、当前产品代码。
- 实施设计固定为：`docs/superpowers/specs/2026-08-02-viewer-atlas-to-product-complete-migration-design.md`。
- 非遗漏台账固定为：`docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md` 中的 17 组、89 个唯一状态。
- 保留当前全部业务能力和即时交互语义；图谱中不存在业务实现的按钮、设置分类或页面不新增。
- 不修改 Rust crates、Tauri 命令、bridge 签名、搜索语义、选择语义、文件事务语义、预览安全预算、对比上限或关闭安全协议。
- 本轮只实现 macOS 开发版，但 Viewer 自有界面必须保持平台中性的白色视觉语言，不能引入 macOS 专属控件外观。
- 使用已批准强调色 `#5869CF`、4 px 间距网格、40 px 主顶栏、24 px 目录行、52 px 折叠轨道、52 px 查看工具栏、12 px popover 圆角和 14 px dialog 圆角。
- 正式图标固定使用 Lucide 1.27.0 的静态 SVG 最小子集；保留 ISC/MIT 许可文本，不增加运行时图标依赖，不使用 Unicode 字符、emoji、CSS 图形、手绘 SVG 或内联近似图形。
- Viewer 自有界面保持浅色；不新增主题切换器，不使用渐变，不用大面积阴影表达普通卡片层级。
- 缩略图选择只在缩略图内部显示 6 px 内缩、2 px 强调色描边；键盘焦点继续显示在卡片外侧，二者不得互相替代。
- 圆盘菜单保留现有 112/168 px 几何、180 ms 二级展开、8 px 手势阈值、指针/键盘模型和单一可见文件操作入口，只迁移视觉资产和状态表达。
- 筛选继续即时生效；底部 `完成` 只关闭弹层，不重新提交状态；顶栏计数不包含范围和排序。
- 任何图谱动作必须能映射到现有 prop、controller 方法或 Tauri 命令；无法映射时不渲染交互入口。
- 自动化测试不能替代原生视觉验收；最终证据必须来自唯一的当前仓库开发进程，并记录 commit、branch、dirty 状态和视口。
- 规定视口为 1024×720 与 1440×900；每个适用状态都要把参考与当前原生实现放入同一个联合对照输入后判断。
- 每个任务遵循红—绿—重构：先写失败测试、确认失败原因、实现最小迁移、运行相邻回归、再提交。
- 执行时先用 `superpowers:using-git-worktrees` 创建隔离工作树；不得把当前工作区的无关文件或 `target/` 视觉证据加入提交。

---

## File Structure

### New production files

- `scripts/vendor-viewer-icons.mjs` — 从 Lucide 1.27.0 官方 tag 获取固定图标子集并验证 SVG 内容。
- `ui/src/assets/icons/lucide/*.svg` — 仅包含 Viewer 实际使用的静态 Lucide SVG。
- `ui/src/assets/icons/lucide/LICENSE.txt` — Lucide ISC 与 Feather MIT 许可原文。
- `ui/src/components/ui/ViewerIcon.tsx` — 图标名称到静态资源的类型安全映射。
- `ui/src/components/ui/ViewerButton.tsx` — `ViewerButton` 与 `ViewerIconButton`。
- `ui/src/components/ui/ViewerPopover.tsx` — 受控 popover、外部点击、Escape 和焦点恢复。
- `ui/src/components/ui/ViewerMenuRow.tsx` — 普通、当前、禁用、信息和危险命令行。
- `ui/src/components/ui/ViewerStatusTag.tsx` — 信息、成功、警告和危险状态标签。
- `ui/src/components/ui/ViewerChoiceChip.tsx` — 可多选筛选 chip。
- `ui/src/components/ui/ViewerField.tsx` — 字段标签、说明、错误与原生控件容器。
- `ui/src/components/ui/ViewerDialog.tsx` — 正式对话框结构和现有焦点语义。
- `ui/src/components/ui/ViewerInspector.tsx` — 信息与操作结果共用右侧检查器壳。
- `ui/src/components/ui/ViewerTaskSurface.tsx` — 单一任务表面和自绘可见进度线。
- `ui/src/components/ui/ViewerEmptyState.tsx` — 短标题、说明和真实恢复动作。
- `ui/src/components/ui/ViewerLocalFeedback.tsx` — 局部 info/warning/danger/recovery 状态。
- `ui/src/components/ui/ViewerToolbar.tsx` — 图片、对比、文本和不支持文件共用三列工具栏。
- `ui/src/components/ui/ViewerSegmentedControl.tsx` — 变换、模式与选项的分段容器。
- `ui/src/styles/primitives.css` — 上述正式 primitives 的唯一结构和状态样式。
- `ui/src/app/useToolbarPopover.ts` — `filter | view | more | null` 的单一受控状态。
- `ui/src/visualMigrationCoverage.test.ts` — 审计、迁移台账与 89 个状态一一对应的防遗漏测试。
- `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md` — 每个状态的参考状态、原生进入方法、代码、测试、1024/1440 原生证据和结论。

### New test files

- `ui/src/components/ui/ViewerIcon.test.tsx`
- `ui/src/components/ui/ViewerControls.test.tsx`
- `ui/src/components/ui/ViewerPopover.test.tsx`
- `ui/src/components/ui/ViewerSurfaces.test.tsx`
- `ui/src/app/useToolbarPopover.test.tsx`
- `ui/src/styles/visualAccessibility.test.ts`

### Existing product files migrated by responsibility

- `ui/src/main.tsx`, `ui/src/styles/tokens.css`, `ui/src/styles/app.css`, `ui/src/styles/filePreviewExtensions.css`, `ui/src/styles/adaptiveOtherFilePanel.css` — 样式入口、token、正式 primitives 之后的模块布局与可访问性覆盖。
- `ui/src/App.tsx`, `ui/src/App.test.tsx` — 单一顶栏弹层状态、正式表面组合和所有业务回调保持。
- `ui/src/components/EmptyProject.tsx`, `WorkspaceLoadingState.tsx`, `AspectThumbnail.tsx` 及测试 — LAU 生命周期状态。
- `ui/src/components/FolderTree.tsx`, `FolderOverview.tsx`, `FolderFilmstripRow.tsx`, `ContentBrowser.tsx`, `AspectVirtualGrid.tsx`、`ui/src/components/contentBrowser/*` 及测试 — SID/STR/THU/OTH 主工作区。
- `ui/src/components/SearchToolbar.tsx`, `SearchResults.tsx`, `WorkspaceViewMenu.tsx`, `WorkspaceMoreMenu.tsx` 及测试 — SEA/FIL/MEN。
- `ui/src/components/radialMenuModel.ts`, `RadialFileMenu.tsx` 及测试 — RAD。
- `ui/src/components/ImagePreview.tsx`, `CompareWorkspace.tsx`, `ComparePane.tsx` 及测试 — PRE/COM。
- `ui/src/components/TextPreview.tsx`, `TextPreviewPane.tsx`, `UnsupportedFilePreview.tsx`, `UnsupportedFileState.tsx`, `InfoOverlay.tsx` 及测试 — DOC/INF。
- `ui/src/components/ModalSheet.tsx`, `SettingsDialog.tsx`, `RenameDialog.tsx`, `BatchRenameDialog.tsx`, `DestinationDialog.tsx`, `TrashConfirmation.tsx`, `CloseOperationDialog.tsx` 及测试 — DIA。
- `ui/src/components/TaskBar.tsx`, `OperationResults.tsx`, `GlobalNoticeStack.tsx`, `ReadOnlyBanner.tsx` 及测试 — TAS/RES。
- `THIRD_PARTY_NOTICES.md` — 静态 Lucide/Feather 许可和版本记录。
- `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md` — 最终命令证据和历史未验证项的关闭记录。

### Scope decomposition rationale

本计划不拆成互不关联的多个实施文档，因为所有产品波次共同依赖 Tasks 1–5 的正式 primitives、图标和顶栏状态边界，最终还必须进入同一个 89 状态原生门禁。Tasks 6–13 仍是可独立测试、独立审查、独立拒绝或批准的最小迁移单元；它们不能跳过 Wave 0，也不能各自建立视觉组件分支。

---

### Task 1: Establish the non-omission migration ledger

**Files:**
- Create: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Create: `ui/src/visualMigrationCoverage.test.ts`
- Read: `docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md`

**Interfaces:**
- Consumes: 审计表中 `LAU|SID|STR|THU|OTH|SEA|FIL|MEN|RAD|PRE|COM|DOC|INF|DIA|TAS|RES|A11Y` 前缀的 89 个 ID。
- Produces: 89 行唯一台账；列固定为 `ID | Wave | Reference state | Product owner | Native entry recipe | Automated evidence | Native 1024 | Native 1440 | Result`。

- [ ] **Step 1: Write the failing coverage contract**

Create `ui/src/visualMigrationCoverage.test.ts`:

```ts
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const statePattern = /^\| ((?:LAU|SID|STR|THU|OTH|SEA|FIL|MEN|RAD|PRE|COM|DOC|INF|DIA|TAS|RES|A11Y)-\d+) \|/gm

function ids(path: string): string[] {
  const source = readFileSync(resolve(import.meta.dirname, path), 'utf8')
  return [...source.matchAll(statePattern)].map((match) => match[1] as string)
}

describe('atlas-to-product migration coverage', () => {
  it('tracks every audited state exactly once', () => {
    const audit = ids('../../docs/reviews/2026-08-02-viewer-atlas-product-component-gap-audit.md')
    const ledger = ids('../../docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md')
    expect(audit).toHaveLength(89)
    expect(new Set(audit).size).toBe(89)
    expect(ledger).toHaveLength(89)
    expect(new Set(ledger).size).toBe(89)
    expect([...ledger].sort()).toEqual([...audit].sort())
  })
})
```

- [ ] **Step 2: Run the contract and verify the missing-ledger failure**

Run:

```bash
pnpm --dir ui exec vitest run src/visualMigrationCoverage.test.ts
```

Expected: FAIL with `ENOENT` for `viewer-atlas-product-migration-ledger.md`.

- [ ] **Step 3: Create all 89 ledger rows**

Create the ledger with these exact wave assignments:

```markdown
# Viewer Atlas-to-Product Migration Ledger

| ID | Wave | Reference state | Product owner | Native entry recipe | Automated evidence | Native 1024 | Native 1440 | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
```

Append every ID from the approved audit exactly once. Assign `LAU/SID/STR/THU/OTH/SEA/FIL/MEN` to Wave 1, `RAD/PRE/COM/DOC/INF` to Wave 2, `DIA/TAS/RES` to Wave 3 and `A11Y` to Wave 4. Copy the exact atlas state name into `Reference state`, set `Product owner` to the exact existing component named in the audit, and write a concrete `Native entry recipe` using only real project fixtures and existing commands. Each recipe must name the project/folder/file selection plus the click, keyboard, drag, permission, conflict or operation sequence that reaches the state; use `OS setting: forced colors`, `OS setting: reduce motion` and `Window: 200% / minimum size` for the three platform states. Set `Automated evidence`, `Native 1024` and `Native 1440` to `not-recorded`; set `Result` to `pending`.

- [ ] **Step 4: Verify exact coverage and commit the guard**

Run:

```bash
pnpm --dir ui exec vitest run src/visualMigrationCoverage.test.ts
```

Expected: PASS with 89 audit IDs, 89 unique ledger IDs and no extra rows.

Commit:

```bash
git add docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md ui/src/visualMigrationCoverage.test.ts
git commit -m "test: lock complete Viewer UI migration coverage"
```

---

### Task 2: Vendor the approved real icon subset

**Files:**
- Create: `scripts/vendor-viewer-icons.mjs`
- Create: `ui/src/assets/icons/lucide/*.svg`
- Create: `ui/src/assets/icons/lucide/LICENSE.txt`
- Create: `ui/src/components/ui/ViewerIcon.tsx`
- Create: `ui/src/components/ui/ViewerIcon.test.tsx`
- Modify: `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Produces: `export type ViewerIconName` and `ViewerIcon({ name, size, className })`.
- Produces: static assets only; `ui/package.json` runtime dependencies remain unchanged.

- [ ] **Step 1: Write the failing icon test**

```tsx
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import ViewerIcon, { VIEWER_ICON_NAMES } from './ViewerIcon'

describe('ViewerIcon', () => {
  it('renders the approved static asset without exposing duplicate speech', () => {
    render(<ViewerIcon name="search" size={16} data-testid="icon" />)
    const icon = screen.getByTestId('icon')
    expect(icon).toHaveAttribute('aria-hidden', 'true')
    expect(icon).toHaveAttribute('src', expect.stringContaining('search'))
    expect(icon).toHaveAttribute('width', '16')
    expect(icon).toHaveAttribute('height', '16')
  })

  it('publishes every icon name required by the migration', () => {
    expect(VIEWER_ICON_NAMES).toEqual([
      'alert-triangle', 'arrow-down', 'arrow-up', 'check', 'chevron-down',
      'chevron-left', 'chevron-right', 'chevron-up', 'circle', 'circle-dot',
      'columns-2', 'copy', 'ellipsis', 'eye', 'folder-input', 'folder-output',
      'grip-vertical', 'info', 'layout-grid', 'lock', 'minus', 'move', 'panel-right',
      'pencil', 'plus', 'refresh-cw', 'rotate-cw', 'search', 'settings',
      'sliders-horizontal', 'star', 'trash-2', 'x',
    ])
  })
})
```

- [ ] **Step 2: Run the focused test and verify the import failure**

Run `pnpm --dir ui exec vitest run src/components/ui/ViewerIcon.test.tsx`.

Expected: FAIL because `ViewerIcon.tsx` does not exist.

- [ ] **Step 3: Add a reproducible vendor script and fetch the exact assets**

Create `scripts/vendor-viewer-icons.mjs` with the exact 33-name array from the test and this fixed source boundary:

```js
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const version = '1.27.0'
const names = [
  'alert-triangle', 'arrow-down', 'arrow-up', 'check', 'chevron-down',
  'chevron-left', 'chevron-right', 'chevron-up', 'circle', 'circle-dot',
  'columns-2', 'copy', 'ellipsis', 'eye', 'folder-input', 'folder-output',
  'grip-vertical', 'info', 'layout-grid', 'lock', 'minus', 'move', 'panel-right',
  'pencil', 'plus', 'refresh-cw', 'rotate-cw', 'search', 'settings',
  'sliders-horizontal', 'star', 'trash-2', 'x',
]
const target = path.resolve(process.cwd(), 'ui/src/assets/icons/lucide')
const base = `https://raw.githubusercontent.com/lucide-icons/lucide/refs/tags/${version}/icons`

await mkdir(target, { recursive: true })
for (const name of names) {
  const response = await fetch(`${base}/${name}.svg`)
  if (!response.ok) throw new Error(`Lucide ${name}: HTTP ${response.status}`)
  const source = await response.text()
  if (!source.startsWith('<svg') || !source.includes('viewBox="0 0 24 24"')) {
    throw new Error(`Lucide ${name}: invalid SVG source`)
  }
  await writeFile(path.join(target, `${name}.svg`), source)
}
```

Run `node scripts/vendor-viewer-icons.mjs`, then copy the complete official Lucide 1.27.0 `LICENSE` file to `ui/src/assets/icons/lucide/LICENSE.txt` and add a `Bundled visual assets` row to `THIRD_PARTY_NOTICES.md` with version `1.27.0`, license `ISC and MIT for Feather-derived icons`, upstream `https://github.com/lucide-icons/lucide` and distributed `Yes`.

- [ ] **Step 4: Implement the typed static icon renderer**

`ViewerIcon.tsx` must export the exact tuple from Step 1, derive `ViewerIconName` from it and use this prop boundary:

```ts
export interface ViewerIconProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, 'src' | 'alt' | 'width' | 'height'> {
  name: ViewerIconName
  size?: number
}
```

Resolve assets through `import.meta.glob<string>('../../assets/icons/lucide/*.svg', { eager: true, query: '?url', import: 'default' })`, throw on a missing mapped asset and render an empty-alt `<img aria-hidden="true">`. It must not use `dangerouslySetInnerHTML`, `<svg>`, a Unicode fallback or a network URL.

- [ ] **Step 5: Verify the icon boundary and dependency graph**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ui/ViewerIcon.test.tsx
pnpm --dir ui typecheck
node scripts/check-npm-licenses.mjs
```

Expected: all commands PASS and `git diff -- ui/package.json pnpm-lock.yaml` is empty.

- [ ] **Step 6: Commit the icon source boundary**

```bash
git add scripts/vendor-viewer-icons.mjs ui/src/assets/icons/lucide ui/src/components/ui/ViewerIcon.tsx ui/src/components/ui/ViewerIcon.test.tsx THIRD_PARTY_NOTICES.md
git commit -m "feat: add approved Viewer icon assets"
```

---

### Task 3: Build the formal control primitives

**Files:**
- Create: `ui/src/components/ui/ViewerButton.tsx`
- Create: `ui/src/components/ui/ViewerChoiceChip.tsx`
- Create: `ui/src/components/ui/ViewerField.tsx`
- Create: `ui/src/components/ui/ViewerStatusTag.tsx`
- Create: `ui/src/components/ui/ViewerSegmentedControl.tsx`
- Create: `ui/src/components/ui/ViewerControls.test.tsx`
- Create: `ui/src/styles/primitives.css`
- Modify: `ui/src/main.tsx`

**Interfaces:**
- Produces: `ViewerButtonTone = 'primary' | 'secondary' | 'quiet' | 'danger'`.
- Produces: `ViewerButton`, `ViewerIconButton`, `ViewerChoiceChip`, `ViewerField`, `ViewerStatusTag`, `ViewerSegmentedControl`.
- Consumes: `ViewerIconName` and `ViewerIcon` from Task 2.

- [ ] **Step 1: Write failing behavior and semantic tests**

The test file must assert these exact behaviors:

```tsx
render(<ViewerButton tone="primary">完成</ViewerButton>)
expect(screen.getByRole('button', { name: '完成' })).toHaveAttribute('data-tone', 'primary')

render(<ViewerIconButton icon="x" label="关闭筛选" />)
expect(screen.getByRole('button', { name: '关闭筛选' })).toHaveClass('viewer-icon-button')

const change = vi.fn()
render(<ViewerChoiceChip checked={false} onCheckedChange={change}>JPEG</ViewerChoiceChip>)
fireEvent.click(screen.getByRole('checkbox', { name: 'JPEG' }))
expect(change).toHaveBeenCalledWith(true)

render(<ViewerStatusTag tone="warning">只读</ViewerStatusTag>)
expect(screen.getByText('只读')).toHaveAttribute('data-tone', 'warning')
```

Also assert that a loading button has `aria-busy="true"`, a disabled icon button cannot call its handler, every icon button has an accessible label, and the segmented wrapper exposes a named toolbar.

- [ ] **Step 2: Run the focused test and verify missing imports**

Run `pnpm --dir ui exec vitest run src/components/ui/ViewerControls.test.tsx`.

Expected: FAIL because the five primitive modules do not exist.

- [ ] **Step 3: Implement exact component contracts**

Use these public props without introducing a second state model:

```ts
export type ViewerButtonTone = 'primary' | 'secondary' | 'quiet' | 'danger'

export interface ViewerButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  tone?: ViewerButtonTone
  active?: boolean
  loading?: boolean
  leadingIcon?: ViewerIconName
}

export interface ViewerIconButtonProps
  extends Omit<ViewerButtonProps, 'children' | 'aria-label'> {
  icon: ViewerIconName
  label: string
}

export interface ViewerChoiceChipProps {
  checked: boolean
  disabled?: boolean
  children: ReactNode
  onCheckedChange(checked: boolean): void
}

export interface ViewerFieldProps {
  label: string
  hint?: string
  error?: string
  inline?: boolean
  children: ReactNode
}

export interface ViewerStatusTagProps {
  tone?: 'neutral' | 'info' | 'success' | 'warning' | 'danger'
  children: ReactNode
}

export interface ViewerSegmentedControlProps {
  label: string
  children: ReactNode
}
```

`ViewerChoiceChip` renders a real checkbox and visual span; the input may be visually hidden but remains focusable. `ViewerButton` maps `active` to `aria-pressed` only when the prop is explicitly supplied. `ViewerIconButton` always supplies `aria-label={label}`.

- [ ] **Step 4: Add the exact primitive visual rules**

In `primitives.css` implement 28–32 px ordinary controls, 34–36 px primary actions, 32 px icon targets, 8 px control radius, 2 px `focus-visible` outline with 2 px offset, `--viewer-accent` active state, quiet hover, disabled opacity without hiding labels and danger only on `data-tone="danger"`. Choice chips use a 1 px border, 8 px radius and `--viewer-accent-soft` checked surface. Do not add gradients or ordinary-card shadows.

Import order in `main.tsx` must be:

```ts
import './styles/tokens.css'
import './styles/primitives.css'
import './styles/app.css'
import './styles/filePreviewExtensions.css'
import './styles/adaptiveOtherFilePanel.css'
```

- [ ] **Step 5: Run focused, token and build checks**

```bash
pnpm --dir ui exec vitest run src/components/ui/ViewerControls.test.tsx src/styles/tokens.test.ts
pnpm --dir ui check
pnpm --dir ui build
```

Expected: all commands PASS.

- [ ] **Step 6: Commit the control layer**

```bash
git add ui/src/components/ui/ViewerButton.tsx ui/src/components/ui/ViewerChoiceChip.tsx ui/src/components/ui/ViewerField.tsx ui/src/components/ui/ViewerStatusTag.tsx ui/src/components/ui/ViewerSegmentedControl.tsx ui/src/components/ui/ViewerControls.test.tsx ui/src/styles/primitives.css ui/src/main.tsx
git commit -m "feat: add Viewer control primitives"
```

---

### Task 4: Build surfaces and centralize top-toolbar popover ownership

**Files:**
- Create: `ui/src/components/ui/ViewerPopover.tsx`
- Create: `ui/src/components/ui/ViewerMenuRow.tsx`
- Create: `ui/src/components/ui/ViewerPopover.test.tsx`
- Create: `ui/src/app/useToolbarPopover.ts`
- Create: `ui/src/app/useToolbarPopover.test.tsx`
- Modify: `ui/src/styles/primitives.css`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/SearchToolbar.tsx`
- Modify: `ui/src/components/WorkspaceViewMenu.tsx`
- Modify: `ui/src/components/WorkspaceMoreMenu.tsx`
- Modify: their three test files

**Interfaces:**
- Produces: `ToolbarPopover = 'filter' | 'view' | 'more' | null`.
- Produces: `useToolbarPopover(): { openPopover; setPopoverOpen; togglePopover; closePopover }`.
- Changes menu components from internal state to `open: boolean` and `onOpenChange(open: boolean): void`.

- [ ] **Step 1: Write the failing single-owner test**

```tsx
function Harness() {
  const toolbar = useToolbarPopover()
  return (
    <>
      <button onClick={() => toolbar.togglePopover('filter')}>筛选</button>
      <button onClick={() => toolbar.togglePopover('view')}>视图</button>
      <output>{toolbar.openPopover ?? 'none'}</output>
    </>
  )
}

render(<Harness />)
fireEvent.click(screen.getByRole('button', { name: '筛选' }))
expect(screen.getByText('filter')).toBeVisible()
fireEvent.click(screen.getByRole('button', { name: '视图' }))
expect(screen.getByText('view')).toBeVisible()
```

Add a `console.error` spy in `App.test.tsx`, open filter then view then more and assert no message contains `Cannot update a component while rendering a different component`.

- [ ] **Step 2: Verify the hook and controlled props are missing**

Run:

```bash
pnpm --dir ui exec vitest run src/app/useToolbarPopover.test.tsx src/App.test.tsx src/components/SearchToolbar.test.tsx src/components/WorkspaceViewMenu.test.tsx src/components/WorkspaceMoreMenu.test.tsx
```

Expected: FAIL because the hook and controlled props do not exist.

- [ ] **Step 3: Implement the popover state and surface contract**

```ts
export type ToolbarPopover = 'filter' | 'view' | 'more' | null

export function useToolbarPopover() {
  const [openPopover, setOpenPopover] = useState<ToolbarPopover>(null)
  const setPopoverOpen = (name: Exclude<ToolbarPopover, null>, open: boolean) =>
    setOpenPopover((current) => (open ? name : current === name ? null : current))
  const togglePopover = (name: Exclude<ToolbarPopover, null>) =>
    setOpenPopover((current) => (current === name ? null : name))
  const closePopover = () => setOpenPopover(null)
  return { openPopover, setPopoverOpen, togglePopover, closePopover }
}
```

`ViewerPopover` must accept `open`, `label`, `triggerRef`, `onOpenChange`, `align` and `children`; while open it listens for document `pointerdown` outside and `keydown` Escape, then calls `onOpenChange(false)` and restores `triggerRef.current` focus. `ViewerMenuRow` must accept `current`, `disabledReason`, `shortcut`, `tone`, `icon` and `onSelect`; current rows render visible text `当前`, disabled reasons remain readable and danger color appears only on hover/focus or final destructive action.

- [ ] **Step 4: Move all toolbar ownership into `App`**

In `ViewerWorkspace` create one `const toolbarPopover = useToolbarPopover()`. Pass:

```tsx
<SearchToolbar
  filterOpen={toolbarPopover.openPopover === 'filter'}
  onFilterOpenChange={(open) => toolbarPopover.setPopoverOpen('filter', open)}
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
```

Give `WorkspaceViewMenu` and `WorkspaceMoreMenu` equivalent controlled props. Replace `viewMenuOpenRequest` with `toolbarPopover.setPopoverOpen('view', true)`. Delete every `viewer-toolbar-popover` listener and every global custom-event dispatch.

- [ ] **Step 5: Verify mutual exclusion, outside dismissal and focus restoration**

Run the five focused suites from Step 2 and `pnpm --dir ui check`.

Expected: all PASS; `rg -n "viewer-toolbar-popover" ui/src` returns no matches.

- [ ] **Step 6: Commit the controlled popover boundary**

```bash
git add ui/src/components/ui/ViewerPopover.tsx ui/src/components/ui/ViewerMenuRow.tsx ui/src/components/ui/ViewerPopover.test.tsx ui/src/app/useToolbarPopover.ts ui/src/app/useToolbarPopover.test.tsx ui/src/styles/primitives.css ui/src/App.tsx ui/src/App.test.tsx ui/src/components/SearchToolbar.tsx ui/src/components/SearchToolbar.test.tsx ui/src/components/WorkspaceViewMenu.tsx ui/src/components/WorkspaceViewMenu.test.tsx ui/src/components/WorkspaceMoreMenu.tsx ui/src/components/WorkspaceMoreMenu.test.tsx
git commit -m "refactor: centralize Viewer toolbar popovers"
```

---

### Task 5: Build the formal dialog, inspector, task and content surfaces

**Files:**
- Create: `ui/src/components/ui/ViewerDialog.tsx`
- Create: `ui/src/components/ui/ViewerInspector.tsx`
- Create: `ui/src/components/ui/ViewerTaskSurface.tsx`
- Create: `ui/src/components/ui/ViewerEmptyState.tsx`
- Create: `ui/src/components/ui/ViewerLocalFeedback.tsx`
- Create: `ui/src/components/ui/ViewerToolbar.tsx`
- Create: `ui/src/components/ui/ViewerSurfaces.test.tsx`
- Modify: `ui/src/styles/primitives.css`
- Modify: `ui/src/components/ModalSheet.tsx`
- Modify: `ui/src/components/ModalSheet.test.tsx`

**Interfaces:**
- Produces: the remaining six formal surface components from the approved design.
- Preserves: `ModalSheet` import compatibility during module migration by making it a thin wrapper over `ViewerDialog`.

- [ ] **Step 1: Write failing surface tests**

Assert the following exact structures:

```tsx
render(<ViewerInspector label="文件信息" title="信息" onClose={vi.fn()}>内容</ViewerInspector>)
expect(screen.getByRole('complementary', { name: '文件信息' })).toHaveClass('viewer-inspector')
expect(screen.getByRole('button', { name: '关闭信息' })).toBeVisible()

render(<ViewerEmptyState title="此文件夹为空" description="这里还没有可查看的文件。" />)
expect(screen.getByRole('heading', { name: '此文件夹为空' })).toBeVisible()

render(<ViewerLocalFeedback tone="danger" title="无法载入预览">文件已移动。</ViewerLocalFeedback>)
expect(screen.getByRole('alert')).toHaveAttribute('data-tone', 'danger')

render(<ViewerToolbar label="图片预览" leading="A.jpg" center="控制" actions="完成" />)
expect(screen.getByRole('toolbar', { name: '图片预览' })).toHaveClass('viewer-toolbar')
```

Also preserve the existing `ModalSheet` focus trap, Escape handling and return focus test.

- [ ] **Step 2: Run the focused tests and verify missing surfaces**

Run `pnpm --dir ui exec vitest run src/components/ui/ViewerSurfaces.test.tsx src/components/ModalSheet.test.tsx`.

Expected: FAIL because the surface files are absent.

- [ ] **Step 3: Implement the public surface contracts**

Use semantic props only:

```ts
export interface ViewerDialogProps {
  title: string
  description?: string
  children: ReactNode
  footer?: ReactNode
  onCancel(): void
  initialFocusRef?: RefObject<HTMLElement | null>
  returnFocusRef?: RefObject<HTMLElement | null>
  destructive?: boolean
  size?: 'small' | 'medium' | 'large'
}

export interface ViewerInspectorProps {
  label: string
  title: string
  status?: ReactNode
  children: ReactNode
  footer?: ReactNode
  onClose(): void
}

export interface ViewerTaskSurfaceProps {
  label: string
  current: number
  total: number
  indeterminate?: boolean
  children: ReactNode
}

export interface ViewerEmptyStateProps {
  title: string
  description: string
  action?: ReactNode
}

export interface ViewerLocalFeedbackProps {
  tone: 'info' | 'warning' | 'danger' | 'recovery'
  title: string
  children: ReactNode
  action?: ReactNode
}

export interface ViewerToolbarProps {
  label: string
  leading: ReactNode
  center?: ReactNode
  actions: ReactNode
}
```

The visible task progress line is a styled `div` with `transform: scaleX(var(--viewer-progress))`; keep a native `<progress>` as visually hidden semantic output. `ViewerInspector` renders an `<aside role="complementary">`, aligns below the 40 px application header and uses the formal icon button.

- [ ] **Step 4: Implement the exact surface geometry**

Add to `primitives.css`: dialog radius 14 px, 18–20 px body padding, fixed footer action row; inspector width `clamp(320px, 24vw, 340px)`, `top: 40px`, `bottom: 0`, one left border and `--viewer-shadow-info-panel`; toolbar height 52 px with three grid columns; task surface 12 px radius and one popover shadow; empty/local feedback max-width 520 px and no ordinary-card shadow.

- [ ] **Step 5: Run surface, modal, check and build commands**

```bash
pnpm --dir ui exec vitest run src/components/ui/ViewerSurfaces.test.tsx src/components/ModalSheet.test.tsx
pnpm --dir ui check
pnpm --dir ui build
```

Expected: PASS.

- [ ] **Step 6: Commit the surface layer**

```bash
git add ui/src/components/ui/ViewerDialog.tsx ui/src/components/ui/ViewerInspector.tsx ui/src/components/ui/ViewerTaskSurface.tsx ui/src/components/ui/ViewerEmptyState.tsx ui/src/components/ui/ViewerLocalFeedback.tsx ui/src/components/ui/ViewerToolbar.tsx ui/src/components/ui/ViewerSurfaces.test.tsx ui/src/styles/primitives.css ui/src/components/ModalSheet.tsx ui/src/components/ModalSheet.test.tsx
git commit -m "feat: add Viewer surface primitives"
```

---

### Task 6: Migrate launch, opening, loading, empty, error and recovery states

**Files:**
- Modify: `ui/src/components/EmptyProject.tsx`, `EmptyProject.test.tsx`
- Modify: `ui/src/components/WorkspaceLoadingState.tsx`, `WorkspaceLoadingState.test.tsx`
- Modify: `ui/src/components/AspectThumbnail.tsx`, `AspectThumbnail.test.tsx`
- Modify: `ui/src/components/GlobalNoticeStack.tsx`, `GlobalNoticeStack.test.tsx`
- Modify: `ui/src/App.tsx`, `App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Update: migration ledger rows `LAU-01`–`LAU-09`

**Interfaces:**
- Consumes: `ViewerEmptyState`, `ViewerLocalFeedback`, `ViewerTaskSurface`, `ViewerButton`.
- Preserves: `ViewerBridge.chooseProject/openProject`, dropped-directory validation and scan/task state.

- [ ] **Step 1: Add failing lifecycle assertions**

Extend `EmptyProject.test.tsx` so invalid drop keeps the same fixed target geometry and exposes `data-drop-state="invalid"`; valid drag exposes `data-drop-state="valid"`; opening remains indeterminate. Extend `App.test.tsx` so an empty folder renders heading `此文件夹为空` rather than a naked paragraph, and recovery remains one global notice with the real result action.

- [ ] **Step 2: Verify the old structures fail**

Run:

```bash
pnpm --dir ui exec vitest run src/components/EmptyProject.test.tsx src/components/WorkspaceLoadingState.test.tsx src/components/AspectThumbnail.test.tsx src/components/GlobalNoticeStack.test.tsx src/App.test.tsx
```

Expected: FAIL on missing drop-state attributes and formal empty-state heading.

- [ ] **Step 3: Replace lifecycle presentation without changing commands**

Keep the resting no-project state at exactly `Viewer`, `选择或拖入一个项目文件夹`, and the primary button. Render transient valid/invalid drag feedback in the same inset target. Use `ViewerTaskSurface indeterminate` for opening/scanning when no real total exists. Replace the empty workspace paragraph with:

```tsx
<ViewerEmptyState
  title="此文件夹为空"
  description="这里还没有可查看的文件。"
/>
```

Use `ViewerLocalFeedback tone="danger"` for open/scan/local failures; do not add external-file-manager actions.

- [ ] **Step 4: Remove superseded lifecycle CSS and keep geometry stable**

Delete old standalone `.empty-project-error` card treatment and naked workspace-empty rules after all call sites migrate. Keep the no-project resting state visually strict, valid drag accent-soft, invalid drag danger-soft, and loading placeholders as solid-surface opacity animation only.

- [ ] **Step 5: Run lifecycle regressions and Wave 1 smoke build**

Run the focused suites from Step 2, then `pnpm --dir ui check` and `pnpm --dir ui build`.

Expected: PASS.

- [ ] **Step 6: Record automated evidence and commit**

Set `Automated evidence` for `LAU-01`–`LAU-09` to the exact passing suite names; leave native columns `not-recorded` until Task 14.

```bash
git add ui/src/components/EmptyProject.tsx ui/src/components/EmptyProject.test.tsx ui/src/components/WorkspaceLoadingState.tsx ui/src/components/WorkspaceLoadingState.test.tsx ui/src/components/AspectThumbnail.tsx ui/src/components/AspectThumbnail.test.tsx ui/src/components/GlobalNoticeStack.tsx ui/src/components/GlobalNoticeStack.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer lifecycle surfaces"
```

---

### Task 7: Migrate the application shell, sidebar, structure, thumbnails and other files

**Files:**
- Modify: `ui/src/App.tsx`, `App.test.tsx`
- Modify: `ui/src/components/FolderTree.tsx`, `FolderTree.test.tsx`
- Modify: `ui/src/components/FolderOverview.tsx`, `FolderOverview.test.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`, `FolderFilmstripRow.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`, `ContentBrowser.test.tsx`
- Modify: `ui/src/components/AspectVirtualGrid.tsx`, `AspectVirtualGrid.test.tsx`
- Modify: `ui/src/components/contentBrowser/ImageCell.tsx`
- Modify: `ui/src/components/contentBrowser/OrganizationDragHandle.tsx`
- Modify: `ui/src/components/contentBrowser/OtherFilePanel.tsx`, `OtherFilePanel.test.tsx`
- Modify: `ui/src/styles/app.css`, `ui/src/styles/adaptiveOtherFilePanel.css`
- Update: ledger rows `SID-01`–`SID-04`, `STR-01`–`STR-05`, `THU-01`–`THU-07`, `OTH-01`–`OTH-03`

**Interfaces:**
- Consumes: `ViewerIcon`, `ViewerIconButton`, `ViewerStatusTag`, `ViewerLocalFeedback`.
- Preserves: sidebar resize hook, virtual grid, selection model, drag model and direct/aggregate folder behavior.

- [ ] **Step 1: Add failing shell and selection contracts**

Assert that collapse/expand and folder disclosure controls contain `ViewerIcon` images and no `展开/收起/▾/▸` visible glyph; `ImageCell` selection remains only on `.image-cell-thumbnail-frame`; keyboard focus remains outside; other-file expansion uses a 32 px named icon button; legal and illegal drop targets expose both `data-drop-valid` and visible text.

- [ ] **Step 2: Run the browsing suites and verify icon/state failures**

Run:

```bash
pnpm --dir ui exec vitest run src/App.test.tsx src/components/FolderTree.test.tsx src/components/FolderOverview.test.tsx src/components/FolderFilmstripRow.test.tsx src/components/ContentBrowser.test.tsx src/components/AspectVirtualGrid.test.tsx src/components/contentBrowser/OtherFilePanel.test.tsx
```

Expected: FAIL where character controls and old drop indicators remain.

- [ ] **Step 3: Migrate controls while retaining approved geometry**

Use `chevron-left/right/down` for shell and tree disclosure, `grip-vertical` for organization drag and a quiet `chevron-up/down` icon button for other files. Preserve exact 40 px header, 220 px default sidebar, 52 px collapsed rail, 24 px rows, 200–420 px resize range and direct-to-grid content. Keep aggregate state as a small `ViewerStatusTag`; do not restore the atlas duplicate structure heading.

- [ ] **Step 4: Consolidate module CSS**

Delete rules that style raw character buttons. Keep filmstrip bands flat with one row divider, cards without ordinary shadows, selection inset `6px` and focus outline `2px`. Valid drop uses accent border plus text `移动到…`/`复制到…`; invalid drop uses danger border plus reason.

- [ ] **Step 5: Run browsing, layout and drag regressions**

Run Step 2 plus:

```bash
pnpm --dir ui exec vitest run src/layout/aspectLayout.test.ts src/state/useOrganizationPointerDrag.test.tsx src/components/contentBrowser/contentSelection.test.ts
pnpm --dir ui check
```

Expected: PASS with no selection, virtualization or drag behavior changes.

- [ ] **Step 6: Update ledger evidence and commit**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/FolderTree.tsx ui/src/components/FolderTree.test.tsx ui/src/components/FolderOverview.tsx ui/src/components/FolderOverview.test.tsx ui/src/components/FolderFilmstripRow.tsx ui/src/components/FolderFilmstripRow.test.tsx ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx ui/src/components/AspectVirtualGrid.tsx ui/src/components/AspectVirtualGrid.test.tsx ui/src/components/contentBrowser/ImageCell.tsx ui/src/components/contentBrowser/OrganizationDragHandle.tsx ui/src/components/contentBrowser/OtherFilePanel.tsx ui/src/components/contentBrowser/OtherFilePanel.test.tsx ui/src/styles/app.css ui/src/styles/adaptiveOtherFilePanel.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer browsing surfaces"
```

---

### Task 8: Migrate search, filter, view and more menus

**Files:**
- Modify: `ui/src/components/SearchToolbar.tsx`, `SearchToolbar.test.tsx`
- Modify: `ui/src/components/SearchResults.tsx`, `SearchResults.test.tsx`
- Modify: `ui/src/components/WorkspaceViewMenu.tsx`, `WorkspaceViewMenu.test.tsx`
- Modify: `ui/src/components/WorkspaceMoreMenu.tsx`, `WorkspaceMoreMenu.test.tsx`
- Modify: `ui/src/App.tsx`, `App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Update: ledger rows `SEA-01`–`SEA-05`, `FIL-01`–`FIL-04`, `MEN-01`–`MEN-03`

**Interfaces:**
- Consumes: Tasks 2–4 primitives and `SearchQueryModel` unchanged.
- Preserves: all file kinds, review states, orientation/range/time filters, grouped/flat search, pagination and immediate updates.

- [ ] **Step 1: Rewrite filter tests around the approved visible semantics**

Add assertions that opening filter renders choice-chip checkboxes, header close icon, scope and sort fields, advanced rules, enabled-condition removable chips, footer buttons `清除全部` and `完成`, and no button named `应用筛选`. Clicking JPEG must call `onFiltersChange` immediately; clicking `完成` must call only `onFilterOpenChange(false)`. Trigger count must ignore scope and sort.

For view/more tests assert current layout includes visible `当前`, read-only status has a status tag and disabled reason, and `关闭项目` is not danger-colored at rest through a semantic `data-danger-reveal="interaction"` hook.

- [ ] **Step 2: Run the search/menu suites and verify old controls fail**

```bash
pnpm --dir ui exec vitest run src/components/SearchToolbar.test.tsx src/components/SearchResults.test.tsx src/components/WorkspaceViewMenu.test.tsx src/components/WorkspaceMoreMenu.test.tsx src/App.test.tsx
```

Expected: FAIL because filter still uses raw checkbox rows and lacks `完成`.

- [ ] **Step 3: Migrate `SearchToolbar` to formal controls**

Use `ViewerIcon name="search"`, `ViewerPopover`, `ViewerField`, `ViewerChoiceChip`, `ViewerButton` and `ViewerIconButton`. Preserve the existing `filterChips()` and `toggleValue()` calculations. Use this footer behavior:

```tsx
<footer className="viewer-filter-footer">
  <ViewerButton tone="quiet" disabled={chips.length === 0} onClick={onClearFilters}>
    清除全部
  </ViewerButton>
  <ViewerButton tone="primary" onClick={() => onFilterOpenChange(false)}>
    完成
  </ViewerButton>
</footer>
```

The count remains `chips.length`; scope and sort are not inserted into that array.

- [ ] **Step 4: Migrate result rows and menus**

Give grouped and flat search the same result-row component structure: type/thumbnail, name, path, match context, metadata and `ViewerStatusTag`. Indexing renders compact status with real count, paging keeps a 40 px footer and empty search uses `ViewerEmptyState` with existing clear/return actions. Build view/more rows with `ViewerMenuRow`; do not add rescan, file-manager or unsupported external-open commands.

- [ ] **Step 5: Remove old form/menu selectors and run regressions**

Run Step 2, `pnpm --dir ui check` and `pnpm --dir ui build`. Then verify:

```bash
rg -n "viewer-toolbar-popover|⌕|↑|↓" ui/src/components/SearchToolbar.tsx ui/src/components/WorkspaceViewMenu.tsx ui/src/components/WorkspaceMoreMenu.tsx
```

Expected: no matches; all tests and builds PASS.

- [ ] **Step 6: Update ledger evidence and commit Wave 1 completion**

```bash
git add ui/src/components/SearchToolbar.tsx ui/src/components/SearchToolbar.test.tsx ui/src/components/SearchResults.tsx ui/src/components/SearchResults.test.tsx ui/src/components/WorkspaceViewMenu.tsx ui/src/components/WorkspaceViewMenu.test.tsx ui/src/components/WorkspaceMoreMenu.tsx ui/src/components/WorkspaceMoreMenu.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer search and menus"
```

---

### Task 9: Migrate the radial menu to real visual assets

**Files:**
- Modify: `ui/src/components/radialMenuModel.ts`, `radialMenuModel.test.ts`
- Modify: `ui/src/components/RadialFileMenu.tsx`, `RadialFileMenu.test.tsx`
- Modify: `ui/src/styles/app.css`
- Update: ledger rows `RAD-01`–`RAD-07`

**Interfaces:**
- Changes presentation field: `RadialMenuItem.symbol: string` becomes `RadialMenuItem.icon: ViewerIconName`.
- Preserves: every `RadialLeafAction`, enabled/disabled rule, geometry constant and event sequence.

- [ ] **Step 1: Write failing icon and geometry-preservation tests**

Assert the exact mapping:

```ts
expect(model.map((item) => [item.id, item.icon])).toEqual([
  ['preview', 'eye'],
  ['mark', 'star'],
  ['organize', 'folder-input'],
  ['trash', 'trash-2'],
  ['compare', 'columns-2'],
  ['info', 'info'],
])
```

Keep existing tests for 112/168 px, 30° secondary sectors, 180 ms expansion, 8 px threshold, release execution, center cancel, read-only and arrow-key navigation. Add an assertion that `.radial-menu-button` contains a `ViewerIcon` image and no raw symbol text.

- [ ] **Step 2: Run radial suites and verify the missing icon field**

Run:

```bash
pnpm --dir ui exec vitest run src/components/radialMenuModel.test.ts src/components/radialMenuGeometry.test.ts src/components/RadialFileMenu.test.tsx src/app/appSessionCoordinators.test.tsx
```

Expected: FAIL on the new `icon` expectations only; existing behavior remains green.

- [ ] **Step 3: Replace presentation symbols with the approved mapping**

Use `check`, `circle-dot`, `x`, `circle`, `star` for mark children and `pencil`, `copy`, `folder-output` for organize children. Render `ViewerIcon` and a real `check` icon for checked state. Mixed state must include visible text `混合`, not a `±` glyph. Disabled reason remains in the title and an on-surface text region.

- [ ] **Step 4: Preserve the prior round-menu visual character**

Keep the existing segmented annular SVG geometry because it is the product’s approved circular interaction surface; only its icons come from Lucide. Maintain white/soft surfaces, restrained divider strokes, accent active sector, danger only on trash interaction, upright labels, center selection count and subtle scrim. Do not convert the menu into a rectangular context menu.

- [ ] **Step 5: Run all radial and controller regressions**

Run Step 2 plus `pnpm --dir ui check` and `pnpm --dir ui build`.

Expected: PASS with unchanged actions and timing.

- [ ] **Step 6: Update ledger evidence and commit**

```bash
git add ui/src/components/radialMenuModel.ts ui/src/components/radialMenuModel.test.ts ui/src/components/RadialFileMenu.tsx ui/src/components/RadialFileMenu.test.tsx ui/src/styles/app.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer radial menu visuals"
```

---

### Task 10: Migrate image preview and comparison chrome

**Files:**
- Modify: `ui/src/components/ImagePreview.tsx`, `ImagePreview.test.tsx`
- Modify: `ui/src/components/CompareWorkspace.tsx`, `CompareWorkspace.test.tsx`
- Modify: `ui/src/components/ComparePane.tsx`, `ComparePane.test.tsx`
- Modify: `ui/src/styles/app.css`
- Update: ledger rows `PRE-01`–`PRE-07`, `COM-01`–`COM-04`

**Interfaces:**
- Consumes: `ViewerToolbar`, `ViewerSegmentedControl`, `ViewerButton`, `ViewerIconButton`, `ViewerLocalFeedback`.
- Preserves: fit/original/free modes, original safety fallback, pan/zoom/rotate, navigation, synchronization and comparison virtualization.

- [ ] **Step 1: Add failing shared-toolbar and local-state tests**

Assert image and compare both render a named `.viewer-toolbar`, visible `完成`, icon buttons for minus/plus/rotate and a stable stage-local loading/error surface. Loading must not replace toolbar geometry. Compare 2/3/4/many tests retain the same number of panes and active-pane semantics.

- [ ] **Step 2: Run viewing suites and verify old raw controls fail**

```bash
pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx src/components/CompareWorkspace.test.tsx src/components/ComparePane.test.tsx src/components/CompareVirtualViewport.test.tsx src/state/previewPolicy.test.ts src/state/comparePolicy.test.ts
```

Expected: FAIL on missing shared toolbar and icon components.

- [ ] **Step 3: Compose both screens from formal viewing primitives**

Map fit and 100% to text buttons inside `ViewerSegmentedControl`; map minus, plus and rotate to named icon buttons; preserve the read-only percentage text; render `ViewerButton tone="quiet"` with visible `完成`. Replace naked loading/error paragraphs with `ViewerLocalFeedback` inside the stable stage. Keep preview navigation as a compact floating control with real chevron icons and existing count.

- [ ] **Step 4: Consolidate viewer chrome CSS**

Delete duplicate `.preview-toolbar` and compare-only button visual rules after both use primitives. Keep 52 px toolbar, white chrome, 10–12 px pane gaps, one restrained stage boundary, active pane soft ring and no ordinary-card shadow. At 1024 px compress secondary metadata before allowing any toolbar wrap.

- [ ] **Step 5: Run viewing, layout, memory and build regressions**

Run Step 2 plus:

```bash
pnpm --dir ui exec vitest run src/components/compareLayoutEngine.test.ts src/components/useCompareLayout.test.tsx src/state/compareModel.test.ts
pnpm --dir ui check
pnpm --dir ui build
```

Expected: PASS.

- [ ] **Step 6: Update ledger evidence and commit**

```bash
git add ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx ui/src/components/CompareWorkspace.tsx ui/src/components/CompareWorkspace.test.tsx ui/src/components/ComparePane.tsx ui/src/components/ComparePane.test.tsx ui/src/styles/app.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer image viewing chrome"
```

---

### Task 11: Migrate text, unsupported-file and information surfaces

**Files:**
- Modify: `ui/src/components/TextPreview.tsx`, `TextPreview.test.tsx`
- Modify: `ui/src/components/TextPreviewPane.tsx`
- Modify: `ui/src/components/UnsupportedFilePreview.tsx`, `UnsupportedFilePreview.test.tsx`
- Modify: `ui/src/components/UnsupportedFileState.tsx`
- Modify: `ui/src/components/InfoOverlay.tsx`, `InfoOverlay.test.tsx`
- Modify: `ui/src/styles/filePreviewExtensions.css`, `ui/src/styles/app.css`
- Update: ledger rows `DOC-01`–`DOC-07`, `INF-01`–`INF-02`

**Interfaces:**
- Consumes: `ViewerToolbar`, `ViewerField`, `ViewerLocalFeedback`, `ViewerInspector`, `ViewerStatusTag`.
- Preserves: decoding, 10 MiB truncation, Markdown sanitization, external-link bridge for Markdown links and selection aggregation.

- [ ] **Step 1: Add failing document/inspector structure tests**

Assert text and unsupported previews share the 52 px formal toolbar and visible `完成`; encoding failure and truncation render local feedback; unsupported file exposes only name, type and support state; no external-open action appears. Assert single and multiple information use `role="complementary"`, title reflects selection count, close uses the real `x` icon and the panel begins at 40 px through the formal class.

- [ ] **Step 2: Run focused suites and verify old inspector failure**

```bash
pnpm --dir ui exec vitest run src/components/TextPreview.test.tsx src/components/UnsupportedFilePreview.test.tsx src/components/InfoOverlay.test.tsx
```

Expected: FAIL on formal toolbar/inspector expectations.

- [ ] **Step 3: Migrate document and unsupported states**

Compose `TextPreview` and `UnsupportedFilePreview` with `ViewerToolbar`; style native encoding select through `ViewerField`; place encoding and 10 MiB truncation messages in `ViewerLocalFeedback`. Keep reading width 760–820 px, plain text monospace, Markdown typography and dual-pane single divider. Do not add “在默认应用中打开”.

- [ ] **Step 4: Migrate information to the shared inspector**

Wrap existing `SingleFileInfo`, `AggregateSelectionInfo` and `FallbackMultiFileInfo` bodies in:

```tsx
<ViewerInspector
  label="文件信息"
  title={files.length > 1 ? `信息 · ${files.length} 项` : '信息'}
  onClose={onClose}
>
  {informationBody}
</ViewerInspector>
```

Use `ViewerStatusTag` for common review/favorite status while retaining text `混合` and `未选择`.

- [ ] **Step 5: Remove old 52 px inspector offset and run regressions**

Delete `.info-overlay { top: 52px; }` and superseded close-glyph rules. Run Step 2, `pnpm --dir ui check` and `pnpm --dir ui build`.

Expected: PASS and `rg -n "top:\s*52px" ui/src/styles` has no inspector/result match.

- [ ] **Step 6: Update ledger evidence and commit Wave 2 completion**

```bash
git add ui/src/components/TextPreview.tsx ui/src/components/TextPreview.test.tsx ui/src/components/TextPreviewPane.tsx ui/src/components/UnsupportedFilePreview.tsx ui/src/components/UnsupportedFilePreview.test.tsx ui/src/components/UnsupportedFileState.tsx ui/src/components/InfoOverlay.tsx ui/src/components/InfoOverlay.test.tsx ui/src/styles/filePreviewExtensions.css ui/src/styles/app.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer document and info surfaces"
```

---

### Task 12: Migrate every dialog to the formal hierarchy

**Files:**
- Modify: `ui/src/components/SettingsDialog.tsx`, `SettingsDialog.test.tsx`
- Modify: `ui/src/components/RenameDialog.tsx`, `RenameDialog.test.tsx`
- Modify: `ui/src/components/BatchRenameDialog.tsx`, `BatchRenameDialog.test.tsx`
- Modify: `ui/src/components/DestinationDialog.tsx`, `DestinationDialog.test.tsx`
- Modify: `ui/src/components/TrashConfirmation.tsx`, `TrashConfirmation.test.tsx`
- Modify: `ui/src/components/CloseOperationDialog.tsx`, `CloseOperationDialog.test.tsx`
- Modify: `ui/src/styles/app.css`
- Update: ledger rows `DIA-01`–`DIA-07`

**Interfaces:**
- Consumes: `ViewerDialog`, `ViewerButton`, `ViewerField`, `ViewerChoiceChip`, `ViewerStatusTag`, `ViewerLocalFeedback`.
- Preserves: every existing validation, preview, preflight, conflict decision, busy state and callback.

- [ ] **Step 1: Add failing action-hierarchy and settings-shell tests**

Assert every dialog has a formal footer, cancellation before primary/destructive action, one `data-tone="primary"` safe action or one `data-tone="danger"` final destructive action, and no ordinary final action. Settings must render a two-column shell with one real navigation item `显示与外观`; assert `浏览`, `文件操作` and `快捷键` are absent. Close-operation must make `等待完成后关闭` primary and preserve the other two choices.

- [ ] **Step 2: Run all dialog suites and verify old button hierarchy fails**

```bash
pnpm --dir ui exec vitest run src/components/ModalSheet.test.tsx src/components/SettingsDialog.test.tsx src/components/RenameDialog.test.tsx src/components/BatchRenameDialog.test.tsx src/components/DestinationDialog.test.tsx src/components/TrashConfirmation.test.tsx src/components/CloseOperationDialog.test.tsx
```

Expected: FAIL on formal footer/tone assertions while existing behavior assertions remain green.

- [ ] **Step 3: Migrate settings, rename and batch rename**

Use the graphically two-column settings shell but render only the real density setting. Use `ViewerField` for rename fields and rule inputs, `ViewerLocalFeedback` for validation error, `ViewerButton tone="primary"` for executable rename, and disable it with visible reason when invalid or busy. Preserve extension editing and async preview revision handling.

- [ ] **Step 4: Migrate destination, conflict, trash and close dialogs**

Keep destination tree and preflight summary in two columns. Express ready/blocked/conflict with `ViewerStatusTag` plus text. Preserve per-item and apply-remaining conflict policies. Trash uses danger only on `移到废纸篓`; close uses primary `等待完成后关闭`, secondary `停留在当前项目`, and an explicit non-primary cancellation choice for pending operations.

- [ ] **Step 5: Remove dialog-specific duplicate controls and run regressions**

Run Step 2 plus `pnpm --dir ui check` and `pnpm --dir ui build`.

Expected: PASS at 1024 DOM width with all footer actions still present.

- [ ] **Step 6: Update ledger evidence and commit dialogs**

```bash
git add ui/src/components/SettingsDialog.tsx ui/src/components/SettingsDialog.test.tsx ui/src/components/RenameDialog.tsx ui/src/components/RenameDialog.test.tsx ui/src/components/BatchRenameDialog.tsx ui/src/components/BatchRenameDialog.test.tsx ui/src/components/DestinationDialog.tsx ui/src/components/DestinationDialog.test.tsx ui/src/components/TrashConfirmation.tsx ui/src/components/TrashConfirmation.test.tsx ui/src/components/CloseOperationDialog.tsx ui/src/components/CloseOperationDialog.test.tsx ui/src/styles/app.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer dialogs"
```

---

### Task 13: Migrate tasks, operation results, notices and read-only feedback

**Files:**
- Modify: `ui/src/components/TaskBar.tsx`, `TaskBar.test.tsx`
- Modify: `ui/src/components/OperationResults.tsx`, `OperationResults.test.tsx`
- Modify: `ui/src/components/GlobalNoticeStack.tsx`, `GlobalNoticeStack.test.tsx`
- Modify: `ui/src/components/ReadOnlyBanner.tsx`, `ReadOnlyBanner.test.tsx`
- Modify: `ui/src/App.tsx`, `App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Update: ledger rows `TAS-01`–`TAS-05`, `RES-01`–`RES-05`

**Interfaces:**
- Consumes: `ViewerTaskSurface`, `ViewerInspector`, `ViewerLocalFeedback`, `ViewerStatusTag`, buttons/icons.
- Preserves: task aggregation, cancel/dismiss/result callbacks, 2 second clean-success dismissal, paging, undo/recovery and permission actions.

- [ ] **Step 1: Add failing task/result semantic tests**

Assert task disclosure uses real chevrons, visible progress is formal while semantic `<progress>` remains, clean success auto-dismisses at 2000 ms, failure/cancel/result states stay until handled, and only one `.task-surface` exists. Assert operation results use `ViewerInspector`, align below 40 px, show semantic total tags and preserve 200-row pagination. Notices use formal close icons and named actions; read-only remains a 38 px strip with visible reason.

- [ ] **Step 2: Run task and feedback suites**

```bash
pnpm --dir ui exec vitest run src/components/TaskBar.test.tsx src/components/OperationResults.test.tsx src/components/GlobalNoticeStack.test.tsx src/components/ReadOnlyBanner.test.tsx src/App.test.tsx
```

Expected: FAIL on old progress, glyph and inspector structures.

- [ ] **Step 3: Migrate the single task surface**

Wrap summary and expanded rows in `ViewerTaskSurface`. Replace `▾/▸` with `chevron-down/right`. Keep exact `finishedCount`, `isCleanSuccess` and timer logic. Present failure/skipped/cancelled counts with status tags and preserve `取消任务`, `关闭任务`, `查看结果` callbacks.

- [ ] **Step 4: Migrate results and feedback without duplicating state**

Compose `OperationResults` with `ViewerInspector`; use `ViewerStatusTag` for completed/failed/skipped/cancelled counts and preserve `PAGE_SIZE = 200`. Compose notices and local errors with `ViewerLocalFeedback`; retain `role="alert"` for danger and `role="status"` elsewhere. Keep the read-only banner’s real permission and reselection actions.

- [ ] **Step 5: Remove superseded task/inspector CSS and run regressions**

Run Step 2, `pnpm --dir ui check` and `pnpm --dir ui build`. Verify no visible native progress styling remains and no `.operation-results { top: 52px; }` rule remains.

Expected: PASS.

- [ ] **Step 6: Update ledger evidence and commit Wave 3 completion**

```bash
git add ui/src/components/TaskBar.tsx ui/src/components/TaskBar.test.tsx ui/src/components/OperationResults.tsx ui/src/components/OperationResults.test.tsx ui/src/components/GlobalNoticeStack.tsx ui/src/components/GlobalNoticeStack.test.tsx ui/src/components/ReadOnlyBanner.tsx ui/src/components/ReadOnlyBanner.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: migrate Viewer task and feedback surfaces"
```

---

### Task 14: Close accessibility, forced-colors, viewport and source-cleanup gaps

**Files:**
- Modify: `ui/src/styles/primitives.css`, `ui/src/styles/app.css`, `ui/src/styles/filePreviewExtensions.css`, `ui/src/styles/adaptiveOtherFilePanel.css`
- Modify: `ui/src/styles/app.test.ts`, `ui/src/styles/tokens.test.ts`
- Create: `ui/src/styles/visualAccessibility.test.ts`
- Modify: `ui/src/components/ui/ViewerControls.test.tsx`, `ViewerPopover.test.tsx`, `ViewerSurfaces.test.tsx`
- Modify: `ui/src/components/SearchToolbar.test.tsx`, `WorkspaceViewMenu.test.tsx`, `WorkspaceMoreMenu.test.tsx`
- Modify: `ui/src/components/RadialFileMenu.test.tsx`, `ImagePreview.test.tsx`, `InfoOverlay.test.tsx`
- Modify: `ui/src/components/ModalSheet.test.tsx`, `SettingsDialog.test.tsx`, `TaskBar.test.tsx`
- Modify: `ui/src/visualMigrationCoverage.test.ts`
- Update: ledger rows `A11Y-01`–`A11Y-05`

**Interfaces:**
- Consumes: all migrated formal components.
- Produces: non-color focus/selection/danger/disabled states under forced colors, reduced-motion behavior and 200%/minimum-window reachability.

- [ ] **Step 1: Write failing accessibility and source-cleanup contracts**

Add component tests for logical Tab order, Escape/focus restoration, named icon buttons and structural state cues. In `visualAccessibility.test.ts`, add style assertions that `@media (forced-colors: active)` exists and sets outlines/borders for selected thumbnail, current menu row, danger action, disabled choice and focus-visible. Build the production source input and fail on these visible glyphs:

```ts
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { resolve } from 'node:path'

function sourceFiles(root: string): string[] {
  return readdirSync(root).flatMap((entry) => {
    const path = resolve(root, entry)
    if (statSync(path).isDirectory()) return sourceFiles(path)
    return /\.(ts|tsx)$/.test(path) && !path.endsWith('.test.ts') && !path.endsWith('.test.tsx')
      ? [path]
      : []
  })
}

const productionSource = sourceFiles(resolve(import.meta.dirname, '..'))
  .map((path) => readFileSync(path, 'utf8'))
  .join('\n')
const forbiddenGlyphs = ['⌕', '↻', '▾', '▸', '⇄', 'ⓘ', '◉', '✎', '⧉', '⌫', '★']
for (const glyph of forbiddenGlyphs) expect(productionSource).not.toContain(glyph)
```

Do not forbid mathematical `×`, `+` or `−` inside genuine dimensions/calculations; only assert their old button/model call sites are removed.

- [ ] **Step 2: Run the accessibility/style suites and verify red state**

```bash
pnpm --dir ui exec vitest run src/components/ui src/styles/app.test.ts src/styles/tokens.test.ts src/visualMigrationCoverage.test.ts
```

Expected: FAIL until forced-colors and source cleanup are complete.

- [ ] **Step 3: Add exact forced-colors and reduced-motion rules**

Inside `@media (forced-colors: active)`, use `Canvas`, `CanvasText`, `Highlight`, `HighlightText` and `GrayText`; keep selected/current/focus states structurally distinct through border/outline, not color alone. Inside `@media (prefers-reduced-motion: reduce)`, set primitive transitions and animations to `none`; retain content visibility and task progress semantics.

- [ ] **Step 4: Add 200% and viewport regression fixtures**

In component tests set `window.innerWidth`/`innerHeight` to 1024×720 and verify filter, menu, dialog, inspector, task and preview controls remain in the document and named. Repeat with CSS-pixel viewport 720×450 as the automated approximation for 200% zoom; this is a regression guard, not the native evidence.

- [ ] **Step 5: Remove the legacy CSS rail**

For every migrated component, search its former class selectors, remove rules with no remaining JSX call site and keep only module layout extensions. Run:

```bash
rg -n "viewer-toolbar-popover|⌕|↻|▾|▸|⇄|ⓘ|◉|✎|⧉|⌫|★" ui/src --glob '!*.test.*'
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
```

Expected: the glyph search has no UI-symbol matches; check, all UI tests and build PASS.

- [ ] **Step 6: Update automated A11Y evidence and commit Wave 4 code completion**

```bash
git add ui/src/styles/primitives.css ui/src/styles/app.css ui/src/styles/filePreviewExtensions.css ui/src/styles/adaptiveOtherFilePanel.css ui/src/styles/app.test.ts ui/src/styles/tokens.test.ts ui/src/styles/visualAccessibility.test.ts ui/src/components/ui/ViewerControls.test.tsx ui/src/components/ui/ViewerPopover.test.tsx ui/src/components/ui/ViewerSurfaces.test.tsx ui/src/components/SearchToolbar.test.tsx ui/src/components/WorkspaceViewMenu.test.tsx ui/src/components/WorkspaceMoreMenu.test.tsx ui/src/components/RadialFileMenu.test.tsx ui/src/components/ImagePreview.test.tsx ui/src/components/InfoOverlay.test.tsx ui/src/components/ModalSheet.test.tsx ui/src/components/SettingsDialog.test.tsx ui/src/components/TaskBar.test.tsx ui/src/visualMigrationCoverage.test.ts docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md
git commit -m "feat: complete Viewer UI accessibility migration"
```

---

### Task 15: Execute the 89-state native visual gate and close the migration

**Files:**
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify: `docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md`
- Generated but not staged: `target/atlas-product-migration-acceptance/<commit>/**`

**Interfaces:**
- Consumes: the unique current native Viewer process launched by `pnpm start:viewer`, the atlas reference and every ledger row.
- Produces: exact native 1024×720 and 1440×900 evidence paths plus P0/P1/P2 conclusion for all 89 states.

- [ ] **Step 1: Establish a clean, uniquely identified acceptance build**

Run:

```bash
git status --short
git rev-parse --abbrev-ref HEAD
git rev-parse HEAD
pnpm start:viewer
```

Expected: clean worktree before launch, one reported Viewer PID and exactly one current-repository native development process. Record branch, commit, dirty state, PID, macOS version and display scale at the top of the migration ledger.

- [ ] **Step 2: Run the complete automated gate**

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm test:policy
pnpm security
```

Expected: all commands exit 0. Record command, exit code and test count in the verification document.

- [ ] **Step 3: Capture every state at 1024×720**

For each of the 89 ledger IDs, put the atlas/reference state and the current native product state at 1024×720 into one combined comparison input. Save raw and combined evidence under `target/atlas-product-migration-acceptance/<commit>/1024x720/<ID>/`. Record the exact combined path in `Native 1024`. Judge typography, padding, alignment, radius, border, cropping, button hierarchy, focus and enabled/disabled state. Fix and recapture every P0/P1/P2 before marking the cell `pass`.

- [ ] **Step 4: Capture every state at 1440×900**

Repeat Step 3 at 1440×900 and record the exact combined path in `Native 1440`. Use the same business state, selection, menu/dialog state and fixture content as the 1024 pass; different fixture images are allowed only when structure is unchanged and the variance is documented.

- [ ] **Step 5: Perform manual accessibility and platform checks**

Verify keyboard-only navigation, Escape and focus return for filter/view/more, preview, radial menu, all dialogs and both inspectors. Verify reduced motion, forced colors structure, macOS dark system appearance with Viewer-owned light surfaces, 200% zoom and minimum window reachability. Record pass/fail against `A11Y-01`–`A11Y-05`.

- [ ] **Step 6: Close the ledger only when every gate is satisfied**

For each row set `Result` to `pass` only when automated evidence and both native columns are recorded and P0/P1/P2 are zero. Keep any unresolved row as `pending` or `blocked` with a concrete reason. Update the verification document so historical screenshots are explicitly non-current and the new commit is the only completion evidence.

- [ ] **Step 7: Commit the final evidence index**

```bash
git add docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/reviews/2026-07-30-viewer-ui-visual-upgrade-verification.md
git commit -m "docs: verify complete Viewer UI migration"
```

Do not stage `target/atlas-product-migration-acceptance/`; the review documents retain exact local evidence paths and conclusions.

---

## Coverage Map

| Task | Audit IDs | Count |
| --- | --- | ---: |
| Task 6 | LAU-01–LAU-09 | 9 |
| Task 7 | SID-01–SID-04, STR-01–STR-05, THU-01–THU-07, OTH-01–OTH-03 | 19 |
| Task 8 | SEA-01–SEA-05, FIL-01–FIL-04, MEN-01–MEN-03 | 12 |
| Task 9 | RAD-01–RAD-07 | 7 |
| Task 10 | PRE-01–PRE-07, COM-01–COM-04 | 11 |
| Task 11 | DOC-01–DOC-07, INF-01–INF-02 | 9 |
| Task 12 | DIA-01–DIA-07 | 7 |
| Task 13 | TAS-01–TAS-05, RES-01–RES-05 | 10 |
| Task 14 | A11Y-01–A11Y-05 | 5 |
| **Total** | **17 groups** | **89** |

## Design-spec coverage self-check

| Approved design section | Implementation owner |
| --- | --- |
| Authority, conflict decisions, scope and non-goals | Global Constraints; enforced again in Tasks 8, 9, 11 and 12 |
| Formal component architecture and icon boundary | Tasks 2–5 |
| Button, field, chip, segmented and status rules | Task 3 |
| Popover, menu and single toolbar state rules | Task 4 |
| Dialog, inspector, task, empty, feedback and viewing-toolbar rules | Task 5, then product migrations in Tasks 10–13 |
| Wave 1 main workspace/search/menu states | Tasks 6–8 |
| Wave 2 radial/viewing/document/information states | Tasks 9–11 |
| Wave 3 dialog/task/result states | Tasks 12–13 |
| Accessibility, reduced motion, forced colors and zoom | Task 14 |
| Automated, native visual and completion definitions | Tasks 1, 14 and 15 |
| CSS dual-track, behavior rewrite and partial-completion risks | Per-task old-selector removal, exact staging and ledger gates |

## Required execution order

Tasks 1–5 are Wave 0 and must complete in order. Tasks 6–8 are Wave 1; Task 6 and Task 7 may be reviewed independently after Wave 0, but Task 8 depends on the controlled popover work. Tasks 9–11 are Wave 2 and may be reviewed independently after Wave 0. Tasks 12–13 are Wave 3. Task 14 starts only after Tasks 6–13 have removed their legacy selectors. Task 15 is the final gate and cannot be replaced by browser-only atlas tests or historical screenshots.
