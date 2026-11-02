import { useEffect, useRef, useState } from 'react'
import type { BrowserFile } from '../api/types'
import { fileExtensionLabel } from '../fileKinds'
import InfoOverlay from './InfoOverlay'
import UnsupportedFileState from './UnsupportedFileState'
import ViewerButton, { ViewerIconButton } from './ui/ViewerButton'
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
  const [fileInfoOpen, setFileInfoOpen] = useState(false)

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
        leading={
          <>
            <ViewerIconButton
              icon="arrow-left"
              label="返回"
              title="返回网格"
              tone="quiet"
              className="preview-back-action"
              onClick={onClose}
            />
            <div className="preview-identity">
              <strong>{file.name}</strong>
              <span>
                {fileExtensionLabel(file.name).replace(/^\./, '')} · {formatBytes(file.size)}
              </span>
            </div>
          </>
        }
        actions={
          <>
            <ViewerButton
              active={fileInfoOpen}
              className="preview-file-info-action"
              onClick={() => setFileInfoOpen((current) => !current)}
            >
              文件信息
            </ViewerButton>
            <ViewerButton
              className="preview-complete-action"
              aria-label="返回网格"
              onClick={onClose}
            >
              返回网格
            </ViewerButton>
          </>
        }
      />
      <div className="preview-stage">
        <UnsupportedFileState
          file={file}
          unavailable={unavailable}
          onShowInfo={() => setFileInfoOpen(true)}
        />
      </div>
      {fileInfoOpen && (
        <InfoOverlay files={[file]} dimensions={{}} onClose={() => setFileInfoOpen(false)} />
      )}
    </section>
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`
  if (bytes < 1_024 ** 2) return `${formatUnit(bytes / 1_024)} KiB`
  if (bytes < 1_024 ** 3) return `${formatUnit(bytes / 1_024 ** 2)} MiB`
  return `${formatUnit(bytes / 1_024 ** 3)} GiB`
}

function formatUnit(value: number): string {
  return value >= 10 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, '')
}
