import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import App from '../../App'
import type { BrowserFile, ProjectSnapshot, SearchHit, SearchPage } from '../../api/types'
import type { ViewerBridge } from '../../api/viewer'
import EmptyProject from '../../components/EmptyProject'
import OrganizationDragPreview from '../../components/OrganizationDragPreview'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import { createAcceptanceBridge } from '../acceptanceBridge'
import {
  ACCEPTANCE_FILES,
  ACCEPTANCE_FOLDER_TREE,
  ACCEPTANCE_PROJECT_SNAPSHOT,
  acceptanceWorkspace,
} from '../acceptanceFixtures'
import type { AcceptanceRequest } from '../acceptanceRequest'

export const WORKSPACE_SCENES: AcceptanceSceneRegistry = {
  'LAU-01': (props) => <LaunchScene {...props} state="idle" />,
  'LAU-02': (props) => <LaunchScene {...props} state="drag" />,
  'LAU-03': (props) => <LaunchScene {...props} state="invalid" />,
  'LAU-04': (props) => <LaunchScene {...props} state="opening" />,
  'LAU-05': (props) => <WorkspaceScene {...props} />,
  'LAU-06': (props) => <WorkspaceScene {...props} />,
  'LAU-07': (props) => <WorkspaceScene {...props} />,
  'LAU-08': (props) => <LaunchScene {...props} state="error" />,
  'LAU-09': (props) => <WorkspaceScene {...props} />,
  'SID-01': (props) => <WorkspaceScene {...props} />,
  'SID-02': (props) => <WorkspaceScene {...props} />,
  'SID-03': (props) => <WorkspaceScene {...props} />,
  'SID-04': (props) => <WorkspaceScene {...props} />,
  'STR-01': (props) => <WorkspaceScene {...props} />,
  'STR-02': (props) => <WorkspaceScene {...props} />,
  'STR-03': (props) => <WorkspaceScene {...props} />,
  'STR-04': (props) => <WorkspaceScene {...props} />,
  'STR-05': (props) => <WorkspaceScene {...props} />,
  'THU-01': (props) => <WorkspaceScene {...props} />,
  'THU-02': (props) => <WorkspaceScene {...props} />,
  'THU-03': (props) => <WorkspaceScene {...props} />,
  'THU-04': (props) => <WorkspaceScene {...props} />,
  'THU-05': (props) => <WorkspaceScene {...props} />,
  'THU-06': (props) => <WorkspaceScene {...props} />,
  'THU-07': (props) => <WorkspaceScene {...props} />,
  'OTH-01': (props) => <WorkspaceScene {...props} />,
  'OTH-02': (props) => <WorkspaceScene {...props} />,
  'OTH-03': (props) => <WorkspaceScene {...props} />,
  'SEA-01': (props) => <WorkspaceScene {...props} />,
  'SEA-02': (props) => <WorkspaceScene {...props} />,
  'SEA-03': (props) => <WorkspaceScene {...props} />,
  'SEA-04': (props) => <WorkspaceScene {...props} />,
  'SEA-05': (props) => <WorkspaceScene {...props} />,
  'FIL-01': (props) => <WorkspaceScene {...props} />,
  'FIL-02': (props) => <WorkspaceScene {...props} />,
  'FIL-03': (props) => <WorkspaceScene {...props} />,
  'FIL-04': (props) => <WorkspaceScene {...props} />,
  'MEN-01': (props) => <WorkspaceScene {...props} />,
  'MEN-02': (props) => <WorkspaceScene {...props} />,
  'MEN-03': (props) => <WorkspaceScene {...props} />,
}

type LaunchState = 'idle' | 'drag' | 'invalid' | 'opening' | 'error'

function LaunchScene({ request, state }: { request: AcceptanceRequest; state: LaunchState }) {
  const bridge = useMemo(() => launchBridge(state), [state])
  if (state === 'error') {
    return (
      <SceneReady ready>
        <EmptyProject
          bridge={bridge}
          fatalError
          errorMessage="没有读取这个文件夹的权限，请重新选择已授权的项目目录。"
        />
      </SceneReady>
    )
  }
  const readyWhen = () => {
    if (state === 'idle') return document.querySelector('.empty-project') !== null
    if (state === 'drag') return document.querySelector('[data-drop-state="valid"]') !== null
    if (state === 'invalid') return document.querySelector('[data-drop-state="invalid"]') !== null
    const choose = namedElement('选择项目文件夹')
    if (choose !== null) {
      clickOnce(choose, 'opening')
      return false
    }
    return document.querySelector('[aria-label="正在打开项目"]') !== null
  }
  return (
    <AutomatedScene request={request} run={readyWhen}>
      <EmptyProject bridge={bridge} />
    </AutomatedScene>
  )
}

