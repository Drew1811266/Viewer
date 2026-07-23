# Viewer Folder Filmstrip Rows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the numbered-folder card grid with one full-width, independently scrolling image filmstrip per folder, with single-click preview and folder-name navigation.

**Architecture:** Keep the existing category projection for row metadata, then lazily call the existing non-aggregate `queryFolder` bridge for each row as it nears the viewport. `FolderOverview` owns an entity-id keyed promise cache, `FolderFilmstripRow` owns visibility and row UI state, and `App` stores either the active content workspace or an explicit filmstrip image list as the preview context.

**Tech Stack:** React 19, TypeScript 6, CSS, Vitest 4, Testing Library, existing Tauri `ViewerBridge`

## Global Constraints

- Each numbered folder occupies one full-width horizontal row.
- Each row has its own horizontal image scroller and shows every direct image in that folder.
- Clicking a thumbnail previews that image without leaving the category overview.
- Clicking the folder number or name enters that folder.
- Preview navigation stays within the images from the row that opened it.
- Load a folder only when its row approaches the viewport.
- Keep the fixed identity panel visible while the row scrolls horizontally.
- Retrying a failed row replaces only that row's failed cached request.
- Right-click file actions are not added to the overview.
- Backend contracts and dependencies remain unchanged.

---

## File Map

- Create `ui/src/components/FolderFilmstripRow.tsx`: row visibility detection, loading/error/empty states, folder navigation, image buttons, and thumbnail loading.
- Create `ui/src/components/FolderFilmstripRow.test.tsx`: focused row behavior, accessibility, lazy loading, preview context, and retry coverage.
- Modify `ui/src/components/FolderOverview.tsx`: vertical row list and per-overview request deduplication cache.
- Modify `ui/src/components/FolderOverview.test.tsx`: source order, folder navigation, metadata, and cache lifetime coverage.
- Modify `ui/src/App.tsx`: child-folder image request callback and explicit filmstrip preview session.
- Modify `ui/src/App.test.tsx`: category-overview preview, row-bounded navigation, focus restoration, and external-refresh invalidation.
- Modify `ui/src/styles/app.css`: replace card-grid rules with fixed-identity filmstrip-row rules, loading/error states, and dark-mode colors.
- Modify `ui/src/styles/app.test.ts`: enforce the fixed identity column and per-row horizontal overflow contract.

### Task 1: Build the lazy filmstrip row

**Files:**
- Create: `ui/src/components/FolderFilmstripRow.tsx`
- Create: `ui/src/components/FolderFilmstripRow.test.tsx`

**Interfaces:**
- Consumes: `ContentFolderCard`, `BrowserFile`, and `loadImages(entityId: string, retry?: boolean): Promise<BrowserFile[]>`.
- Produces: default `FolderFilmstripRow` component with `onSelect(entityId: string)` and `onPreview(file: BrowserFile, files: BrowserFile[])`.

- [ ] **Step 1: Write the failing row tests**

Create `ui/src/components/FolderFilmstripRow.test.tsx` with:

