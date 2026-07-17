import { useCallback, useEffect, useRef, useState } from 'react'
import type { MouseEvent } from 'react'
import { safeUserMessage } from '../api/viewer'
import type {
  BrowserFile,
  TextEncoding,
  TextPreview as TextPreviewDto,
} from '../api/types'
import type { TaskFeedback } from './TaskBar'

interface TextPreviewProps {
  file: BrowserFile
  requestPreview: (file: BrowserFile, encoding?: TextEncoding) => Promise<TextPreviewDto>
  openExternalLink: (url: string) => Promise<void>
  onClose: () => void
  onTaskChange?: (task: TaskFeedback | null) => void
}

export default function TextPreview({
  file,
  requestPreview,
  openExternalLink,
  onClose,
  onTaskChange,
}: TextPreviewProps) {
  const [preview, setPreview] = useState<TextPreviewDto | null>(null)
  const dialog = useRef<HTMLElement>(null)
  const [error, setError] = useState<string | null>(null)
  const [encodingRequired, setEncodingRequired] = useState(false)
  const [selectedEncoding, setSelectedEncoding] = useState<TextEncoding | ''>('')

  const load = useCallback(
    async (encoding?: TextEncoding) => {
      setPreview(null)
      setError(null)
      setEncodingRequired(false)
      onTaskChange?.({
        id: `text-preview-${file.entityId}`,
        label: '读取文本预览',
        status: 'running',
        requested: 1,
        completed: 0,
        failed: 0,
        cancellable: false,
        failures: [],
      })
      try {
        const next = await requestPreview(file, encoding)
        setPreview(next)
        onTaskChange?.({
          id: `text-preview-${file.entityId}`,
          label: '读取文本预览',
          status: 'complete',
          requested: 1,
          completed: 1,
          failed: 0,
          cancellable: false,
          failures: [],
        })
      } catch (caught) {
        setError(safeUserMessage(caught))
        setEncodingRequired(commandCode(caught) === 'text_encoding_required')
        onTaskChange?.({
          id: `text-preview-${file.entityId}`,
          label: '读取文本预览',
          status: 'failed',
          requested: 1,
          completed: 0,
          failed: 1,
          cancellable: false,
          failures: [],
        })
      }
    },
    [file.entityId, file.modifiedNs, onTaskChange, requestPreview],
  )

  useEffect(() => {
    const previous = document.activeElement
    dialog.current?.focus()
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus()
    }
  }, [])

  useEffect(() => {
    setSelectedEncoding('')
    void load()
    return () => onTaskChange?.(null)
  }, [load, onTaskChange])

  function markdownClicked(event: MouseEvent<HTMLDivElement>) {
    const target = event.target
    if (!(target instanceof Element)) return
    const anchor = target.closest('a')
    const href = anchor?.getAttribute('href')
    if (!href) return
    event.preventDefault()
    void openExternalLink(href).catch(() => setError('无法使用系统浏览器打开该链接。'))
  }

  return (
    <section
      ref={dialog}
      className="preview-overlay text-preview"
      role="dialog"
      aria-label={file.name}
      tabIndex={-1}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault()
          onClose()
        }
      }}
    >
      <header className="preview-toolbar">
        <strong>{file.name}</strong>
        <label>
          文本编码
          <select
            aria-label="文本编码"
            value={selectedEncoding || preview?.encoding || ''}
            onChange={(event) => {
              const encoding = event.currentTarget.value as TextEncoding
              setSelectedEncoding(encoding)
              void load(encoding)
            }}
          >
            <option value="" disabled>
              自动识别
            </option>
            <option value="utf8">UTF-8</option>
            <option value="utf16_le">UTF-16 LE</option>
            <option value="utf16_be">UTF-16 BE</option>
            <option value="gb18030">GB18030</option>
          </select>
        </label>
        <button type="button" aria-label="关闭预览" onClick={onClose}>
          ×
        </button>
      </header>
      {preview === null && error === null && <p>正在读取文本…</p>}
      {error && <p role="alert">{error}</p>}
      {encodingRequired && <p>请选择适合当前文件的文本编码。</p>}
      {preview?.format === 'plain_text' && (
        <pre className="plain-text-preview">{preview.plainText}</pre>
      )}
      {preview?.format === 'markdown' && (
        <div
          className="markdown-preview"
          onClick={markdownClicked}
          dangerouslySetInnerHTML={{ __html: preview.markdownHtml ?? '' }}
        />
      )}
      {preview?.truncated && <p className="truncation-note">仅显示前 10 MiB</p>}
    </section>
  )
}

function commandCode(error: unknown): string | null {
  if (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    typeof error.code === 'string'
  ) {
    return error.code
  }
  return null
}
