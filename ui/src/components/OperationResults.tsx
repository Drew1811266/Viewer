import type { OperationProgressEvent, OperationResultPage } from '../api/types'
import { ViewerIconButton } from './ui/ViewerButton'
import ViewerInspector from './ui/ViewerInspector'
import ViewerStatusTag, { type ViewerStatusTagTone } from './ui/ViewerStatusTag'

interface OperationResultsProps {
  batchId: string
  progress: OperationProgressEvent
  page: OperationResultPage
  onPageChange: (offset: number) => void
  onClose: () => void
}

const PAGE_SIZE = 200

export default function OperationResults({
  batchId,
  progress,
  page,
  onPageChange,
  onClose,
}: OperationResultsProps) {
  const displayedEnd = page.offset + page.items.length
  const nextOffset = page.offset + PAGE_SIZE
  return (
    <ViewerInspector
      label="文件操作结果"
      title="文件操作结果"
      status={
        <div className="operation-results-summary">
          <ViewerStatusTag tone="success">{progress.completed} 项完成</ViewerStatusTag>
          <ViewerStatusTag tone={progress.failed > 0 ? 'danger' : 'neutral'}>
            {progress.failed} 项失败
          </ViewerStatusTag>
          <ViewerStatusTag tone={progress.skipped > 0 ? 'warning' : 'neutral'}>
            {progress.skipped} 项跳过
          </ViewerStatusTag>
          {progress.cancelled > 0 && (
            <ViewerStatusTag tone="neutral">{progress.cancelled} 项取消</ViewerStatusTag>
          )}
        </div>
      }
      footer={
        <nav className="operation-results-pagination" aria-label="文件操作结果分页">
          <span>
            {page.total === 0 ? 0 : page.offset + 1}–{Math.min(page.total, displayedEnd)} /{' '}
            {page.total}
          </span>
          <span className="operation-results-pagination__actions">
            <ViewerIconButton
              icon="chevron-left"
              label="上一页结果"
              tone="quiet"
              disabled={page.offset === 0}
              onClick={() => onPageChange(Math.max(0, page.offset - PAGE_SIZE))}
            />
            <ViewerIconButton
              icon="chevron-right"
              label="下一页结果"
              tone="quiet"
              disabled={nextOffset >= page.total}
              onClick={() => onPageChange(nextOffset)}
            />
          </span>
        </nav>
      }
      onClose={onClose}
    >
      <ul className="operation-results-list" data-batch-id={batchId}>
        {page.items.map((item) => (
          <li key={item.entityId} data-status={item.status}>
            <span>{item.relativePath}</span>
            <ViewerStatusTag tone={statusTone(item.status)}>
              {statusLabel(item.status)}
            </ViewerStatusTag>
            <code>{item.code}</code>
          </li>
        ))}
      </ul>
    </ViewerInspector>
  )
}

function statusLabel(status: OperationResultPage['items'][number]['status']): string {
  if (status === 'completed') return '完成'
  if (status === 'failed') return '失败'
  if (status === 'skipped') return '跳过'
  return '取消'
}

function statusTone(status: OperationResultPage['items'][number]['status']): ViewerStatusTagTone {
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'danger'
  if (status === 'skipped') return 'warning'
  return 'neutral'
}