```tsx
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ContentFolderCard } from '../api/types'
import FolderFilmstripRow from './FolderFilmstripRow'

const folder: ContentFolderCard = {
  entityId: 'folder-b01',
  relativePath: '角色/B01',
  name: 'B01',
  marker: { reviewState: null, favorite: false },
  imageCount: 2,
  textCount: 1,
  reviewProgress: {
    total: 3,
    keep: 1,
    pending: 0,
    reject: 0,
    unmarked: 2,
    favorite: 0,
  },
  representativeImages: [],
}

const images: BrowserFile[] = ['front', 'side'].map((name, index) => ({
  entityId: `image-${index + 1}`,
  relativePath: `角色/B01/${name}.jpg`,
  name: `${name}.jpg`,
  kind: 'jpeg',
  size: 100,
  modifiedNs: String(index + 1),
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
}))

let intersectionCallback: IntersectionObserverCallback | null = null
const originalIntersectionObserver = globalThis.IntersectionObserver

afterEach(() => {
  intersectionCallback = null
  globalThis.IntersectionObserver = originalIntersectionObserver
})

function installIntersectionObserver() {
  class TestIntersectionObserver {
    readonly root = null
    readonly rootMargin = '240px 0px'
    readonly thresholds = [0]

    constructor(callback: IntersectionObserverCallback) {
      intersectionCallback = callback
    }

    disconnect() {}
    observe() {}
    takeRecords(): IntersectionObserverEntry[] {
      return []
    }
    unobserve() {}
  }
  globalThis.IntersectionObserver =
    TestIntersectionObserver as unknown as typeof IntersectionObserver
}

function revealRow() {
  intersectionCallback?.(
    [{ isIntersecting: true } as IntersectionObserverEntry],
    {} as IntersectionObserver,
  )
}

describe('FolderFilmstripRow', () => {
  it('loads only after approaching the viewport and previews in returned image order', async () => {
    installIntersectionObserver()
    const loadImages = vi.fn().mockResolvedValue(images)
    const preview = vi.fn()
    render(
      <FolderFilmstripRow
        folder={folder}
        loadImages={loadImages}
        requestThumbnail={vi.fn().mockResolvedValue('viewer-image://thumbnail')}
        onSelect={vi.fn()}
        onPreview={preview}
      />,
    )

    expect(loadImages).not.toHaveBeenCalled()
    expect(screen.getByRole('region', { name: 'B01 图片' })).toHaveAttribute(
      'data-state',
      'idle',
    )

    act(revealRow)

    const filmstrip = await screen.findByRole('region', { name: 'B01 图片' })
    await within(filmstrip).findByRole('button', { name: '预览 front.jpg' })
    const imageButtons = within(filmstrip).getAllByRole('button', { name: /^预览 / })
    expect(imageButtons.map((button) => button.getAttribute('aria-label'))).toEqual([
      '预览 front.jpg',
      '预览 side.jpg',
    ])
    fireEvent.click(imageButtons[1]!)

    expect(loadImages).toHaveBeenCalledWith('folder-b01', false)
    expect(preview).toHaveBeenCalledWith(images[1], images)
  })

  it('keeps metadata available while loading and opens the folder from its identity button', () => {
    const select = vi.fn()
    render(
      <FolderFilmstripRow
        folder={folder}
        loadImages={() => new Promise<BrowserFile[]>(() => undefined)}
        onSelect={select}
        onPreview={vi.fn()}
      />,
    )

    expect(screen.getByText('2 张图片')).toBeVisible()
    expect(screen.getByText('1 个文本')).toBeVisible()
    expect(screen.getByText('文件夹：未标记')).toBeVisible()
    expect(screen.getByText('已审阅 1 / 3')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '打开 B01' }))
    expect(select).toHaveBeenCalledWith('folder-b01')
  })

  it('renders empty rows and retries only the failed row request', async () => {
    const loadImages = vi
      .fn<(entityId: string, retry?: boolean) => Promise<BrowserFile[]>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce([])
    render(
      <FolderFilmstripRow
        folder={folder}
        loadImages={loadImages}
        onSelect={vi.fn()}
        onPreview={vi.fn()}
      />,
    )

    expect(await screen.findByRole('alert')).toHaveTextContent('无法加载图片')
    fireEvent.click(screen.getByRole('button', { name: '重试 B01' }))

    await waitFor(() => expect(screen.getByText('无图片')).toBeVisible())
    expect(loadImages).toHaveBeenNthCalledWith(1, 'folder-b01', false)
    expect(loadImages).toHaveBeenNthCalledWith(2, 'folder-b01', true)
  })
})
```

- [ ] **Step 2: Run the row test to verify it fails**

Run:

```bash
pnpm --dir ui exec vitest run src/components/FolderFilmstripRow.test.tsx
```

Expected: FAIL because `./FolderFilmstripRow` does not exist.

- [ ] **Step 3: Implement the row and thumbnail components**

Create `ui/src/components/FolderFilmstripRow.tsx`:

