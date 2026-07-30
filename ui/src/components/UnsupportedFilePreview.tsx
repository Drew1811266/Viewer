import { useEffect, useRef } from 'react'
import type { BrowserFile } from '../api/types'
import UnsupportedFileState from './UnsupportedFileState'

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
      <header className="preview-toolbar">
        <div className="preview-toolbar-leading">
          <strong>{file.name}</strong>
        </div>
        <div className="preview-toolbar-actions" role="toolbar" aria-label="文件预览控制">
          <button type="button" aria-label="关闭预览" onClick={onClose}>
            完成
          </button>
        </div>
      </header>
      <div className="preview-stage">
        <UnsupportedFileState file={file} unavailable={unavailable} />
      </div>
    </section>
  )
}