function WorkspaceScene({ request }: { request: AcceptanceRequest }) {
  const bridge = useMemo(() => workspaceBridge(request.id), [request.id])
  const recipe = useMemo(() => workspaceRecipe(request.id), [request.id])
  return (
    <AutomatedScene request={request} run={recipe}>
      <App bridge={bridge} />
      {(request.id === 'SID-04' || request.id === 'OTH-03') && (
        <OrganizationDragPreview
          clientX={Math.round(request.width * 0.36)}
          clientY={Math.round(request.height * 0.52)}
          itemCount={3}
          mode="move"
        />
      )}
    </AutomatedScene>
  )
}

function SceneReady({ ready, children }: { ready: boolean; children: ReactNode }) {
  return (
    <div
      className="acceptance-scene-root"
      data-acceptance-scene-ready={ready ? 'true' : 'false'}
      style={{ width: '100%', height: '100%' }}
    >
      {children}
    </div>
  )
}

function AutomatedScene({
  request,
  run,
  children,
}: {
  request: AcceptanceRequest
  run(): boolean
  children: ReactNode
}) {
  const [ready, setReady] = useState(false)
  const settled = useRef(false)
  useEffect(() => {
    let observer: MutationObserver | undefined
    let firstFrame: number | undefined
    let secondFrame: number | undefined
    const attempt = () => {
      if (settled.current || !run()) return
      settled.current = true
      observer?.disconnect()
      firstFrame = requestAnimationFrame(() => {
        secondFrame = requestAnimationFrame(() => setReady(true))
      })
    }
    observer = new MutationObserver(attempt)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    attempt()
    return () => {
      observer?.disconnect()
      if (firstFrame !== undefined) cancelAnimationFrame(firstFrame)
      if (secondFrame !== undefined) cancelAnimationFrame(secondFrame)
    }
  }, [request.id, run])
  return <SceneReady ready={ready}>{children}</SceneReady>
}

function launchBridge(state: LaunchState): ViewerBridge {
  if (state === 'drag') {
    return createAcceptanceBridge({
      async projectSnapshot() {
        return null
      },
      async listenProjectDropEvents(handler) {
        queueMicrotask(() => handler({ type: 'enter', paths: ['/ViewerAcceptance'] }))
        return () => undefined
      },
    })
  }
  if (state === 'invalid') {
    return createAcceptanceBridge({
      async projectSnapshot() {
        return null
      },
      async openProject() {
        const error = new Error('请选择项目文件夹，不能导入单个文件。') as Error & {
          code: string
        }
        error.code = 'invalid_project_root'
        throw error
      },
      async listenProjectDropEvents(handler) {
        queueMicrotask(() =>
          handler({ type: 'drop', paths: ['/ViewerAcceptance/衣服/A01/商品-01.jpg'] }),
        )
        return () => undefined
      },
    })
  }
  if (state === 'opening') {
    return createAcceptanceBridge({
      async projectSnapshot() {
        return null
      },
      async openProject() {
        return new Promise<never>(() => undefined)
      },
    })
  }
  return createAcceptanceBridge({
    async projectSnapshot() {
      return null
    },
  })
}