```tsx
import { useCallback, useEffect, useRef, useState } from 'react'
import type { BrowserFile, ContentFolderCard } from '../api/types'

interface FolderFilmstripRowProps {
  folder: ContentFolderCard
  loadImages: (entityId: string, retry?: boolean) => Promise<BrowserFile[]>
  onSelect: (entityId: string) => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestThumbnail?: (file: BrowserFile) => Promise<string>
}

type RowState =
  | { status: 'idle' | 'loading' }
  | { status: 'ready'; images: BrowserFile[] }
  | { status: 'failed' }

export default function FolderFilmstripRow({
  folder,
  loadImages,
  onSelect,
  onPreview,
  requestThumbnail,
}: FolderFilmstripRowProps) {
  const row = useRef<HTMLElement>(null)
  const requestSequence = useRef(0)
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === 'undefined')
  const [state, setState] = useState<RowState>({ status: 'idle' })

  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined' || row.current === null) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return
        setVisible(true)
        observer.disconnect()
      },
      { rootMargin: '240px 0px' },
    )
    observer.observe(row.current)
    return () => observer.disconnect()
  }, [])

  const requestImages = useCallback(
    (retry: boolean) => {
      requestSequence.current += 1
      const requestId = requestSequence.current
      setState({ status: 'loading' })
      void loadImages(folder.entityId, retry).then(
        (images) => {
          if (requestId === requestSequence.current) {
            setState({ status: 'ready', images })
          }
        },
        () => {
          if (requestId === requestSequence.current) setState({ status: 'failed' })
        },
      )
    },
    [folder.entityId, loadImages],
  )

  useEffect(() => {
    if (!visible || state.status !== 'idle') return
    requestImages(false)
  }, [requestImages, state.status, visible])

  useEffect(
    () => () => {
      requestSequence.current += 1
    },
    [],
  )

  const reviewed = folder.reviewProgress.total - folder.reviewProgress.unmarked

  return (
    <article ref={row} className="folder-filmstrip-row">
      <button
        type="button"
        className="folder-filmstrip-identity"
        aria-label={`打开 ${folder.name}`}
        onClick={() => onSelect(folder.entityId)}
      >
        <strong>{folder.name}</strong>
        <span>{folder.relativePath}</span>
        <span className="folder-filmstrip-counts">
          {folder.imageCount} 张图片 · {folder.textCount} 个文本
        </span>
        <span className="folder-filmstrip-marker">文件夹：{markerLabel(folder.marker)}</span>
        <span>已审阅 {reviewed} / {folder.reviewProgress.total}</span>
      </button>
      <div
        className="folder-filmstrip"
        role="region"
        aria-label={`${folder.name} 图片`}
        data-state={state.status}
      >
        {state.status === 'idle' && (
          <span className="folder-filmstrip-deferred" aria-label="等待加载图片" />
        )}
        {state.status === 'loading' &&
          Array.from({ length: 4 }, (_, index) => (
            <span
              className="folder-filmstrip-skeleton"
              aria-label="图片加载中"
              key={index}
            />
          ))}
        {state.status === 'failed' && (
          <div className="folder-filmstrip-error" role="alert">
            <span>无法加载图片</span>
            <button
              type="button"
              aria-label={`重试 ${folder.name}`}
              onClick={() => requestImages(true)}
            >
              重试
            </button>
          </div>
        )}
        {state.status === 'ready' && state.images.length === 0 && <p>无图片</p>}
        {state.status === 'ready' &&
          state.images.map((file) => (
            <button
              type="button"
              className="folder-filmstrip-thumbnail"
              aria-label={`预览 ${file.name}`}
              title={file.name}
              key={file.entityId}
              onClick={() => onPreview(file, state.images)}
            >
              <FolderThumbnail file={file} requestThumbnail={requestThumbnail} />
            </button>
          ))}
      </div>
    </article>
  )
}

function markerLabel(marker: ContentFolderCard['marker']): string {
  const review =
    marker.reviewState === 'keep'
      ? '保留'
      : marker.reviewState === 'pending'
        ? '待定'
        : marker.reviewState === 'reject'
          ? '淘汰'
          : '未标记'
  return marker.favorite ? `${review} · 收藏` : review
}

function FolderThumbnail({
  file,
  requestThumbnail,
}: {
  file: BrowserFile
  requestThumbnail?: (file: BrowserFile) => Promise<string>
}) {
  const element = useRef<HTMLSpanElement>(null)
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === 'undefined')
  const [state, setState] = useState<{ status: 'loading' | 'ready' | 'failed'; url?: string }>(
    { status: 'loading' },
  )

  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined' || element.current === null) return
    const observer = new IntersectionObserver((entries) => {
      if (!entries.some((entry) => entry.isIntersecting)) return
      setVisible(true)
      observer.disconnect()
    })
    observer.observe(element.current)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!visible || requestThumbnail === undefined) return
    let current = true
    void requestThumbnail(file).then(
      (url) => {
        if (current) setState({ status: 'ready', url })
      },
      () => {
        if (current) setState({ status: 'failed' })
      },
    )
    return () => {
      current = false
    }
  }, [file, requestThumbnail, visible])

  if (state.status === 'ready') return <img src={state.url} alt="" />
  return (
    <span
      ref={element}
      aria-label={state.status === 'failed' ? '缩略图不可用' : '缩略图加载中'}
    />
  )
}
```

