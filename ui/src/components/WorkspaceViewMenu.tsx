import type { KeyboardEvent } from 'react'
import { useRef, useState } from 'react'
import type { SearchLayout } from '../api/types'
import type { SelectAllRequest, SelectAllScope } from './contentBrowser/adaptiveOtherFilePanelModel'

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

export default function WorkspaceViewMenu({ context }: { context: WorkspaceViewContext }) {
  const [open, setOpen] = useState(false)
  const summaryRef = useRef<HTMLElement>(null)

  function toggle() {
    setOpen((current) => !current)
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
      <div className="workspace-menu-popover" hidden={!open}>
        {context.kind === 'search' && (
          <>
            <button
              type="button"
              aria-pressed={context.layout === 'grouped'}
              onClick={() => run(() => context.onLayoutChange('grouped'))}
            >
              按文件夹分组
            </button>
            <button
              type="button"
              aria-pressed={context.layout === 'flat'}
              onClick={() => run(() => context.onLayoutChange('flat'))}
            >
              展平结果
            </button>
          </>
        )}
        {context.kind === 'category' && (
          <button type="button" onClick={() => run(context.onShowAllDescendants)}>
            显示全部后代文件
          </button>
        )}
        {context.kind === 'content' && (
          <>
            {context.showingAggregate ? (
              <button type="button" onClick={() => run(context.onReturnToFolder)}>
                返回当前文件夹
              </button>
            ) : (
              <button type="button" onClick={() => run(context.onShowAllDescendants)}>
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
      <button type="button" onClick={() => run(() => onSelectAll(request.scope))}>
        {request.scope === 'images' ? '全选图片' : '全选其它文件'}
      </button>
    )
  }
  return (
    <>
      <button type="button" onClick={() => run(() => onSelectAll('images'))}>
        全选图片
      </button>
      <button type="button" onClick={() => run(() => onSelectAll('other'))}>
        全选其它文件
      </button>
      <button type="button" onClick={() => run(() => onSelectAll('all'))}>
        全选全部文件
      </button>
    </>
  )
}