function workspaceBridge(id: string): ViewerBridge {
  const density =
    id === 'THU-01' || id.startsWith('OTH-') ? 'compact' : id === 'THU-03' ? 'large' : 'standard'
  const snapshot: ProjectSnapshot =
    id === 'MEN-03'
      ? { ...ACCEPTANCE_PROJECT_SNAPSHOT, access: 'read_only' }
      : id === 'LAU-09'
        ? {
            ...ACCEPTANCE_PROJECT_SNAPSHOT,
            recoveryReport: { recovered: 3, needsUserReview: 1 },
          }
        : ACCEPTANCE_PROJECT_SNAPSHOT
  const overrides: Partial<ViewerBridge> = {
    async projectSnapshot() {
      return null
    },
    async openProject() {
      return snapshot
    },
    async folderTree() {
      return ACCEPTANCE_FOLDER_TREE
    },
    async getViewerSettings() {
      return { schemaVersion: 1, thumbnailDensity: density }
    },
    async queryFolder(entityId, showingAggregate) {
      if (id === 'LAU-05') return new Promise<never>(() => undefined)
      if (id === 'LAU-07') return { workspace: 'empty' }
      return acceptanceWorkspace(entityId, showingAggregate)
    },
  }
  if (id === 'LAU-06') {
    overrides.requestImage = async () => new Promise<never>(() => undefined)
  }
  if (id === 'SEA-03') {
    overrides.searchProject = async (search) => incompleteSearchPage(search.revision)
  }
  if (id === 'SEA-04') {
    overrides.searchProject = async (search) => pagingSearchPage(search.revision, search.offset)
  }
  return createAcceptanceBridge(overrides)
}

function workspaceRecipe(id: string): () => boolean {
  let phase = 0
  return () => {
    const choose = namedElement('选择项目文件夹')
    if (choose !== null) {
      clickOnce(choose, 'open-project')
      return false
    }

    if (id === 'LAU-05') return document.querySelector('[aria-label="项目内容加载中"]') !== null
    if (id === 'LAU-09') return textElement('项目状态已恢复') !== null
    if (document.querySelector('.viewer-shell') === null) return false
    if (id === 'LAU-07') return textElement('这个项目中还没有可显示的文件') !== null

    if (id === 'STR-01') return document.querySelector('[aria-label="内容文件夹概览"]') !== null
    if (id === 'STR-02' || id === 'STR-05') {
      return selectFolderAndWait('衣服', '[aria-label="内容文件夹概览"]')
    }

    if (
      !['MEN-03'].includes(id) &&
      id.startsWith('MEN-') === false &&
      id.startsWith('SEA-') === false &&
      id.startsWith('FIL-') === false
    ) {
      if (!selectFolderAndWait('衣服/A01', '[aria-label="文件内容"]')) return false
    }

    if (id === 'LAU-06') return document.querySelector('[aria-label="缩略图加载中"]') !== null
    if (
      id === 'SID-01' ||
      id === 'STR-03' ||
      id === 'THU-01' ||
      id === 'THU-02' ||
      id === 'THU-03' ||
      id === 'THU-04'
    ) {
      return document.querySelector('[aria-label="文件内容"]') !== null
    }
    if (id === 'SID-02') {
      const separator = document.querySelector<HTMLElement>('[aria-label="调整文件夹栏宽度"]')
      if (separator === null) return false
      if (!separator.dataset.acceptanceActed) {
        separator.dataset.acceptanceActed = 'true'
        separator.dispatchEvent(pointerEvent('pointerdown', { clientX: 220, button: 0 }))
        window.dispatchEvent(pointerEvent('pointermove', { clientX: 320, button: 0 }))
        window.dispatchEvent(pointerEvent('pointerup', { clientX: 320, button: 0 }))
      }
      return separator.getAttribute('aria-valuenow') === '320'
    }
    if (id === 'SID-03') {
      const collapse = namedElement('折叠文件夹栏')
      if (collapse !== null) clickOnce(collapse, 'collapse-sidebar')
      return namedElement('展开文件夹栏') !== null
    }
    if (id === 'SID-04' || id === 'OTH-03') return prepareOrganizationDrag()
    if (id === 'STR-04') {
      if (textElement('全部后代文件') !== null) return true
      const view = namedElement('视图')
      if (document.querySelector('[aria-label="视图选项"]') === null) {
        if (view !== null) clickOnce(view, 'aggregate-view')
        return false
      }
      const allDescendants = textElement('显示全部后代文件', 'button')
      if (allDescendants !== null) clickOnce(allDescendants, 'aggregate-all')
      return false
    }

    if (id === 'THU-05') return selectImages(1)
    if (id === 'THU-06' || id === 'OTH-01') return selectImages(3)
    if (id === 'THU-07') {
      if (!selectImages(1)) return false
      const grid = document.querySelector<HTMLElement>('[role="listbox"][aria-label="图片文件"]')
      if (grid === null) return false
      if (!grid.dataset.acceptanceActed) {
        grid.dataset.acceptanceActed = 'true'
        grid.focus()
        grid.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowRight' }))
      }
      return document.querySelector('[data-active="true"][aria-selected="true"]') !== null
    }
    if (id === 'OTH-02') {
      if (!selectImages(3)) return false
      const other = namedElement(/^其它文件 · \d+$/)
      if (other !== null) clickOnce(other, 'expand-other')
      return document.querySelector('[role="listbox"][aria-label="其它文件"]') !== null
    }

    if (id.startsWith('SEA-') || id === 'MEN-01') {
      const query = id === 'SEA-05' ? 'viewer-no-match-20260802' : 'jpg'
      if (!enterSearch(query)) return false
      if (id === 'SEA-05') return document.querySelector('[aria-label="无搜索结果"]') !== null
      if (document.querySelector('[aria-label="搜索结果区域"]') === null) return false
      if (id === 'SEA-03') return textElement('结果仍在更新') !== null
      if (id === 'SEA-04') {
        if (textElement('第 2 页') !== null) return true
        const next = textElement('下一页', 'button')
        if (next !== null && !(next as HTMLButtonElement).disabled) clickOnce(next, 'next-page')
        return false
      }
      if (id === 'SEA-02') {
        if (phase === 0) {
          const view = namedElement('视图')
          if (view !== null) clickOnce(view, 'flat-view')
          phase = 1
          return false
        }
        const flat = textElement('展平结果', 'button')
        if (flat !== null) {
          clickOnce(flat, 'flat-results')
          phase = 2
          return false
        }
        return phase === 2 && document.querySelector('[role="group"]') === null
      }
      if (id === 'MEN-01') {
        const view = namedElement('视图')
        if (document.querySelector('[aria-label="视图选项"]') === null && view !== null) {
          clickOnce(view, 'search-view-menu')
          return false
        }
        return document.querySelector('[aria-label="视图选项"]') !== null
      }
      return document.querySelector('[role="group"]') !== null
    }

    if (id.startsWith('FIL-')) return prepareFilter(id)
    if (id === 'MEN-02' || id === 'MEN-03') {
      if (id === 'MEN-03' && document.querySelector('[aria-label="只读模式"]') === null)
        return false
      const more = namedElement('更多')
      if (document.querySelector('[aria-label="更多操作"]') === null && more !== null) {
        clickOnce(more, 'more-menu')
        return false
      }
      return document.querySelector('[aria-label="更多操作"]') !== null
    }
    return document.querySelector('.viewer-shell') !== null
  }
}

