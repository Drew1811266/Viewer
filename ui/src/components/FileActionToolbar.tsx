interface FileActionToolbarProps {
  selectedCount: number
  selectedImageCount: number
  readOnly: boolean
  busy: boolean
  compareContextAvailable?: boolean
  onRename: () => void
  onCopy: () => void
  onMove: () => void
  onTrash: () => void
  onCompare: () => void
  onInfo: () => void
}

export default function FileActionToolbar({
  selectedCount,
  selectedImageCount,
  readOnly,
  busy,
  compareContextAvailable = true,
  onRename,
  onCopy,
  onMove,
  onTrash,
  onCompare,
  onInfo,
}: FileActionToolbarProps) {
  const writesDisabled = selectedCount === 0 || readOnly || busy
  const compareDisabled =
    selectedCount !== selectedImageCount ||
    !compareContextAvailable ||
    selectedImageCount < 2 ||
    selectedImageCount > 4 ||
    busy
  return (
    <div className="file-action-toolbar" aria-label="文件操作">
      <span>{selectedCount > 0 ? `已选择 ${selectedCount} 项` : '未选择文件'}</span>
      <button type="button" disabled={compareDisabled} onClick={onCompare}>
        并排对比
      </button>
      <button type="button" disabled={selectedCount === 0} onClick={onInfo}>
        信息
      </button>
      <button type="button" disabled={writesDisabled} onClick={onRename}>
        {selectedCount > 1 ? '批量重命名' : '重命名'}
      </button>
      <button type="button" disabled={writesDisabled} onClick={onCopy}>
        复制到…
      </button>
      <button type="button" disabled={writesDisabled} onClick={onMove}>
        移动到…
      </button>
      <button
        type="button"
        className="destructive-button"
        disabled={writesDisabled}
        onClick={onTrash}
      >
        移到废纸篓
      </button>
    </div>
  )
}