- [ ] **Step 4: Run the focused row tests**

Run:

```bash
pnpm --dir ui exec vitest run src/components/FolderFilmstripRow.test.tsx
```

Expected: 3 tests PASS.

- [ ] **Step 5: Commit the row component**

```bash
git add ui/src/components/FolderFilmstripRow.tsx ui/src/components/FolderFilmstripRow.test.tsx
git commit -m "feat: add lazy folder filmstrip row"
```

### Task 2: Replace the folder card grid with cached rows

**Files:**
- Modify: `ui/src/components/FolderOverview.tsx`
- Modify: `ui/src/components/FolderOverview.test.tsx`

**Interfaces:**
- Consumes: `FolderFilmstripRow` and `requestFolderImages(entityId: string): Promise<BrowserFile[]>`.
- Produces: `FolderOverview` props `requestFolderImages` and `onPreview`, plus an entity-id promise cache shared by all rows in the current overview.

- [ ] **Step 1: Replace the existing overview tests with row-list and cache tests**

Keep the existing `card` fixture in `ui/src/components/FolderOverview.test.tsx`, add a `secondCard` fixture, and replace the three tests with:

```tsx
describe('FolderOverview', () => {
  it('renders one row per folder in source order with compact metadata', async () => {
    const secondCard = {
      ...card,
      entityId: 'folder-2',
      relativePath: 'catalog/shoes/B02',
      name: 'B02',
    }
    render(
      <FolderOverview
        folders={[card, secondCard]}
        currentPath="catalog/shoes"
        requestFolderImages={vi.fn().mockResolvedValue([])}
        onPreview={vi.fn()}
        onSelect={vi.fn()}
        onShowAll={vi.fn()}
      />,
    )

    const rows = screen.getAllByRole('article')
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveTextContent('id-001')
    expect(rows[1]).toHaveTextContent('B02')
    expect(screen.queryByText(/保留 0 · 待定/)).not.toBeInTheDocument()
  })

  it('keeps aggregate and folder navigation actions unchanged', () => {
    const showAll = vi.fn()
    const select = vi.fn()
    render(
      <FolderOverview
        folders={[card]}
        currentPath="catalog/shoes"
        requestFolderImages={vi.fn().mockResolvedValue([])}
        onPreview={vi.fn()}
        onSelect={select}
        onShowAll={showAll}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '显示全部后代文件' }))
    expect(showAll).toHaveBeenCalledOnce()
    expect(screen.getByText('catalog/shoes')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '打开 id-001' }))
    expect(select).toHaveBeenCalledWith('folder-1')
  })

  it('reuses a completed folder request while the overview remains mounted', async () => {
    const requestFolderImages = vi.fn().mockResolvedValue(card.representativeImages)
    const props = {
      currentPath: 'catalog/shoes',
      requestFolderImages,
      onPreview: vi.fn(),
      onSelect: vi.fn(),
      onShowAll: vi.fn(),
    }
    const rendered = render(<FolderOverview {...props} folders={[card]} />)

    await screen.findByRole('button', { name: '预览 1.jpg' })
    rendered.rerender(<FolderOverview {...props} folders={[]} />)
    rendered.rerender(<FolderOverview {...props} folders={[card]} />)
    await screen.findByRole('button', { name: '预览 1.jpg' })

    expect(requestFolderImages).toHaveBeenCalledOnce()
  })
})
```

Update the imports at the top of the test to:

```tsx
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ContentFolderCard } from '../api/types'
import FolderOverview from './FolderOverview'
```

- [ ] **Step 2: Run the overview test to verify the new contract fails**

Run:

```bash
pnpm --dir ui exec vitest run src/components/FolderOverview.test.tsx
```

Expected: FAIL because `requestFolderImages`, `onPreview`, row articles, and complete image loading are not implemented.

- [ ] **Step 3: Replace the card implementation with the cached row list**

Replace `ui/src/components/FolderOverview.tsx` with:

```tsx
import { useCallback, useRef } from 'react'
import type { BrowserFile, ContentFolderCard } from '../api/types'
import FolderFilmstripRow from './FolderFilmstripRow'

interface FolderOverviewProps {
  folders: ContentFolderCard[]
  currentPath: string
  onSelect: (entityId: string) => void
  onShowAll: () => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestFolderImages: (entityId: string) => Promise<BrowserFile[]>
  requestThumbnail?: (file: BrowserFile) => Promise<string>
}

export default function FolderOverview({
  folders,
  currentPath,
  onSelect,
  onShowAll,
  onPreview,
  requestFolderImages,
  requestThumbnail,
}: FolderOverviewProps) {
  const requests = useRef(new Map<string, Promise<BrowserFile[]>>())
  const loadImages = useCallback(
    (entityId: string, retry = false) => {
      if (retry) requests.current.delete(entityId)
      const cached = requests.current.get(entityId)
      if (cached !== undefined) return cached
      const request = requestFolderImages(entityId)
      requests.current.set(entityId, request)
      return request
    },
    [requestFolderImages],
  )

  return (
    <section className="folder-overview" aria-label="内容文件夹概览">
      <header className="workspace-heading">
        <p>{currentPath || '项目根目录'}</p>
        <button type="button" onClick={onShowAll}>
          显示全部后代文件
        </button>
      </header>
      <div className="folder-filmstrip-list">
        {folders.map((folder) => (
          <FolderFilmstripRow
            folder={folder}
            key={folder.entityId}
            loadImages={loadImages}
            requestThumbnail={requestThumbnail}
            onSelect={onSelect}
            onPreview={onPreview}
          />
        ))}
      </div>
    </section>
  )
}
```

- [ ] **Step 4: Run both folder component test files**

Run:

```bash
pnpm --dir ui exec vitest run src/components/FolderFilmstripRow.test.tsx src/components/FolderOverview.test.tsx
```

Expected: 6 tests PASS.

- [ ] **Step 5: Commit the overview refactor**

```bash
git add ui/src/components/FolderOverview.tsx ui/src/components/FolderOverview.test.tsx
git commit -m "feat: render folders as cached filmstrip rows"
```

### Task 3: Connect complete row data and explicit preview context

