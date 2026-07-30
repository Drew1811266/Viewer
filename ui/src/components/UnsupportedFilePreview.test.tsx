import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile } from '../api/types'
import { fileExtensionLabel } from '../fileKinds'
import '../styles/app.css'
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
    render(<UnsupportedFilePreview file={file('archive.zip')} onClose={onClose} />)

    const dialog = screen.getByRole('dialog', { name: 'archive.zip' })
    const actions = within(dialog).getByRole('toolbar', { name: '文件预览控制' })
    expect(actions).toBeVisible()
    expect(actions).toHaveClass('preview-toolbar-actions')
    expect(getComputedStyle(actions).gridColumn).toBe('3')
    expect(getComputedStyle(actions).justifySelf).toBe('end')
    expect(within(dialog).getByText('暂不支持预览')).toHaveClass('unsupported-file-message')
    expect(screen.getByText('.ZIP')).toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: /外部|其它应用/ })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('reports unavailable files without inventing an external preview action', () => {
    render(<UnsupportedFilePreview file={file('license')} unavailable onClose={() => undefined} />)

    expect(screen.getByLabelText('license 无扩展名 文件已不可用')).toBeVisible()
    expect(screen.queryByRole('button', { name: /外部|其它应用/ })).not.toBeInTheDocument()
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
