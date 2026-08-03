import { useEffect, useRef } from 'react'
import type { BrowserFile } from '../api/types'
import UnsupportedFileState from './UnsupportedFileState'
import ViewerButton from './ui/ViewerButton'
import ViewerToolbar from './ui/ViewerToolbar'

interface UnsupportedFilePreviewProps {
  file: BrowserFile
  unavailable?: boolean
  onClose: () => void
}

export default function UnsupportedFilePreview({
  file,
  unavailable = false,
  onClose,
}: UnsupportedFilePreviewProps) {
  const dialog = useRef<HTMLElement>(null)

  useEffect(() => {
    const previous = document.activeElement
    dialog.current?.focus()
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [])

  return (
    <section
      ref={dialog}
      className="preview-overlay unsupported-file-preview"
      role="dialog"
      aria-label={file.name}
      tabIndex={-1}
      onKeyDown={(event) => {
        if (event.key !== 'Escape') return
        event.preventDefault()
        onClose()
      }}
    >
      <ViewerToolbar
        label="文件预览工具"
        leading={<strong>{file.name}</strong>}
        actions={
          <ViewerButton
            tone="quiet"
            className="preview-complete-action"
            aria-label="关闭预览"
            onClick={onClose}
          >
            完成
          </ViewerButton>
        }
      />
      <div className="preview-stage">
        <UnsupportedFileState file={file} unavailable={unavailable} />
      </div>
    </section>
  )
}
