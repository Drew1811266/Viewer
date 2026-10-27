import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@uiw/react-codemirror', () => ({
  default: ({
    value,
    onChange,
    'aria-label': ariaLabel,
  }: {
    value: string
    onChange: (next: string) => void
    'aria-label'?: string
  }) => (
    <textarea
      aria-label={ariaLabel}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}))
vi.mock('@codemirror/lang-markdown', () => ({ markdown: () => [] }))

import type { BrowserFile, SaveTextResult, TextPreview } from '../../api/types'
import MarkdownDocumentEditor from './MarkdownDocumentEditor'

const markdownFile: BrowserFile = {
  entityId: 'doc-1',
  relativePath: 'notes/doc.md',
  name: 'doc.md',
  kind: 'markdown',
  size: 128,
  modifiedNs: '1000',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
  videoMetadata: null,
}

function preview(overrides: Partial<TextPreview> = {}): TextPreview {
  return {
    entityId: 'doc-1',
    format: 'markdown',
    plainText: '# 标题\n\n初始内容。',
    markdownHtml: '<h1>标题</h1><p>初始内容。</p>',
    encoding: 'utf8',
    truncated: false,
    fingerprint: 'fingerprint-1',
    usesCrlf: false,
    ...overrides,
  }
}

function saveResult(overrides: Partial<SaveTextResult> = {}): SaveTextResult {
  return { fingerprint: 'fingerprint-2', entityId: 'doc-2', ...overrides }
}

function commandError(code: string) {
  return { code, category: 'conflict', userMessage: '失败', retryable: false }
}

