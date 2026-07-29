import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../api/types'
import { fileExtensionLabel } from '../fileKinds'
import UnsupportedFilePreview from './UnsupportedFilePreview'

const file = (name: string): BrowserFile => ({
  entityId: `other-${name}`,
  relativePath: name,
  name,
  kind: 'other',
  size: 1,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
})

describe('UnsupportedFilePreview', () => {
  it('shows one request-free generic unsupported-file dialog', () => {
    const onClose = vi.fn()
    render(<UnsupportedFilePreview file={file('license.other')} onClose={onClose} />)

    const dialog = screen.getByRole('dialog', { name: 'license.other' })
    expect(dialog).toHaveTextContent('暂不支持预览')
    expect(screen.getByText('.OTHER')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Quick Look|默认应用/ })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('reports unavailable files without inventing an external preview action', () => {
    render(<UnsupportedFilePreview file={file('license')} unavailable onClose={() => undefined} />)

    expect(screen.getByLabelText('license 无扩展名 文件已不可用')).toBeVisible()
    expect(screen.queryByRole('button', { name: /Quick Look|默认应用/ })).not.toBeInTheDocument()
  })
})

describe('fileExtensionLabel', () => {
  it.each([
    ['archive.tar.gz', '.GZ'],
    ['license', '无扩展名'],
    ['.gitignore', '无扩展名'],
    ['trailing.', '无扩展名'],
  ])('labels %s as %s', (name, expected) => {
    expect(fileExtensionLabel(name)).toBe(expected)
  })
})
