import type { BrowserFile } from '../api/types'

interface InfoOverlayProps {
  files: BrowserFile[]
  dimensions: Record<string, { width: number; height: number } | undefined>
  onClose: () => void
}

export default function InfoOverlay({ files, dimensions, onClose }: InfoOverlayProps) {
  const totalSize = files.reduce((sum, file) => sum + file.size, 0)
  const counts = files.reduce<Record<string, number>>((result, file) => {
    result[file.kind] = (result[file.kind] ?? 0) + 1
    return result
  }, {})

  return (
    <aside className="info-overlay" aria-label="文件信息">
      <header>
        <h2>信息</h2>
        <button type="button" aria-label="关闭信息" onClick={onClose}>
          ×
        </button>
      </header>
      {files.length === 0 && <p>请选择文件以查看信息。</p>}
      {files.length === 1 && <SingleFileInfo file={files[0]} dimensions={dimensions} />}
      {files.length > 1 && (
        <dl>
          <dt>所选项目</dt>
          <dd>{files.length} 个文件</dd>
          <dt>总大小</dt>
          <dd>{formatBytes(totalSize)}</dd>
          <dt>类型</dt>
          <dd>
            {Object.entries(counts)
              .map(([kind, count]) => `${kindLabel(kind)} ${count}`)
              .join(' · ')}
          </dd>
        </dl>
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
    <dl>
      <dt>名称</dt>
      <dd>{file.name}</dd>
      <dt>路径</dt>
      <dd>{file.relativePath}</dd>
      <dt>类型</dt>
      <dd>{kindLabel(file.kind)}</dd>
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
  )
}

function kindLabel(kind: string): string {
  return (
    {
      jpeg: 'JPEG',
      png: 'PNG',
      markdown: 'Markdown',
      text: 'TXT',
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
