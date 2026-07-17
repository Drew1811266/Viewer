import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile, TextEncoding, TextPreview as TextPreviewDto } from '../api/types'
import TextPreview from './TextPreview'

const file: BrowserFile = {
  entityId: 'text-1',
  relativePath: 'id-1/prompt.md',
  name: 'prompt.md',
  kind: 'markdown',
  size: 100,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
}

describe('TextPreview', () => {
  it('retries only the current preview encoding and opens links explicitly', async () => {
    localStorage.clear()
    const preview: TextPreviewDto = {
      entityId: file.entityId,
      format: 'markdown',
      plainText: null,
      markdownHtml: '<h1>Prompt</h1><a href="https://example.com/a">文档</a>',
      encoding: 'gb18030',
      truncated: false,
    }
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
        file={file}
        requestPreview={request}
        openExternalLink={openExternalLink}
        onClose={vi.fn()}
      />,
    )

    expect(await screen.findByRole('alert')).toHaveTextContent('请选择文本编码')
    fireEvent.change(screen.getByRole('combobox', { name: '文本编码' }), {
      target: { value: 'gb18030' },
    })
    await screen.findByRole('heading', { name: 'Prompt' })

    fireEvent.click(screen.getByRole('link', { name: '文档' }))
    expect(openExternalLink).toHaveBeenCalledWith('https://example.com/a')
    expect(request).toHaveBeenLastCalledWith(file, 'gb18030')
    expect(localStorage.length).toBe(0)
  })

  it('renders plain text as selectable text and reports truncation', async () => {
    const request = vi.fn().mockResolvedValue({
      entityId: file.entityId,
      format: 'plain_text',
      plainText: 'product notes',
      markdownHtml: null,
      encoding: 'utf8',
      truncated: true,
    })
    render(
      <TextPreview
        file={{ ...file, name: 'notes.txt', kind: 'text' }}
        requestPreview={request}
        openExternalLink={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(await screen.findByText('product notes')).toBeVisible()
    expect(screen.getByText('仅显示前 10 MiB')).toBeVisible()
  })
})
