import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile, TextEncoding, TextPreview as TextPreviewDto } from '../api/types'
import '../styles/app.css'
import type { TaskFeedback } from './TaskBar'
import TextPreview, { type TextPreviewFiles } from './TextPreview'

const plainTextFile: BrowserFile = {
  entityId: 'text-plain',
  relativePath: 'id-1/plain.txt',
  name: 'plain.txt',
  kind: 'text',
  size: 100,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
}

const markdownFile: BrowserFile = {
  ...plainTextFile,
  entityId: 'text-markdown',
  relativePath: 'id-2/notes.md',
  name: 'notes.md',
  kind: 'markdown',
}

function textPreview(
  file: BrowserFile,
  plainText: string,
  encoding: TextEncoding = 'utf8',
): TextPreviewDto {
  return {
    entityId: file.entityId,
    format: 'plain_text',
    plainText,
    markdownHtml: null,
    encoding,
    truncated: false,
  }
}

function markdownPreview(
  file: BrowserFile,
  markdownHtml: string,
  encoding: TextEncoding = 'utf8',
): TextPreviewDto {
  return {
    entityId: file.entityId,
    format: 'markdown',
    plainText: null,
    markdownHtml,
    encoding,
    truncated: false,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

describe('TextPreview', () => {
  it('loads two panes independently and gives their encoding controls unique labels', async () => {
    const plain = deferred<TextPreviewDto>()
    const markdown = deferred<TextPreviewDto>()
    const requestPreview = vi.fn((file: BrowserFile) =>
      file.entityId === plainTextFile.entityId ? plain.promise : markdown.promise,
    )

    render(
      <TextPreview
        files={[plainTextFile, markdownFile]}
        requestPreview={requestPreview}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: /plain\.txt.*notes\.md/ })
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    const actions = within(dialog).getByRole('toolbar', { name: '文本预览控制' })
    expect(actions).toBeVisible()
    expect(actions).toHaveClass('preview-toolbar-actions')
    expect(getComputedStyle(actions).gridColumn).toBe('3')
    expect(getComputedStyle(actions).justifySelf).toBe('end')
    expect(within(dialog).getByRole('button', { name: '关闭预览' })).toHaveTextContent('完成')
    expect(within(dialog).getAllByRole('region')).toHaveLength(2)
    expect(within(dialog).queryByText(/差异|合并/)).not.toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: 'plain.txt 文本编码' })).toBeVisible()
    expect(screen.getByRole('combobox', { name: 'notes.md 文本编码' })).toBeVisible()
    expect(screen.getByRole('region', { name: 'plain.txt' })).toBeVisible()
    expect(screen.getByRole('region', { name: 'notes.md' })).toBeVisible()

    await act(async () => {
      plain.resolve(textPreview(plainTextFile, 'plain body'))
      await plain.promise
    })
    expect(screen.getByTestId(`text-pane-${plainTextFile.entityId}`)).toHaveTextContent(
      'plain body',
    )
    expect(screen.getByTestId(`text-pane-${markdownFile.entityId}`)).toHaveTextContent(
      '正在读取文本',
    )

    await act(async () => {
      markdown.resolve(markdownPreview(markdownFile, '<h1>Markdown title</h1>'))
      await markdown.promise
    })
    expect(screen.getByTestId(`text-pane-${markdownFile.entityId}`)).toHaveTextContent(
      'Markdown title',
    )
  })

  it('keeps a successful pane visible when its peer fails and reloads only the changed encoding', async () => {
    const onTaskChange = vi.fn<(task: TaskFeedback | null) => void>()
    const requestPreview = vi.fn(
      (file: BrowserFile, encoding?: TextEncoding): Promise<TextPreviewDto> => {
        if (file.entityId === plainTextFile.entityId) {
          return Promise.resolve(textPreview(plainTextFile, 'plain remains'))
        }
        if (encoding === 'gb18030') {
          return Promise.resolve(markdownPreview(markdownFile, '<p>decoded notes</p>', encoding))
        }
        return Promise.reject({
          code: 'text_encoding_required',
          userMessage: '请选择文本编码。',
        })
      },
    )

    render(
      <TextPreview
        files={[plainTextFile, markdownFile]}
        requestPreview={requestPreview}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
        onTaskChange={onTaskChange}
      />,
    )

    expect(await screen.findByText('plain remains')).toBeVisible()
    expect(await screen.findByRole('alert')).toHaveTextContent('请选择文本编码')
    await waitFor(() =>
      expect(onTaskChange).toHaveBeenLastCalledWith(
        expect.objectContaining({
          requested: 2,
          completed: 1,
          failed: 1,
          status: 'failed',
        }),
      ),
    )

    fireEvent.change(screen.getByRole('combobox', { name: 'notes.md 文本编码' }), {
      target: { value: 'gb18030' },
    })

    expect(await screen.findByText('decoded notes')).toBeVisible()
    expect(screen.getByText('plain remains')).toBeVisible()
    expect(
      requestPreview.mock.calls.filter(
        ([requested]) => requested.entityId === plainTextFile.entityId,
      ),
    ).toHaveLength(1)
    expect(
      requestPreview.mock.calls.filter(
        ([requested]) => requested.entityId === markdownFile.entityId,
      ),
    ).toHaveLength(2)
  })

  it('contains focus, closes with Escape, and restores focus to the launcher', async () => {
    function FocusHarness() {
      const [files, setFiles] = useState<TextPreviewFiles | null>(null)
      return (
        <>
          <button type="button" onClick={() => setFiles([plainTextFile, markdownFile])}>
            打开文本预览
          </button>
          {files && (
            <TextPreview
              files={files}
              requestPreview={(file) => Promise.resolve(textPreview(file, file.name))}
              openExternalLink={() => Promise.resolve()}
              onClose={() => setFiles(null)}
            />
          )}
        </>
      )
    }

    render(<FocusHarness />)
    const launcher = screen.getByRole('button', { name: '打开文本预览' })
    launcher.focus()
    fireEvent.click(launcher)

    const dialog = screen.getByRole('dialog')
    const first = screen.getByRole('button', { name: '关闭预览' })
    const last = screen.getByRole('combobox', { name: 'notes.md 文本编码' })
    await waitFor(() => expect(first).toHaveFocus())

    last.focus()
    fireEvent.keyDown(dialog, { key: 'Tab' })
    expect(first).toHaveFocus()

    fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true })
    expect(last).toHaveFocus()

    fireEvent.keyDown(dialog, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(launcher).toHaveFocus()
  })

  it('invalidates pending pane responses and task updates when the dialog unmounts', async () => {
    const plain = deferred<TextPreviewDto>()
    const markdown = deferred<TextPreviewDto>()
    const onTaskChange = vi.fn<(task: TaskFeedback | null) => void>()
    const requestPreview = (file: BrowserFile) =>
      file.entityId === plainTextFile.entityId ? plain.promise : markdown.promise
    const view = render(
      <TextPreview
        files={[plainTextFile, markdownFile]}
        requestPreview={requestPreview}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
        onTaskChange={onTaskChange}
      />,
    )

    await waitFor(() => expect(onTaskChange).toHaveBeenCalled())
    view.unmount()
    const callsAfterUnmount = onTaskChange.mock.calls.length

    await act(async () => {
      plain.resolve(textPreview(plainTextFile, 'stale plain'))
      markdown.resolve(markdownPreview(markdownFile, '<p>stale markdown</p>'))
      await Promise.all([plain.promise, markdown.promise])
    })

    expect(screen.queryByText('stale plain')).not.toBeInTheDocument()
    expect(screen.queryByText('stale markdown')).not.toBeInTheDocument()
    expect(onTaskChange).toHaveBeenCalledTimes(callsAfterUnmount)
    expect(onTaskChange).toHaveBeenLastCalledWith(null)
  })

  it('does not let an old entity response replace a newly rendered pane', async () => {
    const oldRequest = deferred<TextPreviewDto>()
    const newRequest = deferred<TextPreviewDto>()
    const replacementFile = {
      ...plainTextFile,
      entityId: 'text-replacement',
      relativePath: 'id-3/replacement.txt',
      name: 'replacement.txt',
      modifiedNs: '2',
    }
    const requestPreview = (file: BrowserFile) =>
      file.entityId === plainTextFile.entityId ? oldRequest.promise : newRequest.promise
    const view = render(
      <TextPreview
        files={[plainTextFile]}
        requestPreview={requestPreview}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    view.rerender(
      <TextPreview
        files={[replacementFile]}
        requestPreview={requestPreview}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    await act(async () => {
      newRequest.resolve(textPreview(replacementFile, 'new entity body'))
      await newRequest.promise
    })
    expect(screen.getByText('new entity body')).toBeVisible()

    await act(async () => {
      oldRequest.resolve(textPreview(plainTextFile, 'old entity body'))
      await oldRequest.promise
    })
    expect(screen.queryByText('old entity body')).not.toBeInTheDocument()
    expect(screen.getByText('new entity body')).toBeVisible()
  })

  it('marks only an unavailable pane failed without reloading or clearing its peer', async () => {
    const requestPreview = vi.fn((file: BrowserFile) =>
      Promise.resolve(textPreview(file, `${file.name} decoded`)),
    )
    const view = render(
      <TextPreview
        files={[plainTextFile, markdownFile]}
        requestPreview={requestPreview}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(await screen.findByText('plain.txt decoded')).toBeVisible()
    expect(await screen.findByText('notes.md decoded')).toBeVisible()

    view.rerender(
      <TextPreview
        files={[plainTextFile, markdownFile]}
        unavailableEntityIds={new Set([markdownFile.entityId])}
        requestPreview={requestPreview}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByText('plain.txt decoded')).toBeVisible()
    expect(screen.getByTestId(`text-pane-${markdownFile.entityId}`)).toHaveTextContent(
      '文件已不可用',
    )
    expect(screen.getByTestId(`text-pane-${markdownFile.entityId}`)).not.toHaveTextContent(
      'notes.md decoded',
    )
    expect(requestPreview).toHaveBeenCalledTimes(2)
  })

  it('retries one preview encoding and opens Markdown links explicitly', async () => {
    localStorage.clear()
    const preview = markdownPreview(
      markdownFile,
      '<h1>Prompt</h1><a href="https://example.com/a">文档</a>',
      'gb18030',
    )
    const request = vi
      .fn<(file: BrowserFile, encoding?: TextEncoding) => Promise<TextPreviewDto>>()
      .mockRejectedValueOnce({
        code: 'text_encoding_required',
        userMessage: '请选择文本编码。',
      })
      .mockResolvedValueOnce(preview)
    const openExternalLink = vi.fn().mockResolvedValue(undefined)
    render(
      <TextPreview
        files={[markdownFile]}
        requestPreview={request}
        openExternalLink={openExternalLink}
        onClose={vi.fn()}
      />,
    )

    expect(await screen.findByRole('alert')).toHaveTextContent('请选择文本编码')
    fireEvent.change(screen.getByRole('combobox', { name: 'notes.md 文本编码' }), {
      target: { value: 'gb18030' },
    })
    await screen.findByRole('heading', { name: 'Prompt' })

    fireEvent.click(screen.getByRole('link', { name: '文档' }))
    expect(openExternalLink).toHaveBeenCalledWith('https://example.com/a')
    expect(request).toHaveBeenLastCalledWith(markdownFile, 'gb18030')
    expect(localStorage.length).toBe(0)
  })

  it('renders plain text as selectable text and reports truncation', async () => {
    const request = vi.fn().mockResolvedValue({
      ...textPreview(plainTextFile, 'product notes'),
      truncated: true,
    })
    render(
      <TextPreview
        files={[plainTextFile]}
        requestPreview={request}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(await screen.findByText('product notes')).toBeVisible()
    expect(screen.getByText('仅显示前 10 MiB')).toBeVisible()
  })
})
