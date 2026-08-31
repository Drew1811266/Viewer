import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ReviewUsageImportPreview } from '../../api/reviewWorkspaceTypes'
import ReviewUsageImport from './ReviewUsageImport'

function preview(): ReviewUsageImportPreview {
  return {
    declaration: {
      id: 'usage-1',
      projectId: 'project-1',
      streamId: 'stream-1',
      basis: { snapshotId: 'snapshot-b', blake3: '12'.repeat(32) },
      targets: [
        {
          feedbackId: 'feedback-1',
          textRevisionId: 'text-1',
          targetId: 'target-1',
          targetRevisionId: 'target-revision-1',
        },
      ],
      outputs: [],
    },
    canonicalDigest: '34'.repeat(32),
    source: 'handoff/review-usage.json',
    sourceDigest: '56'.repeat(32),
    outputs: [],
  }
}

describe('ReviewUsageImport', () => {
  it('shows the desktop-validated project-relative source and adopts its exact declaration', async () => {
    const onSelect = vi.fn().mockResolvedValue(preview())
    const onConfirm = vi.fn()
    render(<ReviewUsageImport onSelect={onSelect} onConfirm={onConfirm} onCancel={vi.fn()} />)

    fireEvent.click(screen.getByRole('button', { name: '选择声明文件' }))
    await waitFor(() => expect(screen.getByText('handoff/review-usage.json')).toBeVisible())
    expect(screen.getByText('依据快照 snapshot-b')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '采用声明' }))
    expect(onConfirm).toHaveBeenCalledWith(preview())
  })

  it('maps a wrong-project rejection and still lets the user continue without a declaration', async () => {
    const onSelect = vi.fn().mockRejectedValue({
      code: 'wrong_context',
      message: 'usage declaration belongs to another context',
      retryable: false,
      committedReceipt: null,
    })
    const onCancel = vi.fn()
    render(<ReviewUsageImport onSelect={onSelect} onConfirm={vi.fn()} onCancel={onCancel} />)

    fireEvent.click(screen.getByRole('button', { name: '选择声明文件' }))
    expect(await screen.findByText('声明不属于当前项目')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '不使用声明' }))
    expect(onCancel).toHaveBeenCalledOnce()
  })

  it('treats native picker cancellation as no selection without an error', async () => {
    render(
      <ReviewUsageImport
        onSelect={vi.fn().mockResolvedValue(null)}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '选择声明文件' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '选择声明文件' })).toBeEnabled())
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '采用声明' })).toBeDisabled()
  })

  it('keeps the inspected declaration visible when adoption is rejected by a read-only project', async () => {
    render(
      <ReviewUsageImport
        onSelect={vi.fn().mockResolvedValue(preview())}
        onConfirm={vi.fn().mockRejectedValue({
          code: 'read_only',
          message: 'project is read only',
          retryable: false,
          committedReceipt: null,
        })}
        onCancel={vi.fn()}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: '选择声明文件' }))
    await screen.findByText('handoff/review-usage.json')
    fireEvent.click(screen.getByRole('button', { name: '采用声明' }))

    expect(await screen.findByText('当前项目只读，声明未采用')).toBeVisible()
    expect(screen.getByRole('dialog', { name: '导入返工依据声明' })).toBeVisible()
  })
})
