import type { ReactNode } from 'react'
import type { BrowserFile, ReviewState, SelectionAgreement, SelectionInfo } from '../api/types'

interface InfoOverlayProps {
  files: BrowserFile[]
  selectionInfo?: SelectionInfo | null
  dimensions: Record<string, { width: number; height: number } | undefined>
  onClose: () => void
}

export default function InfoOverlay({
  files,
  selectionInfo,
  dimensions,
  onClose,
}: InfoOverlayProps) {
  const totalSize = files.reduce((sum, file) => sum + file.size, 0)
  const counts = files.reduce<Record<string, number>>((result, file) => {
    result[file.kind] = (result[file.kind] ?? 0) + 1
    return result
  }, {})
  const hasSelection =
    selectionInfo !== undefined && selectionInfo !== null && selectionInfo.relativePaths.length > 0
  const [onlyFile] = files
  if (files.length === 1 && onlyFile === undefined) {
    throw new Error('Single-file information selection is missing its file')
  }

  return (
    <aside className="info-overlay" aria-label="文件信息">
      <header>
        <h2>信息</h2>
        <button type="button" aria-label="关闭信息" onClick={onClose}>
          ×
        </button>
      </header>
      {files.length === 0 && !hasSelection && <p>请选择文件或文件夹以查看信息。</p>}
      {files.length === 1 &&
        onlyFile !== undefined &&
        (selectionInfo?.types.folders ?? 0) === 0 && (
          <SingleFileInfo file={onlyFile} dimensions={dimensions} />
        )}
      {selectionInfo !== undefined &&
        selectionInfo !== null &&
        (selectionInfo.relativePaths.length > 1 || selectionInfo.types.folders > 0) && (
          <AggregateSelectionInfo info={selectionInfo} />
        )}
      {files.length > 1 && selectionInfo === undefined && (
        <FallbackMultiFileInfo files={files.length} totalSize={totalSize} counts={counts} />
      )}
    </aside>
  )
}

function SingleFileInfo({
  file,
  dimensions,
}: {
  file: BrowserFile
  dimensions: InfoOverlayProps['dimensions']
}) {
  const size = dimensions[file.entityId]
  return (
    <>
      <InfoSection heading="身份与位置" headingId="info-identity">
        <dl>
          <dt>名称</dt>
          <dd>{file.name}</dd>
          <dt>路径</dt>
          <dd>{file.relativePath}</dd>
          <dt>类型</dt>
          <dd>{kindLabel(file.kind)}</dd>
        </dl>
      </InfoSection>
      <InfoSection heading="审阅信息" headingId="info-review">
        <dl>
          <dt>审阅状态</dt>
          <dd>{reviewLabel(file.marker.reviewState)}</dd>
          <dt>收藏</dt>
          <dd>{file.marker.favorite ? '是' : '否'}</dd>
        </dl>
      </InfoSection>
      <InfoSection heading="技术信息" headingId="info-technical">
        <dl>
          {size && (
            <>
              <dt>尺寸</dt>
              <dd>
                {size.width} × {size.height}
              </dd>
            </>
          )}
          <dt>大小</dt>
          <dd>{formatBytes(file.size)}</dd>
          <dt>修改时间</dt>
          <dd>{formatModifiedNs(file.modifiedNs)}</dd>
        </dl>
      </InfoSection>
    </>
  )
}

function AggregateSelectionInfo({ info }: { info: SelectionInfo }) {
  return (
    <>
      <InfoSection heading="身份与位置" headingId="info-identity">
        <dl>
          <dt>所选项目</dt>
          <dd>{info.relativePaths.length} 个项目</dd>
          <dt>类型</dt>
          <dd>
            文件夹 {info.types.folders} · 图片 {info.types.images} · 其它文件{' '}
            {info.types.otherFiles}
          </dd>
        </dl>
      </InfoSection>
      <InfoSection heading="审阅信息" headingId="info-review">
        <dl>
          <dt>审阅状态</dt>
          <dd>{agreementLabel(info.commonReview, reviewLabel)}</dd>
          <dt>收藏</dt>
          <dd>{agreementLabel(info.commonFavorite, (value) => (value ? '是' : '否'))}</dd>
        </dl>
      </InfoSection>
      <InfoSection heading="技术信息" headingId="info-technical">
        <dl>
          <dt>总大小</dt>
          <dd>{formatBytes(info.totalSize)}</dd>
        </dl>
      </InfoSection>
    </>
  )
}

function FallbackMultiFileInfo({
  files,
  totalSize,
  counts,
}: {
  files: number
  totalSize: number
  counts: Record<string, number>
}) {
  return (
    <>
      <InfoSection heading="身份与位置" headingId="info-identity">
        <dl>
          <dt>所选项目</dt>
          <dd>{files} 个文件</dd>
          <dt>类型</dt>
          <dd>
            {Object.entries(counts)
              .map(([kind, count]) => `${kindLabel(kind)} ${count}`)
              .join(' · ')}
          </dd>
        </dl>
      </InfoSection>
      <InfoSection heading="审阅信息" headingId="info-review">
        <p className="aggregate-label">尚未载入共同审阅信息。</p>
      </InfoSection>
      <InfoSection heading="技术信息" headingId="info-technical">
        <dl>
          <dt>总大小</dt>
          <dd>{formatBytes(totalSize)}</dd>
        </dl>
      </InfoSection>
    </>
  )
}

function InfoSection({
  heading,
  headingId,
  children,
}: {
  heading: string
  headingId: string
  children: ReactNode
}) {
  return (
    <section className="info-section" role="group" aria-labelledby={headingId}>
      <h3 id={headingId}>{heading}</h3>
      {children}
    </section>
  )
}

function agreementLabel<T>(agreement: SelectionAgreement<T>, label: (value: T) => string): string {
  if (agreement.state === 'common') return label(agreement.value)
  if (agreement.state === 'mixed') return '混合'
  return '未选择'
}

function reviewLabel(value: ReviewState | null): string {
  if (value === 'keep') return '保留'
  if (value === 'pending') return '待定'
  if (value === 'reject') return '淘汰'
  return '未标记'
}

function kindLabel(kind: string): string {
  return (
    {
      jpeg: 'JPEG',
      png: 'PNG',
      markdown: 'Markdown',
      text: 'TXT',
      unsupported_image: '其它图片',
      other: '其它文件',
    }[kind] ?? kind
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`
  if (bytes < 1_024 ** 2) return `${formatUnit(bytes / 1_024)} KiB`
  if (bytes < 1_024 ** 3) return `${formatUnit(bytes / 1_024 ** 2)} MiB`
  return `${formatUnit(bytes / 1_024 ** 3)} GiB`
}

function formatUnit(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1)
}

function formatModifiedNs(value: string): string {
  try {
    const milliseconds = Number(BigInt(value) / 1_000_000n)
    if (!Number.isFinite(milliseconds)) return '未知'
    return new Date(milliseconds).toLocaleString()
  } catch {
    return '未知'
  }
}
