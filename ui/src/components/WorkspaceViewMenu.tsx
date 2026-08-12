import type { KeyboardEvent } from 'react'
import { useRef } from 'react'
import type { SearchLayout } from '../api/types'
import type { SelectAllRequest, SelectAllScope } from './contentBrowser/adaptiveOtherFilePanelModel'
import ViewerMenuRow from './ui/ViewerMenuRow'
import ViewerPopover from './ui/ViewerPopover'
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
  open,
  onOpenChange,
}: {
  context: WorkspaceViewContext
  open: boolean
  onOpenChange(open: boolean): void
}) {
  const summaryRef = useRef<HTMLElement>(null)
  const popoverMaxWidth = useViewportPopoverMaxWidth()

  function toggle() {
    onOpenChange(!open)
  }

  function handleSummaryKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'Spacebar') return
    event.preventDefault()
    toggle()
  }

  function run(action: () => void) {
    onOpenChange(false)
    action()
  }

  return (
    <details className="workspace-view-menu" open={open}>
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
      <ViewerPopover
        className="workspace-menu-popover"
        open={open}
        label="视图选项"
        triggerRef={summaryRef}
        onOpenChange={onOpenChange}
        style={{ maxWidth: popoverMaxWidth }}
      >
        {context.kind === 'search' && (
          <>
            <ViewerMenuRow
              className="workspace-menu-item"
              current={context.layout === 'grouped'}
              onSelect={() => run(() => context.onLayoutChange('grouped'))}
            >
              按文件夹分组
            </ViewerMenuRow>
            <ViewerMenuRow
              className="workspace-menu-item"
              current={context.layout === 'flat'}
              onSelect={() => run(() => context.onLayoutChange('flat'))}
            >
              展平结果
            </ViewerMenuRow>
          </>
        )}
        {context.kind === 'category' && (
          <ViewerMenuRow
            className="workspace-menu-item"
            onSelect={() => run(context.onShowAllDescendants)}
          >
            显示全部后代文件
          </ViewerMenuRow>
        )}
        {context.kind === 'content' && (
          <>
            {context.showingAggregate ? (
              <ViewerMenuRow
                className="workspace-menu-item"
                onSelect={() => run(context.onReturnToFolder)}
              >
                返回当前文件夹
              </ViewerMenuRow>
            ) : (
              <ViewerMenuRow
                className="workspace-menu-item"
                onSelect={() => run(context.onShowAllDescendants)}
              >
                显示全部后代文件
              </ViewerMenuRow>
            )}
            {context.selectAllRequest.kind !== 'none' && (
              <hr className="workspace-menu-separator" />
            )}
            {selectAllButtons(context.selectAllRequest, context.onSelectAll, run)}
          </>
        )}
      </ViewerPopover>
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
      <ViewerMenuRow
        className="workspace-menu-item"
        onSelect={() => run(() => onSelectAll(request.scope))}
      >
        {selectAllLabel(request.scope)}
      </ViewerMenuRow>
    )
  }
  return (
    <>
      {request.scopes.map((scope) => (
        <ViewerMenuRow
          key={scope}
          className="workspace-menu-item"
          onSelect={() => run(() => onSelectAll(scope))}
        >
          {selectAllLabel(scope)}
        </ViewerMenuRow>
      ))}
      <ViewerMenuRow className="workspace-menu-item" onSelect={() => run(() => onSelectAll('all'))}>
        全选全部文件
      </ViewerMenuRow>
    </>
  )
}

function selectAllLabel(scope: Exclude<SelectAllScope, 'all'>): string {
  if (scope === 'images') return '全选图片'
  if (scope === 'videos') return '全选视频'
  return '全选其它文件'
}
