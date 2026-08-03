import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import TrashConfirmation from './TrashConfirmation'

describe('TrashConfirmation', () => {
  it('requires visible confirmation and explains system restoration', () => {
    const confirm = vi.fn()
    const cancel = vi.fn()
    render(<TrashConfirmation count={3} busy={false} onConfirm={confirm} onCancel={cancel} />)
    expect(screen.getByText(/macOS 废纸篓/)).toBeVisible()
    expect(screen.queryByText(/永久删除/)).not.toBeInTheDocument()
    const destructive = screen.getByRole('button', { name: '移到废纸篓' })
    expect(destructive.closest('.viewer-dialog__footer')).not.toBeNull()
    expect(destructive).toHaveAttribute('data-tone', 'danger')
    expect(screen.getByRole('button', { name: '取消' }).compareDocumentPosition(destructive)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    )
    fireEvent.click(destructive)
    expect(confirm).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '取消' }))
    expect(cancel).toHaveBeenCalledOnce()
  })
})
