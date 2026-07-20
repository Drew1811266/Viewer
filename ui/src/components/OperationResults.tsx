import type { OperationProgressEvent, OperationResultPage } from '../api/types'

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
    <aside className="operation-results" aria-label="文件操作结果" data-batch-id={batchId}>
      <header>
        <div>
          <h2>文件操作结果</h2>
          <p>
            <span>{progress.completed} 项完成</span>
            <span>{progress.failed} 项失败</span>
            <span>{progress.skipped} 项跳过</span>
            {progress.cancelled > 0 && <span>{progress.cancelled} 项取消</span>}
          </p>
        </div>
        <button type="button" aria-label="关闭操作结果" onClick={onClose}>
          关闭
        </button>
      </header>
      <ul>
        {page.items.map((item) => (
          <li key={item.entityId} data-status={item.status}>
            <span>{item.relativePath}</span>
            <span>{statusLabel(item.status)}</span>
            <code>{item.code}</code>
          </li>
        ))}
      </ul>
      <footer>
        <span>
          {page.total === 0 ? 0 : page.offset + 1}–{Math.min(page.total, displayedEnd)} / {page.total}
        </span>
        <button
          type="button"
          aria-label="上一页结果"
          disabled={page.offset === 0}
          onClick={() => onPageChange(Math.max(0, page.offset - PAGE_SIZE))}
        >
          上一页
        </button>
        <button
          type="button"
          aria-label="下一页结果"
          disabled={nextOffset >= page.total}
          onClick={() => onPageChange(nextOffset)}
        >
          下一页
        </button>
      </footer>
    </aside>
  )
}

function statusLabel(status: OperationResultPage['items'][number]['status']): string {
  if (status === 'completed') return '完成'
  if (status === 'failed') return '失败'
  if (status === 'skipped') return '跳过'
  return '取消'
}