**Files:**
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`

**Interfaces:**
- Consumes: existing `bridge.queryFolder(entityId, false): Promise<FolderWorkspace>`.
- Produces: `requestFolderImages(entityId): Promise<BrowserFile[]>`, `PreviewSession`, `openFilmstripPreview(file, files)`, and preview navigation that preserves its originating context.

- [ ] **Step 1: Add failing category preview and external-refresh tests**

Add this fixture near `contentWorkspace()` in `ui/src/App.test.tsx`:

```tsx
function categoryWorkspace() {
  return {
    workspace: 'category' as const,
    folders: [
      {
        entityId: 'folder-b01',
        relativePath: '角色/B01',
        name: 'B01',
        marker: { reviewState: null, favorite: false },
        imageCount: 2,
        textCount: 0,
        reviewProgress: {
          total: 2,
          keep: 0,
          pending: 0,
          reject: 0,
          unmarked: 2,
          favorite: 0,
        },
        representativeImages: [],
      },
    ],
  }
}
```

Add this test inside the main `describe` block:

```tsx
it('previews a filmstrip image in row order while keeping the category overview', async () => {
  const viewer = bridge()
  vi.mocked(viewer.queryFolder)
    .mockResolvedValueOnce(categoryWorkspace())
    .mockResolvedValueOnce(compareContentWorkspace())
  render(<App bridge={viewer} />)
  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

  const front = await screen.findByRole('button', { name: '预览 front.jpg' })
  front.focus()
  fireEvent.click(front)

  const preview = screen.getByRole('dialog', { name: '图片预览' })
  expect(preview).toHaveTextContent('front.jpg')
  expect(preview).toHaveTextContent('1 / 2')
  expect(screen.getByRole('region', { name: 'B01 图片' })).toBeInTheDocument()

  fireEvent.keyDown(preview, { key: 'ArrowRight' })
  expect(preview).toHaveTextContent('back.jpg')
  expect(preview).toHaveTextContent('2 / 2')

  fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))
  expect(front).toHaveFocus()
  expect(viewer.queryFolder).toHaveBeenNthCalledWith(2, 'folder-b01', false)
})
```

Add a second test to prove that a same-generation external refresh does not
reuse stale row data:

```tsx
it('reloads visible filmstrip rows after an external category refresh', async () => {
  const viewer = bridge()
  let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
  vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
    receiveProjectChanged = handler
    return () => undefined
  })
  const refreshedImages = {
    ...compareContentWorkspace(),
    images: [
      {
        ...compareContentWorkspace().images[0]!,
        entityId: 'image-updated',
        name: 'updated.jpg',
        relativePath: 'id/updated.jpg',
      },
    ],
  }
  vi.mocked(viewer.queryFolder)
    .mockResolvedValueOnce(categoryWorkspace())
    .mockResolvedValueOnce(compareContentWorkspace())
    .mockResolvedValueOnce(categoryWorkspace())
    .mockResolvedValueOnce(refreshedImages)
  render(<App bridge={viewer} />)
  await waitFor(() => expect(receiveProjectChanged).toBeDefined())
  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
  await screen.findByRole('button', { name: '预览 front.jpg' })

  act(() => {
    receiveProjectChanged?.({
      sessionId: 'session-1',
      generation: 1,
      reason: 'external_change',
      added: 0,
      removed: 0,
      modified: 1,
      moved: 0,
      markerPathsMoved: 0,
      failed: 0,
    })
  })

  expect(await screen.findByRole('button', { name: '预览 updated.jpg' })).toBeVisible()
  expect(viewer.queryFolder).toHaveBeenNthCalledWith(4, 'folder-b01', false)
})
```

- [ ] **Step 2: Run the application tests to verify they fail**

Run:

```bash
pnpm --dir ui exec vitest run src/App.test.tsx -t "filmstrip"
```

Expected: both tests FAIL because the category overview does not request full
row images, the preview is gated on `workspace === 'content'`, and a refreshed
projection does not yet remount the row cache.

- [ ] **Step 3: Add the preview-session model and child-folder loader**

Add below `RadialMenuSession` in `ui/src/App.tsx`:

```tsx
interface PreviewSession {
  file: BrowserFile
  files: BrowserFile[] | null
}
```

Change the preview state to:

```tsx
const [activePreview, setActivePreview] = useState<PreviewSession | null>(null)
```

Add below `requestThumbnail`:

```tsx
const requestFolderImages = useCallback(
  async (entityId: string) => {
    const workspace = await bridge.queryFolder(entityId, false)
    return workspace.workspace === 'content' ? workspace.images : []
  },
  [bridge],
)
```

Add this projection identity bookkeeping beside the other top-level `useRef`
state, before the `if (state.project === null)` return:

```tsx
const folderOverviewProjection = useRef(state.workspace)
const folderOverviewSequence = useRef(0)
if (folderOverviewProjection.current !== state.workspace) {
  folderOverviewProjection.current = state.workspace
  folderOverviewSequence.current += 1
}
const folderOverviewIdentity = [
  state.project?.sessionId ?? 'no-session',
  state.project?.generation ?? 0,
  state.selectedFolderId ?? 'root',
  folderOverviewSequence.current,
].join(':')
```

Replace `openPreview` and `closePreview` with:

```tsx
const openPreview = useCallback(
  (file: BrowserFile) => {
    setActivePreview({ file, files: null })
    setPreviewEntityId(file.entityId)
  },
  [setPreviewEntityId],
)

const openFilmstripPreview = useCallback(
  (file: BrowserFile, files: BrowserFile[]) => {
    setActivePreview({ file, files })
    setPreviewEntityId(file.entityId)
  },
  [setPreviewEntityId],
)

const navigatePreview = useCallback(
  (file: BrowserFile) => {
    setActivePreview((current) => (current === null ? null : { ...current, file }))
    setPreviewEntityId(file.entityId)
  },
  [setPreviewEntityId],
)