function selectFolderAndWait(label: string, readySelector: string): boolean {
  const row = document.querySelector<HTMLElement>(`[role="treeitem"][aria-label="${label}"]`)
  if (row === null) return false
  if (row.getAttribute('aria-selected') !== 'true') {
    clickOnce(row, `folder-${label}`)
    return false
  }
  return document.querySelector(readySelector) !== null
}

function selectImages(count: number): boolean {
  const options = [
    ...document.querySelectorAll<HTMLElement>('[role="option"][aria-label^="商品-"]'),
  ]
  if (options.length < count) return false
  const selected = options.filter((option) => option.getAttribute('aria-selected') === 'true')
  if (selected.length === count) return true
  if (selected.length === 0 || selected.length > count) {
    options[0]?.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    return false
  }
  const next = options
    .slice(0, count)
    .find((option) => option.getAttribute('aria-selected') !== 'true')
  next?.dispatchEvent(new MouseEvent('click', { bubbles: true, metaKey: true }))
  return false
}

function prepareOrganizationDrag(): boolean {
  const destination = document.querySelector<HTMLElement>(
    '[data-organization-folder-id="acceptance-folder-destination"]',
  )
  if (destination === null) return false
  if (destination.dataset.organizationDropTarget !== 'true') {
    destination.dataset.dropValid = 'true'
    destination.dataset.dropMode = 'move'
    destination.dataset.organizationDropTarget = 'true'
  }
  return document.querySelector('.organization-drag-preview') !== null
}

function enterSearch(value: string): boolean {
  const input = document.querySelector<HTMLInputElement>('input[aria-label="搜索项目"]')
  if (input === null) return false
  if (input.value !== value) {
    setInputValue(input, value)
    return false
  }
  return true
}

