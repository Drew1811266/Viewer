import type { BrowserFile } from '../api/types'
import { fileExtensionLabel } from '../fileKinds'

export interface UnsupportedFileStateProps {
  file: Pick<BrowserFile, 'name'>
  compact?: boolean
  unavailable?: boolean
}

export default function UnsupportedFileState({
  file,
  compact = false,
  unavailable = false,
}: UnsupportedFileStateProps) {
  const format = fileExtensionLabel(file.name)
  const status = unavailable ? '文件已不可用' : '暂不支持预览'
  const accessibleLabel = `${file.name} ${format} ${status}`

  return (
    <div
      className={`unsupported-file-state${compact ? ' unsupported-file-state--compact' : ''}`}
      aria-label={accessibleLabel}
    >
      <strong>{file.name}</strong>
      <span>{format}</span>
      <span className="unsupported-file-message">{status}</span>
    </div>
  )
}