const closePreview = useCallback(() => {
  setActivePreview(null)
  setPreviewEntityId(null)
}, [setPreviewEntityId])
```

Add these derived values immediately before the component return:

```tsx
const activePreviewFiles =
  activePreview?.files ??
  (state.workspace?.workspace === 'content' ? state.workspace.images : [])
const activePreviewFile =
  activePreview === null
    ? null
    : activePreviewFiles.find(
        (candidate) => candidate.entityId === activePreview.file.entityId,
      ) ?? activePreview.file
```

- [ ] **Step 4: Wire the overview identity and render previews from the explicit context**

Replace the category `FolderOverview` call with:

```tsx
<FolderOverview
  key={folderOverviewIdentity}
  folders={state.workspace.folders}
  currentPath={state.selectedFolderPath || state.project.displayName}
  requestFolderImages={requestFolderImages}
  requestThumbnail={requestThumbnail}
  onPreview={openFilmstripPreview}
  onSelect={selectFolderTarget}
  onShowAll={() => void showAllDescendants()}
/>
```

Replace both preview render branches with:

```tsx
{activePreviewFile && matchesImage(activePreviewFile) && activePreviewFiles.length > 0 && (
  <ImagePreview
    file={activePreviewFile}
    files={activePreviewFiles}
    requestImage={requestPreviewImage}
    onNavigate={navigatePreview}
    onClose={closePreview}
    onDimensions={rememberDimensions}
  />
)}
{activePreviewFile && !matchesImage(activePreviewFile) && (
  <TextPreview
    file={activePreviewFile}
    requestPreview={requestTextPreview}
    openExternalLink={bridge.openExternalLink}
    onClose={closePreview}
    onTaskChange={setTextTask}
  />
)}
```

Replace the context-repair preview effect with:

```tsx
useEffect(() => {
  if (
    activePreview &&
    state.contextRepair?.removedEntityIds.includes(activePreview.file.entityId)
  ) {
    setActivePreview(null)
  }
}, [activePreview, state.contextRepair])
```

Confirm that no old direct property access remains:

```bash
rg -n "activePreview\\.(entityId|kind|name|relativePath)" ui/src/App.tsx
```

Expected after the edit: no matches.

- [ ] **Step 5: Run the new test and existing preview tests**

Run:

```bash
pnpm --dir ui exec vitest run src/App.test.tsx -t "filmstrip|restores the selected virtual cell|falls back to the surviving image preview"
pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx
```

Expected: selected App preview tests PASS and 2 `ImagePreview` tests PASS.

- [ ] **Step 6: Commit the application integration**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx
git commit -m "feat: preview images from folder filmstrip context"
```

### Task 4: Apply the compact row layout and verify the repository

**Files:**
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: class names emitted by `FolderOverview` and `FolderFilmstripRow`.
- Produces: fixed 184-pixel identity column, flexible independently scrolling filmstrip, 132-pixel thumbnail cells, stable scrollbar gutter, and dark-mode treatment.

- [ ] **Step 1: Add a failing CSS contract test**

Add to `ui/src/styles/app.test.ts`:

```tsx
it('keeps folder identity fixed beside an independently scrolling filmstrip', () => {
  const rules = parseRules(appCss)
  const row = rules.find((rule) => rule.selector === '.folder-filmstrip-row')
  const filmstrip = rules.find((rule) => rule.selector === '.folder-filmstrip')
  const thumbnail = rules.find(
    (rule) => rule.selector === '.folder-filmstrip-thumbnail',
  )

  expect(row?.declarations).toMatchObject({
    display: 'grid',
    'grid-template-columns': '184px minmax(0, 1fr)',
  })
  expect(filmstrip?.declarations).toMatchObject({
    'min-width': '0',
    'overflow-x': 'auto',
    'overflow-y': 'hidden',
    'scrollbar-gutter': 'stable',
  })
  expect(thumbnail?.declarations).toMatchObject({
    flex: '0 0 132px',
    height: '132px',
  })
})
```

- [ ] **Step 2: Run the CSS test to verify it fails**

Run:

```bash
pnpm --dir ui exec vitest run src/styles/app.test.ts -t "keeps folder identity fixed"
```

Expected: FAIL because the filmstrip selectors do not exist.

- [ ] **Step 3: Replace the old folder-card CSS with filmstrip-row CSS**

