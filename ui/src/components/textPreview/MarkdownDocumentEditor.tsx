import { markdown as markdownLanguage } from '@codemirror/lang-markdown'
import CodeMirror from '@uiw/react-codemirror'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  BrowserFile,
  SaveTextRequest,
  SaveTextResult,
  TextPreview as TextPreviewDto,
} from '../../api/types'
import { safeUserMessage } from '../../api/viewer'
import ViewerButton from '../ui/ViewerButton'
import ViewerDialog from '../ui/ViewerDialog'
import ViewerLocalFeedback from '../ui/ViewerLocalFeedback'

export interface MarkdownDocumentEditorProps {
  file: BrowserFile
  preview: TextPreviewDto
  readOnly: boolean
  saveText(request: SaveTextRequest): Promise<SaveTextResult>
  renderMarkdown(content: string): Promise<string>
  openExternalLink(url: string): Promise<void>
  onSaved(entityId: string): void
}

type SaveIssue = 'conflict' | 'encoding' | null

export default function MarkdownDocumentEditor({
  file,
  preview,
  readOnly,
  saveText,
  renderMarkdown,
  openExternalLink,
  onSaved,
}: MarkdownDocumentEditorProps) {
  const [editing, setEditing] = useState(false)
  const [content, setContent] = useState(preview.plainText ?? '')
  const [fingerprint, setFingerprint] = useState(preview.fingerprint)
  const [entityId, setEntityId] = useState(preview.entityId)
  const [savedContent, setSavedContent] = useState(preview.plainText ?? '')
  const [renderedHtml, setRenderedHtml] = useState(preview.markdownHtml ?? '')
  const [saving, setSaving] = useState(false)
  const [issue, setIssue] = useState<SaveIssue>(null)
  const [error, setError] = useState<string | null>(null)
  const pendingAcknowledge = useRef(false)
  const renderGeneration = useRef(0)

  const dirty = editing && content !== savedContent
  const editable = !readOnly && !preview.truncated

  const extensions = useMemo(() => [markdownLanguage()], [])

  const refreshLivePreview = useCallback(
    (next: string) => {
      const generation = ++renderGeneration.current
      void renderMarkdown(next)
        .then((html) => {
          if (generation === renderGeneration.current) setRenderedHtml(html)
        })
        .catch(() => {
          /* keep the last rendered preview; transient render failures are not fatal */
        })
    },
    [renderMarkdown],
  )

  useEffect(() => {
    if (!editing) return
    const timer = window.setTimeout(() => refreshLivePreview(content), 400)
    return () => window.clearTimeout(timer)
  }, [content, editing, refreshLivePreview])

  const performSave = useCallback(
    async (acknowledge: boolean) => {
      setSaving(true)
      setError(null)
      try {
        const result = await saveText({
          entityId,
          content,
          expectedFingerprint: fingerprint,
          newlineStyle: preview.usesCrlf ? 'crlf' : 'lf',
          acknowledgeEncodingConversion: acknowledge,
        })
        pendingAcknowledge.current = false
        setFingerprint(result.fingerprint)
        setEntityId(result.entityId)
        setSavedContent(content)
        onSaved(result.entityId)
      } catch (caught) {
        const code = commandCode(caught)
        if (code === 'text_save_conflict') {
          setIssue('conflict')
        } else if (code === 'text_encoding_acknowledgement_required') {
          pendingAcknowledge.current = true
          setIssue('encoding')
        } else {
          setError(safeUserMessage(caught))
        }
      } finally {
        setSaving(false)
      }
    },
    [content, entityId, fingerprint, onSaved, preview.usesCrlf, saveText],
  )

  useEffect(() => {
    if (!editing) return
    function interceptSave(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault()
        if (!saving) void performSave(pendingAcknowledge.current)
      }
    }
    window.addEventListener('keydown', interceptSave, true)
    return () => window.removeEventListener('keydown', interceptSave, true)
  }, [editing, performSave, saving])

  function markdownClicked(event: React.MouseEvent<HTMLDivElement>) {
    const target = event.target
    if (!(target instanceof Element)) return
    const anchor = target.closest('a')
    const href = anchor?.getAttribute('href')
    if (!href) return
    event.preventDefault()
    void openExternalLink(href).catch(() => setError('无法使用系统浏览器打开该链接。'))
  }

  function discardAndReload() {
    setIssue(null)
    setEditing(false)
    setContent(savedContent)
    setRenderedHtml(preview.markdownHtml ?? '')
    onSaved(entityId)
  }

  return (
    <div className="markdown-document-editor">
      {!editing ? (
        <>
          {editable && (
            <div className="markdown-editor-actions">
              <ViewerButton tone="quiet" onClick={() => setEditing(true)}>
                编辑
              </ViewerButton>
            </div>
          )}
          {readOnly && (
            <ViewerLocalFeedback tone="info" title="只读项目">
              当前项目为只读，文档以阅读模式打开。
            </ViewerLocalFeedback>
          )}
          <div
            className="markdown-preview markdown-reading-surface"
            onClick={markdownClicked}
            dangerouslySetInnerHTML={{ __html: preview.markdownHtml ?? '' }}
          />
        </>
      ) : (
        <>
          <div className="markdown-editor-actions" data-dirty={dirty ? 'true' : 'false'}>
            <ViewerButton
              tone="quiet"
              onClick={() => {
                setEditing(false)
                setContent(savedContent)
                setRenderedHtml(preview.markdownHtml ?? '')
              }}
            >
              完成编辑
            </ViewerButton>
            <ViewerButton
              tone="primary"
              disabled={!dirty || saving}
              onClick={() => void performSave(pendingAcknowledge.current)}
            >
              {saving ? '保存中…' : dirty ? '保存' : '已保存'}
            </ViewerButton>
          </div>
          {error && (
            <ViewerLocalFeedback tone="danger" title="保存失败">
              {error}
            </ViewerLocalFeedback>
          )}
          <div className="markdown-editor-columns">
            <div className="markdown-editor-source">
              <CodeMirror
                value={content}
                extensions={extensions}
                height="100%"
                theme="light"
                basicSetup={{ foldGutter: false, highlightActiveLine: false }}
                onChange={setContent}
                aria-label={`${file.name} 源码编辑器`}
              />
            </div>
            <div
              className="markdown-preview markdown-reading-surface"
              onClick={markdownClicked}
              dangerouslySetInnerHTML={{ __html: renderedHtml }}
            />
          </div>
          <ViewerLocalFeedback tone="info" title="编辑提示">
            ⌘S 保存；预览栏实时渲染未保存内容（项目内图片在保存后解析）。
          </ViewerLocalFeedback>
        </>
      )}
      {issue === 'conflict' && (
        <ViewerDialog
          title="文件已被外部修改"
          description="自加载以来该文件在 Viewer 之外被修改，本次保存未执行。"
          onCancel={() => setIssue(null)}
        >
          <p>选择“放弃修改并重载”将丢弃当前编辑内容并重新读取文件。</p>
          <ViewerButton tone="quiet" onClick={discardAndReload}>
            放弃修改并重载
          </ViewerButton>
          <ViewerButton tone="primary" onClick={() => setIssue(null)}>
            继续编辑
          </ViewerButton>
        </ViewerDialog>
      )}
      {issue === 'encoding' && (
        <ViewerDialog
          title="保存将转换编码为 UTF-8"
          description="该文件当前不是 UTF-8 编码；保存会以 UTF-8 重写整个文件。"
          onCancel={() => setIssue(null)}
        >
          <p>确认继续将以 UTF-8 编码写回文件内容。</p>
          <ViewerButton tone="quiet" onClick={() => setIssue(null)}>
            取消
          </ViewerButton>
          <ViewerButton tone="primary" onClick={() => void performSave(true)}>
            确认转换并保存
          </ViewerButton>
        </ViewerDialog>
      )}
    </div>
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