function prepareFilter(id: string): boolean {
  const popover = document.querySelector('[aria-label="筛选条件"]')
  if (popover === null) {
    const trigger = namedElement(/^筛选(?:，\d+ 项已启用)?$/)
    if (trigger !== null) clickOnce(trigger, 'open-filter')
    return false
  }
  if (id === 'FIL-01') return true
  const labels = id === 'FIL-02' ? ['JPEG'] : id === 'FIL-03' ? ['JPEG', 'PNG', '保留', '收藏'] : []
  for (const label of labels) {
    const input = choiceInput(label)
    if (input !== null && !input.checked) {
      input.click()
      return false
    }
  }
  if (id === 'FIL-03') return labels.every((label) => choiceInput(label)?.checked)
  if (id === 'FIL-04') {
    const advanced = textElement('高级条件', 'summary')
    if (document.querySelector('input[aria-label="最小宽度"]') === null) {
      const edit = textElement('编辑高级条件', 'button')
      if (edit !== null) clickOnce(edit, 'edit-advanced-filter')
      else if (advanced !== null) clickOnce(advanced, 'advanced-filter')
      return false
    }
    const width = document.querySelector<HTMLInputElement>('input[aria-label="最小宽度"]')
    const modified = document.querySelector<HTMLInputElement>('input[aria-label="最早修改时间"]')
    if (width === null || modified === null) return false
    if (width.value !== '1200') {
      setInputValue(width, '1200')
      return false
    }
    if (modified.value === '') {
      setInputValue(modified, '2026-01-05T09:30')
      return false
    }
    return true
  }
  return true
}

function choiceInput(label: string): HTMLInputElement | null {
  return (
    [...document.querySelectorAll<HTMLLabelElement>('label.viewer-choice-chip')]
      .find((candidate) => candidate.textContent?.trim() === label)
      ?.querySelector('input') ?? null
  )
}

function namedElement(name: string | RegExp): HTMLElement | null {
  const candidates = document.querySelectorAll<HTMLElement>('button, summary, [role="button"]')
  return (
    [...candidates].find((candidate) => {
      const value = candidate.getAttribute('aria-label') ?? candidate.textContent?.trim() ?? ''
      return typeof name === 'string' ? value === name : name.test(value)
    }) ?? null
  )
}

function textElement(value: string, selector = '*'): HTMLElement | null {
  return (
    [...document.querySelectorAll<HTMLElement>(selector)].find(
      (candidate) =>
        (selector !== '*' || candidate.children.length === 0) &&
        candidate.textContent?.trim().includes(value),
    ) ?? null
  )
}

function clickOnce(element: HTMLElement, key: string) {
  if (element.dataset.acceptanceAction === key) return
  element.dataset.acceptanceAction = key
  element.click()
}

function setInputValue(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
  setter?.call(input, value)
  input.dispatchEvent(new Event('input', { bubbles: true }))
  input.dispatchEvent(new Event('change', { bubbles: true }))
}

function pointerEvent(type: string, init: PointerEventInit): Event {
  const EventConstructor = globalThis.PointerEvent ?? MouseEvent
  return new EventConstructor(type, { bubbles: true, ...init })
}

function incompleteSearchPage(revision: number): SearchPage {
  return {
    revision,
    total: 30,
    hits: ACCEPTANCE_FILES.map(searchHit),
    progress: {
      imagesTotal: 30,
      imagesReady: 18,
      imagesFailed: 0,
      textTotal: 4,
      textReady: 1,
      textSkipped: 0,
      textFailed: 0,
      complete: false,
    },
  }
}

function pagingSearchPage(revision: number, _offset: number): SearchPage {
  return {
    revision,
    total: 230,
    hits: ACCEPTANCE_FILES.map(searchHit),
    progress: {
      imagesTotal: 230,
      imagesReady: 230,
      imagesFailed: 0,
      textTotal: 0,
      textReady: 0,
      textSkipped: 0,
      textFailed: 0,
      complete: true,
    },
  }
}

function searchHit(file: BrowserFile): SearchHit {
  return {
    entityId: file.entityId,
    relativePath: file.relativePath,
    name: file.name,
    kind: file.kind,
    size: file.size,
    modifiedNs: file.modifiedNs,
    marker: file.marker,
    imageMetadata: file.imageMetadata,
    matchedField: 'filename',
    score: 1,
    groupRelativePath: '衣服/A01',
    matchRanges: [],
  }
}