Delete the rules from `.folder-card-grid` through `.folder-card-marker`, then insert:

```css
.folder-filmstrip-list {
  border-top: 1px solid #d8dce2;
  display: flex;
  flex-direction: column;
}

.folder-filmstrip-row {
  border-bottom: 1px solid #d8dce2;
  display: grid;
  grid-template-columns: 184px minmax(0, 1fr);
  min-height: 156px;
}

.folder-filmstrip-identity {
  align-content: start;
  background: #f8f9fb;
  border: 0;
  border-right: 1px solid #d8dce2;
  border-radius: 0;
  color: inherit;
  display: grid;
  gap: 5px;
  min-width: 0;
  padding: 14px 12px;
  text-align: left;
}

.folder-filmstrip-identity:hover {
  background: #eef4fc;
}

.folder-filmstrip-identity:focus-visible,
.folder-filmstrip-thumbnail:focus-visible {
  box-shadow: inset 0 0 0 2px #2477d4;
  outline: none;
}

.folder-filmstrip-identity strong {
  font-size: 17px;
}

.folder-filmstrip-identity > span {
  color: #59616c;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.folder-filmstrip-identity .folder-filmstrip-marker {
  color: #714d00;
  font-weight: 600;
}

.folder-filmstrip {
  align-items: center;
  display: flex;
  gap: 8px;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 12px;
  scrollbar-gutter: stable;
}

.folder-filmstrip > p {
  color: #737982;
  margin: 0;
}

.folder-filmstrip-thumbnail,
.folder-filmstrip-skeleton {
  flex: 0 0 132px;
  height: 132px;
}

.folder-filmstrip-thumbnail {
  background: #e5e8ed;
  border: 1px solid transparent;
  border-radius: 6px;
  overflow: hidden;
  padding: 0;
}

.folder-filmstrip-thumbnail:hover {
  border-color: #8cb8ea;
}

.folder-filmstrip-thumbnail > img,
.folder-filmstrip-thumbnail > span {
  background: #e5e8ed;
  display: block;
  height: 100%;
  object-fit: contain;
  width: 100%;
}

.folder-filmstrip-deferred {
  background: #eef0f3;
  border-radius: 6px;
  height: 132px;
  width: min(100%, 560px);
}

.folder-filmstrip-skeleton {
  background: #e5e8ed;
  border-radius: 6px;
}

.folder-filmstrip-error {
  align-items: center;
  color: #a12720;
  display: flex;
  gap: 10px;
}

.folder-filmstrip-error button {
  min-height: 30px;
}
```

Add these selectors to the final dark-mode block:

```css
.folder-filmstrip-identity {
  background: #24282f;
  border-color: #4d5663;
}

.folder-filmstrip-identity:hover {
  background: #2d3845;
}

.folder-filmstrip-identity > span {
  color: #c7ced8;
}

.folder-filmstrip-identity .folder-filmstrip-marker {
  color: #ffd27a;
}

.folder-filmstrip-list,
.folder-filmstrip-row {
  border-color: #4d5663;
}

.folder-filmstrip-thumbnail,
.folder-filmstrip-thumbnail > span,
.folder-filmstrip-skeleton,
.folder-filmstrip-deferred {
  background: #343a43;
}
```

- [ ] **Step 4: Run component, style, and build checks**

Run:

```bash
pnpm --dir ui exec vitest run src/components/FolderFilmstripRow.test.tsx src/components/FolderOverview.test.tsx src/components/ImagePreview.test.tsx src/styles/app.test.ts
pnpm --dir ui build
```

Expected: all selected tests PASS and the TypeScript/Vite production build completes successfully.

- [ ] **Step 5: Commit the visual layout**

```bash
git add ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "style: lay out folders as horizontal filmstrips"
```

- [ ] **Step 6: Run the complete repository verification**

Run:

```bash
pnpm verify
```

Expected: repository policy, all UI tests, UI build, Rust formatting, Clippy, and Rust tests all PASS.

- [ ] **Step 7: Inspect the final change set**

Run:

```bash
git status --short
git log --oneline -5
git diff HEAD~4..HEAD --check
```

Expected: only the user's pre-existing untracked fixture metadata remains; the last four feature commits are present; diff whitespace check prints no errors.
