import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { RenamePreview } from '../api/types'
import BatchRenameDialog from './BatchRenameDialog'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}

describe('BatchRenameDialog', () => {
  it('requests rules in fixed order and blocks execution when any preview row fails', async () => {
    const preview: RenamePreview = {
      executable: false,
      rows: Array.from({ length: 100 }, (_, index) => ({
        entityId: `image-${index}`,
        sourceRelativePath: `id/${index}.jpg`,
        destinationRelativePath: index === 0 ? null : `id/hero-${index}.jpg`,
        proposedName: `hero-${index}.jpg`,
        errors: index === 0 ? ['destination_occupied'] : [],
      })),
    }
    const requestPreview = vi.fn().mockResolvedValue(preview)
    render(
      <BatchRenameDialog
        entityIds={preview.rows.map((row) => row.entityId)}
        busy={false}
        requestPreview={requestPreview}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.change(screen.getByRole('textbox', { name: '查找' }), {
      target: { value: 'front' },
    })
    fireEvent.change(screen.getByRole('textbox', { name: '替换为' }), {
      target: { value: 'hero' },
    })
    fireEvent.click(screen.getByRole('button', { name: '更新预览' }))
    await waitFor(() => expect(requestPreview).toHaveBeenCalledOnce())
    expect(document.querySelector('.batch-rename-rule-region')).not.toBeNull()
    expect(document.querySelector('.rename-preview-summary')).toHaveTextContent('100 项')
    expect(screen.getByText('完整预览：100 项')).toBeVisible()
    expect(screen.getByText('destination_occupied')).toBeVisible()
    expect(screen.getByRole('button', { name: '执行批量重命名' })).toBeDisabled()
  })

  it('submits the exact executable preview and rules', async () => {
    const onConfirm = vi.fn()
    const preview: RenamePreview = {
      executable: true,
      rows: [
        {
          entityId: 'image-1',
          sourceRelativePath: 'id/front.jpg',
          destinationRelativePath: 'id/hero-01.jpg',
          proposedName: 'hero-01.jpg',
          errors: [],
        },
      ],
    }
    render(
      <BatchRenameDialog
        entityIds={['image-1']}
        busy={false}
        requestPreview={vi.fn().mockResolvedValue(preview)}
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('checkbox', { name: '添加序号' }))
    fireEvent.click(screen.getByRole('button', { name: '更新预览' }))
    await screen.findByText('完整预览：1 项')
    const execute = screen.getByRole('button', { name: '执行批量重命名' })
    expect(execute.closest('.viewer-dialog__footer')).not.toBeNull()
    expect(execute).toHaveAttribute('data-tone', 'primary')
    expect(screen.getByRole('button', { name: '取消' }).compareDocumentPosition(execute)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    )
    fireEvent.click(execute)
    expect(onConfirm).toHaveBeenCalledWith(
      expect.objectContaining({ sequence: { start: 1, digits: 2 } }),
      preview,
    )
  })

  it('discards a preview when rules change before the response arrives', async () => {
    const pending = deferred<RenamePreview | null>()
    render(
      <BatchRenameDialog
        entityIds={['image-1']}
        busy={false}
        requestPreview={() => pending.promise}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '更新预览' }))
    fireEvent.change(screen.getByRole('textbox', { name: '前缀' }), {
      target: { value: 'new-' },
    })
    pending.resolve({
      executable: true,
      rows: [
        {
          entityId: 'image-1',
          sourceRelativePath: 'front.jpg',
          destinationRelativePath: 'old-front.jpg',
          proposedName: 'old-front.jpg',
          errors: [],
        },
      ],
    })
    await waitFor(() => expect(screen.getByRole('button', { name: '更新预览' })).toBeEnabled())
    expect(screen.queryByText('完整预览：1 项')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '执行批量重命名' })).toBeDisabled()
  })

  it('reveals and focuses the first invalid row even when it starts off screen', async () => {
    const preview: RenamePreview = {
      executable: false,
      rows: Array.from({ length: 100 }, (_, index) => ({
        entityId: `image-${index}`,
        sourceRelativePath: `id/${index}.jpg`,
        destinationRelativePath: index === 80 ? null : `id/hero-${index}.jpg`,
        proposedName: `hero-${index}.jpg`,
        errors: index === 80 ? ['destination_occupied'] : [],
      })),
    }
    render(
      <BatchRenameDialog
        entityIds={preview.rows.map((row) => row.entityId)}
        busy={false}
        requestPreview={vi.fn().mockResolvedValue(preview)}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '更新预览' }))
    const error = await screen.findByText('destination_occupied')
    await waitFor(() => expect(error.closest('[data-invalid="true"]')).toHaveFocus())
  })
})