describe('MarkdownDocumentEditor', () => {
  const openExternalLink = vi.fn()
  const onSaved = vi.fn()
  const renderMarkdown = vi.fn().mockResolvedValue('<h1>live</h1>')
  let saveText = vi.fn()

  beforeEach(() => {
    saveText = vi.fn().mockResolvedValue(saveResult())
    openExternalLink.mockReset()
    onSaved.mockReset()
    renderMarkdown.mockClear()
  })

  it('shows read-only feedback and no edit entry for read-only projects', () => {
    render(
      <MarkdownDocumentEditor
        file={markdownFile}
        preview={preview()}
        readOnly
        saveText={saveText}
        renderMarkdown={renderMarkdown}
        openExternalLink={openExternalLink}
        onSaved={onSaved}
      />,
    )

    expect(screen.getByText('只读项目')).toBeTruthy()
    expect(screen.queryByRole('button', { name: '编辑' })).toBeNull()
    expect(saveText).not.toHaveBeenCalled()
  })

  it('saves edits, adopts the rekeyed entity and refreshed fingerprint', async () => {
    render(
      <MarkdownDocumentEditor
        file={markdownFile}
        preview={preview()}
        readOnly={false}
        saveText={saveText}
        renderMarkdown={renderMarkdown}
        openExternalLink={openExternalLink}
        onSaved={onSaved}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '编辑' }))
    const editor = await screen.findByLabelText('doc.md 源码编辑器')
    fireEvent.change(editor, { target: { value: '# 标题\n\n修改后的内容。' } })

    const save = await screen.findByRole('button', { name: '保存' })
    fireEvent.click(save)

    await waitFor(() => expect(saveText).toHaveBeenCalledTimes(1))
    expect(saveText).toHaveBeenCalledWith({
      entityId: 'doc-1',
      content: '# 标题\n\n修改后的内容。',
      expectedFingerprint: 'fingerprint-1',
      newlineStyle: 'lf',
      acknowledgeEncodingConversion: false,
    })
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith('doc-2'))

    // A second save addresses the rekeyed entity with the new fingerprint.
    fireEvent.change(editor, { target: { value: '# 标题\n\n再次修改。' } })
    const saveAgain = await screen.findByRole('button', { name: '保存' })
    fireEvent.click(saveAgain)
    await waitFor(() => expect(saveText).toHaveBeenCalledTimes(2))
    expect(saveText).toHaveBeenLastCalledWith({
      entityId: 'doc-2',
      content: '# 标题\n\n再次修改。',
      expectedFingerprint: 'fingerprint-2',
      newlineStyle: 'lf',
      acknowledgeEncodingConversion: false,
    })
  })

  it('a conflict blocks the overwrite and offers reload or continue editing', async () => {
    saveText.mockRejectedValue(commandError('text_save_conflict'))
    render(
      <MarkdownDocumentEditor
        file={markdownFile}
        preview={preview()}
        readOnly={false}
        saveText={saveText}
        renderMarkdown={renderMarkdown}
        openExternalLink={openExternalLink}
        onSaved={onSaved}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '编辑' }))
    const editor = await screen.findByLabelText('doc.md 源码编辑器')
    fireEvent.change(editor, { target: { value: '本地修改' } })
    fireEvent.click(await screen.findByRole('button', { name: '保存' }))

    expect(await screen.findByText('文件已被外部修改')).toBeTruthy()
    expect(saveText).toHaveBeenCalledTimes(1)

    fireEvent.click(screen.getByRole('button', { name: '放弃修改并重载' }))
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith('doc-1'))
    // The editor left editing mode without a second save attempt.
    expect(saveText).toHaveBeenCalledTimes(1)
    expect(screen.queryByLabelText('doc.md 源码编辑器')).toBeNull()
  })

  it('a conflict can save the current content as a sibling copy', async () => {
    saveText
      .mockRejectedValueOnce(commandError('text_save_conflict'))
      .mockResolvedValueOnce(saveResult({ createdPath: 'notes/doc-copy.md' }))
    render(
      <MarkdownDocumentEditor
        file={markdownFile}
        preview={preview()}
        readOnly={false}
        saveText={saveText}
        renderMarkdown={renderMarkdown}
        openExternalLink={openExternalLink}
        onSaved={onSaved}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '编辑' }))
    const editor = await screen.findByLabelText('doc.md 源码编辑器')
    fireEvent.change(editor, { target: { value: '本地修改' } })
    fireEvent.click(await screen.findByRole('button', { name: '保存' }))
    expect(await screen.findByText('文件已被外部修改')).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: '另存副本' }))
    expect(await screen.findByText('副本已保存为 notes/doc-copy.md。')).toBeTruthy()
    expect(saveText).toHaveBeenLastCalledWith({
      entityId: 'doc-1',
      content: '本地修改',
      expectedFingerprint: 'fingerprint-1',
      newlineStyle: 'lf',
      acknowledgeEncodingConversion: true,
      saveAsCopy: true,
    })
    expect(screen.queryByText('文件已被外部修改')).toBeNull()
    expect(screen.getByLabelText('doc.md 源码编辑器')).toBeTruthy()
  })

  it('an encoding conversion requires an explicit acknowledgement before saving', async () => {
    saveText
      .mockRejectedValueOnce(commandError('text_encoding_acknowledgement_required'))
      .mockResolvedValueOnce(saveResult())
    render(
      <MarkdownDocumentEditor
        file={markdownFile}
        preview={preview({ encoding: 'gb18030' })}
        readOnly={false}
        saveText={saveText}
        renderMarkdown={renderMarkdown}
        openExternalLink={openExternalLink}
        onSaved={onSaved}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '编辑' }))
    const editor = await screen.findByLabelText('doc.md 源码编辑器')
    fireEvent.change(editor, { target: { value: '中文修改' } })
    fireEvent.click(await screen.findByRole('button', { name: '保存' }))

    expect(await screen.findByText('保存将转换编码为 UTF-8')).toBeTruthy()
    expect(saveText).toHaveBeenNthCalledWith(1, {
      entityId: 'doc-1',
      content: '中文修改',
      expectedFingerprint: 'fingerprint-1',
      newlineStyle: 'lf',
      acknowledgeEncodingConversion: false,
    })

    fireEvent.click(screen.getByRole('button', { name: '确认转换并保存' }))
    await waitFor(() =>
      expect(saveText).toHaveBeenNthCalledWith(2, {
        entityId: 'doc-1',
        content: '中文修改',
        expectedFingerprint: 'fingerprint-1',
        newlineStyle: 'lf',
        acknowledgeEncodingConversion: true,
      }),
    )
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith('doc-2'))
  })
})
