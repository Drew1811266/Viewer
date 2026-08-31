import { fireEvent, render, screen } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type {
  ReviewArchivePlan,
  ReviewArchiveSelection,
  ReviewTargetVersionKey,
} from '../../api/reviewWorkspaceTypes'
import ReviewArchiveDialog from './ReviewArchiveDialog'

const target = (
  feedbackId: string,
  textRevisionId: string,
  targetId: string,
  targetRevisionId: string,
): ReviewTargetVersionKey => ({ feedbackId, textRevisionId, targetId, targetRevisionId })

const imageOne = target('feedback-1', 'text-b', 'image-1-target', 'target-b')
const imageTwo = target('feedback-1', 'text-b', 'image-2-target', 'target-b')
const laterImageOne = target('feedback-1', 'text-c', 'image-1-target', 'target-c')

function fixture(): { preview: ReviewArchivePlan; selection: ReviewArchiveSelection } {
  const selection: ReviewArchiveSelection = {
    expectedSnapshotId: 'snapshot-c',
    groups: [{ basis: { kind: 'unknown' }, targets: [imageOne] }],
  }
  return {
    selection,
    preview: {
      expectedSnapshotId: 'snapshot-c',
      groups: [{ basis: { kind: 'unknown' }, targets: [imageOne, imageTwo] }],
      removed: [imageOne],
      retained: [
        {
          basis: imageOne,
          current: laterImageOne,
          disposition: 'retain_later_edit',
        },
      ],
      alreadyCovered: [],
    },
  }
}

it('shows the exact B/C archive difference without pulling the shared second image into history', () => {
  const { preview, selection } = fixture()
  render(
    <ReviewArchiveDialog
      preview={preview}
      selection={selection}
      busy={false}
      error={null}
      onSelectionChange={vi.fn()}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  )

  expect(screen.getByText('交接版本未确认')).toBeVisible()
  expect(screen.getByText('保留后补意见')).toBeVisible()
  expect(screen.getByRole('button', { name: '确认存档' })).toBeEnabled()
  expect(screen.getByRole('heading', { name: '将移入历史' }).parentElement).toHaveTextContent(
    'image-1-target',
  )
  expect(screen.getByRole('heading', { name: '未选素材' }).parentElement).toHaveTextContent(
    'image-2-target',
  )
  expect(screen.getByRole('heading', { name: '保留后补意见' }).parentElement).toHaveTextContent(
    'target-c',
  )
})

it('returns a fixed selection that preserves the original IDs and snapshot guard', () => {
  const { preview, selection } = fixture()
  const onSelectionChange = vi.fn()
  render(
    <ReviewArchiveDialog
      preview={preview}
      selection={selection}
      busy={false}
      error={null}
      onSelectionChange={onSelectionChange}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  )

  fireEvent.click(screen.getByRole('checkbox', { name: /image-2-target/ }))
  expect(onSelectionChange).toHaveBeenCalledWith({
    expectedSnapshotId: 'snapshot-c',
    groups: [{ basis: { kind: 'unknown' }, targets: [imageOne, imageTwo] }],
  })
})

it('does not allow an empty selection to create an archive', () => {
  const { preview, selection } = fixture()
  render(
    <ReviewArchiveDialog
      preview={preview}
      selection={{ ...selection, groups: [] }}
      busy={false}
      error={null}
      onSelectionChange={vi.fn()}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  )

  expect(screen.getByRole('button', { name: '确认存档' })).toBeDisabled()
})

it('treats a selection covered by an existing archive as a no-op', () => {
  const { preview, selection } = fixture()
  render(
    <ReviewArchiveDialog
      preview={{ ...preview, groups: [], removed: [], alreadyCovered: [imageOne] }}
      selection={selection}
      busy={false}
      error={null}
      onSelectionChange={vi.fn()}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  )

  expect(screen.getByText('没有新的可存档内容')).toBeVisible()
  expect(screen.getByRole('button', { name: '确认存档' })).toBeDisabled()
})

it('describes each retention disposition without calling removed or absent targets later edits', () => {
  const { preview, selection } = fixture()
  const removedCurrent = target('feedback-2', 'text-b', 'image-2-target', 'target-b')
  const absent = target('feedback-3', 'text-b', 'image-3-target', 'target-b')
  render(
    <ReviewArchiveDialog
      preview={{
        ...preview,
        retained: [
          ...preview.retained,
          { basis: removedCurrent, current: removedCurrent, disposition: 'remove_current' },
          { basis: absent, current: null, disposition: 'already_absent' },
        ],
      }}
      selection={selection}
      busy={false}
      error={null}
      onSelectionChange={vi.fn()}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  )

  expect(screen.getByRole('heading', { name: '保留后补意见' }).parentElement).toHaveTextContent(
    'image-1-target',
  )
  expect(screen.getByRole('heading', { name: '保留后补意见' }).parentElement).not.toHaveTextContent(
    'image-2-target',
  )
  expect(screen.getByRole('heading', { name: '将移入历史' }).parentElement).toHaveTextContent(
    'image-2-target',
  )
  expect(screen.getByRole('heading', { name: '当前已不存在' }).parentElement).toHaveTextContent(
    'image-3-target',
  )
})

it('marks a declared handoff as verified without claiming that any agent read or executed it', () => {
  const { preview, selection } = fixture()
  const declared = {
    expectedSnapshotId: selection.expectedSnapshotId,
    groups: [
      {
        basis: {
          kind: 'known' as const,
          snapshot: { snapshotId: 'snapshot-b', blake3: 'b'.repeat(64) },
          source: { kind: 'agent_declared' as const, usageId: 'usage-1' },
        },
        targets: [imageOne],
      },
    ],
  }
  render(
    <ReviewArchiveDialog
      preview={{ ...preview, groups: declared.groups }}
      selection={declared}
      busy={false}
      error={null}
      onSelectionChange={vi.fn()}
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  )

  expect(screen.getByText('已核对交接版本')).toBeVisible()
  expect(screen.getByText(/不.*表示意见已被执行或修复/)).toBeVisible()
  expect(screen.queryByText(/Agent.*已读|Agent.*已执行|已修复/)).toBeNull()
})

it('keeps a failed archive visible for retry and cancels only when not busy', () => {
  const { preview, selection } = fixture()
  const onCancel = vi.fn()
  const onConfirm = vi.fn()
  const rendered = render(
    <ReviewArchiveDialog
      preview={preview}
      selection={selection}
      busy={false}
      error="存档保存失败，请重试。"
      onSelectionChange={vi.fn()}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />,
  )

  expect(screen.getByRole('alert')).toHaveTextContent('存档保存失败，请重试。')
  fireEvent.click(screen.getByRole('button', { name: '确认存档' }))
  fireEvent.click(screen.getByRole('button', { name: '返回检查' }))
  expect(onConfirm).toHaveBeenCalledOnce()
  expect(onCancel).toHaveBeenCalledOnce()

  rendered.rerender(
    <ReviewArchiveDialog
      preview={preview}
      selection={selection}
      busy
      error="存档保存失败，请重试。"
      onSelectionChange={vi.fn()}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />,
  )
  expect(screen.getByRole('button', { name: '确认存档' })).toBeDisabled()
  expect(screen.getByRole('button', { name: '返回检查' })).toBeDisabled()
})
