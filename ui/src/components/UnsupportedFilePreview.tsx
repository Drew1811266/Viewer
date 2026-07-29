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
        <strong>{file.name}</strong>
        <button type="button" aria-label="关闭预览" onClick={onClose}>
          ×
        </button>
      </header>
      <UnsupportedFileState file={file} unavailable={unavailable} />
    </section>
  )
}
