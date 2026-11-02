import type { BrowserFile } from '../api/types'
import { fileExtensionLabel } from '../fileKinds'
import ViewerButton from './ui/ViewerButton'
import ViewerIcon from './ui/ViewerIcon'

export interface UnsupportedFileStateProps {
  file: Pick<BrowserFile, 'name' | 'kind'>
  compact?: boolean
  unavailable?: boolean
  onShowInfo?: () => void
}

export default function UnsupportedFileState({
  file,
  compact = false,
  unavailable = false,
  onShowInfo,
}: UnsupportedFileStateProps) {
  const format = fileExtensionLabel(file.name)
  const status = unavailable ? '文件已不可用' : '暂不支持预览'
  const accessibleLabel = `${file.name} ${format} ${status}`

  if (compact) {
    return (
      <div
        className="unsupported-file-state unsupported-file-state--compact"
        aria-label={accessibleLabel}
      >
        <strong>{file.name}</strong>
        <span>{format}</span>
        <span className="unsupported-file-message">{status}</span>
      </div>
    )
  }

  return (
    <div className="unsupported-file-state" aria-label={accessibleLabel}>
      <span className="unsupported-file-state__icon">
        <ViewerIcon name={file.kind === 'unsupported_image' ? 'eye' : 'folder'} size={24} />
      </span>
      <strong className="unsupported-file-state__title">
        {unavailable ? '文件已不可用' : '此格式暂不支持预览'}
      </strong>
      <p className="unsupported-file-state__message">
        {unavailable
          ? '该文件已移动、删除或暂时无法访问。你仍然可以查看文件信息。'
          : '仍然可以在项目中选择、整理和查看文件信息；Viewer 不会尝试解码。'}
      </p>
      <span className="unsupported-file-state__format">
        {format} · {categoryLabel(file)}
      </span>
      {onShowInfo !== undefined && (
        <ViewerButton leadingIcon="info" onClick={onShowInfo}>
          查看文件信息
        </ViewerButton>
      )}
    </div>
  )
}

function categoryLabel(file: Pick<BrowserFile, 'kind' | 'name'>): string {
  if (file.kind === 'unsupported_image') return '图片'
  const extension = file.name.split('.').pop()?.toLowerCase() ?? ''
  if (['zip', 'rar', '7z', 'tar', 'gz'].includes(extension)) return '归档'
  return '其它文件'
}
