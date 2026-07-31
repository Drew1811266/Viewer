import type { KeyboardEvent } from 'react'
import { useEffect, useRef, useState } from 'react'
import type { SearchLayout } from '../api/types'
import type { SelectAllRequest, SelectAllScope } from './contentBrowser/adaptiveOtherFilePanelModel'
import useViewportPopoverMaxWidth from './useViewportPopoverMaxWidth'

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

export default function WorkspaceViewMenu({
  context,
  openRequest = 0,
}: {
  context: WorkspaceViewContext
  openRequest?: number
}) {
  const [open, setOpen] = useState(false)
  const summaryRef = useRef<HTMLElement>(null)
  const popoverMaxWidth = useViewportPopoverMaxWidth()

  useEffect(() => {
    if (openRequest > 0) {
      window.dispatchEvent(new CustomEvent('viewer-toolbar-popover', { detail: 'view' }))
      setOpen(true)
    }
  }, [openRequest])

  useEffect(() => {
    const closeForPeer = (event: Event) => {
      if ((event as CustomEvent<string>).detail !== 'view') setOpen(false)
    }
    window.addEventListener('viewer-toolbar-popover', closeForPeer)
    return () => window.removeEventListener('viewer-toolbar-popover', closeForPeer)
  }, [])

  function toggle() {
    setOpen((current) => {
      const next = !current
      if (next) window.dispatchEvent(new CustomEvent('viewer-toolbar-popover', { detail: 'view' }))
      return next
    })
  }

  function handleSummaryKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
    event.preventDefault()
    toggle()
  }

  function closeFromEscape(event: KeyboardEvent<HTMLDetailsElement>) {
    if (event.key !== 'Escape') return
    event.preventDefault()
    setOpen(false)
    summaryRef.current?.focus()
  }

  function run(action: () => void) {
    setOpen(false)
    action()
  }

  return (
    <details className="workspace-view-menu" open={open} onKeyDown={closeFromEscape}>
      <summary
        ref={summaryRef}
        role="button"
        aria-label="视图"
        aria-expanded={open}
        onClick={(event) => {
          event.preventDefault()
          toggle()
        }}
        onKeyDown={handleSummaryKeyDown}
      >
        视图
      </summary>
      <div className="workspace-menu-popover" hidden={!open} style={{ maxWidth: popoverMaxWidth }}>
        {context.kind === 'search' && (
          <>
            <button
              type="button"
              className="workspace-menu-item"
              aria-pressed={context.layout === 'grouped'}
              onClick={() => run(() => context.onLayoutChange('grouped'))}
            >
              按文件夹分组
            </button>
            <button
              type="button"
              className="workspace-menu-item"
              aria-pressed={context.layout === 'flat'}
              onClick={() => run(() => context.onLayoutChange('flat'))}
            >
              展平结果
            </button>
          </>
        )}
        {context.kind === 'category' && (
          <button
            type="button"
            className="workspace-menu-item"
            onClick={() => run(context.onShowAllDescendants)}
          >
            显示全部后代文件
          </button>
        )}
        {context.kind === 'content' && (
          <>
            {context.showingAggregate ? (
              <button
                type="button"
                className="workspace-menu-item"
                onClick={() => run(context.onReturnToFolder)}
              >
                返回当前文件夹
              </button>
            ) : (
              <button
                type="button"
                className="workspace-menu-item"
                onClick={() => run(context.onShowAllDescendants)}
              >
                显示全部后代文件
              </button>
            )}
            {selectAllButtons(context.selectAllRequest, context.onSelectAll, run)}
          </>
        )}
      </div>
    </details>
  )
}

function selectAllButtons(
  request: SelectAllRequest,
  onSelectAll: (scope: SelectAllScope) => void,
  run: (action: () => void) => void,
) {
  if (request.kind === 'none') return null
  if (request.kind === 'direct') {
    return (
      <button
        type="button"
        className="workspace-menu-item"
        onClick={() => run(() => onSelectAll(request.scope))}
      >
        {request.scope === 'images' ? '全选图片' : '全选其它文件'}
      </button>
    )
  }
  return (
    <>
      <button
        type="button"
        className="workspace-menu-item"
        onClick={() => run(() => onSelectAll('images'))}
      >
        全选图片
      </button>
      <button
        type="button"
        className="workspace-menu-item"
        onClick={() => run(() => onSelectAll('other'))}
      >
        全选其它文件
      </button>
      <button
        type="button"
        className="workspace-menu-item"
        onClick={() => run(() => onSelectAll('all'))}
      >
        全选全部文件
      </button>
    </>
  )
}
