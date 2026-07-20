import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import FileActionToolbar from './FileActionToolbar'

describe('FileActionToolbar', () => {
  it('exposes selection-aware actions and never treats unpreviewable files as inoperable', () => {
    const rename = vi.fn()
    const info = vi.fn()
    const { rerender } = render(
      <FileActionToolbar
        selectedCount={0}
        selectedImageCount={0}
        readOnly={false}
        busy={false}
        onRename={rename}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onTrash={vi.fn()}
        onCompare={vi.fn()}
        onInfo={info}
      />,
    )
    expect(screen.getByRole('button', { name: '重命名' })).toBeDisabled()

    rerender(
      <FileActionToolbar
        selectedCount={1}
        selectedImageCount={0}
        readOnly={false}
        busy={false}
        onRename={rename}
        onCopy={vi.fn()}
        onMove={vi.fn()}
        onTrash={vi.fn()}
        onCompare={vi.fn()}
        onInfo={info}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '信息' }))
    expect(info).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '重命名' }))
    expect(rename).toHaveBeenCalledOnce()
    expect(screen.getByRole('button', { name: '移到废纸篓' })).toBeEnabled()
  })

  it('labels batch rename, gates compare to 2–4 images and suppresses all writes when blocked', () => {
    const props = {
      selectedCount: 3,
      selectedImageCount: 3,
      readOnly: false,
      busy: false,
      onRename: vi.fn(),
      onCopy: vi.fn(),
      onMove: vi.fn(),
      onTrash: vi.fn(),
      onCompare: vi.fn(),
      onInfo: vi.fn(),
    }
    const { rerender } = render(<FileActionToolbar {...props} />)
    expect(screen.getByRole('button', { name: '批量重命名' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '并排对比' })).toBeEnabled()

    rerender(<FileActionToolbar {...props} readOnly />)
    expect(screen.getByRole('button', { name: '批量重命名' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '复制到…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '并排对比' })).toBeEnabled()

    rerender(<FileActionToolbar {...props} busy />)
    expect(screen.getByRole('button', { name: '批量重命名' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '并排对比' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '信息' })).toBeEnabled()

    rerender(<FileActionToolbar {...props} selectedCount={4} selectedImageCount={3} />)
    expect(screen.getByRole('button', { name: '并排对比' })).toBeDisabled()
  })
})
