import type { MouseEvent } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import type { BrowserFile, TextEncoding, TextPreview as TextPreviewDto } from '../api/types'
import { safeUserMessage } from '../api/viewer'
import ViewerField from './ui/ViewerField'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

export type TextPaneStatus = 'running' | 'complete' | 'failed'

export interface TextPreviewPaneProps {
  file: BrowserFile
  unavailable: boolean
  requestPreview(file: BrowserFile, encoding?: TextEncoding): Promise<TextPreviewDto>
  openExternalLink(url: string): Promise<void>
  onStatusChange(entityId: string, status: TextPaneStatus): void
}

export default function TextPreviewPane({
  file,
  unavailable,
  requestPreview,
  openExternalLink,
  onStatusChange,
}: TextPreviewPaneProps) {
  const [preview, setPreview] = useState<TextPreviewDto | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [encodingRequired, setEncodingRequired] = useState(false)
  const [selectedEncoding, setSelectedEncoding] = useState<TextEncoding | ''>('')
  const requestGeneration = useRef(0)
  const headingId = `text-pane-heading-${file.entityId}`

  const load = useCallback(
    async (encoding?: TextEncoding) => {
      const generation = ++requestGeneration.current
      setPreview(null)
      setError(null)
      setEncodingRequired(false)
      onStatusChange(file.entityId, 'running')
      try {
        const next = await requestPreview(file, encoding)
        if (generation !== requestGeneration.current) return
        setPreview(next)
        onStatusChange(file.entityId, 'complete')
      } catch (caught) {
        if (generation !== requestGeneration.current) return
        setError(safeUserMessage(caught))
        setEncodingRequired(commandCode(caught) === 'text_encoding_required')
        onStatusChange(file.entityId, 'failed')
      }
    },
    [file.entityId, file.modifiedNs, onStatusChange, requestPreview],
  )

  useEffect(() => {
    setPreview(null)
    setError(null)
    setEncodingRequired(false)
    setSelectedEncoding('')
    if (unavailable) {
      requestGeneration.current += 1
      onStatusChange(file.entityId, 'failed')
      return
    }
    void load()
  }, [file.entityId, file.modifiedNs, load, onStatusChange, unavailable])

  useEffect(
    () => () => {
      requestGeneration.current += 1
    },
    [],
  )

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
      className="text-preview-pane"
      aria-labelledby={headingId}
      tabIndex={0}
      data-testid={`text-pane-${file.entityId}`}
    >
      <header className="text-preview-pane-toolbar">
        <h2 id={headingId}>{file.name}</h2>
        <ViewerField label="文本编码" inline className="text-encoding-field">
          <select
            aria-label={`${file.name} 文本编码`}
            value={selectedEncoding || preview?.encoding || ''}
            disabled={unavailable}
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
        </ViewerField>
      </header>
      {unavailable && (
        <ViewerLocalFeedback tone="danger" title="文件已不可用">
          {file.name} 已移动、删除或暂时无法访问。
        </ViewerLocalFeedback>
      )}
      {!unavailable && preview === null && error === null && (
        <ViewerLocalFeedback tone="info" title="正在读取文本">
          正在识别内容与文本编码…
        </ViewerLocalFeedback>
      )}
      {!unavailable && error && (
        <ViewerLocalFeedback
          tone="danger"
          title={encodingRequired ? '需要选择文本编码' : '无法读取文本'}
        >
          {error}
          {encodingRequired ? ' 请选择适合当前文件的文本编码。' : ''}
        </ViewerLocalFeedback>
      )}
      {!unavailable && preview?.format === 'plain_text' && (
        <pre className="plain-text-preview">{preview.plainText}</pre>
      )}
      {!unavailable && preview?.format === 'markdown' && (
        <div
          className="markdown-preview markdown-reading-surface"
          onClick={markdownClicked}
          dangerouslySetInnerHTML={{ __html: preview.markdownHtml ?? '' }}
        />
      )}
      {!unavailable && preview?.truncated && (
        <ViewerLocalFeedback tone="warning" title="内容已截断">
          仅显示前 10 MiB
        </ViewerLocalFeedback>
      )}
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
