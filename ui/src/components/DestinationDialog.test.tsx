import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { FileCommandPreflight } from '../api/types'
import '../styles/app.css'
import DestinationDialog from './DestinationDialog'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}

const folders = [
  {
    entityId: 'folder-a',
    parentEntityId: null,
    relativePath: 'A',
    name: 'A',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: 'folder-b',
    parentEntityId: null,
    relativePath: 'B',
    name: 'B',
    marker: { reviewState: null, favorite: false },
  },
]

describe('DestinationDialog', () => {
  it('stacks destination and conflict regions at the supported compact width', () => {
    vi.stubGlobal('innerWidth', 500)
    render(
      <DestinationDialog
        mode="copy"
        entityIds={['one']}
        folders={folders}
        busy={false}
        requestPreflight={vi.fn()}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )

    const layout = document.querySelector<HTMLElement>('.destination-dialog-layout')
    expect(layout).not.toBeNull()
    expect(getComputedStyle(layout as HTMLElement).gridTemplateColumns).toBe('1fr')
  })

  it('preflights an in-project destination and collects per-item/apply-rest choices', async () => {
    const result: FileCommandPreflight = {
      executable: true,
      rows: [
        { entityId: 'one', relativePath: 'one.jpg', state: 'conflict' },
        { entityId: 'two', relativePath: 'two.jpg', state: 'conflict' },
      ],
    }
    const preflight = vi.fn().mockResolvedValue(result)
    const execute = vi.fn()
    render(
      <DestinationDialog
        mode="copy"
        entityIds={['one', 'two']}
        folders={folders}
        busy={false}
        requestPreflight={preflight}
        onConfirm={execute}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('radio', { name: /B/ }))
    expect(document.querySelector('.destination-dialog-layout')).not.toBeNull()
    expect(document.querySelector('.destination-dialog-main')).not.toBeNull()
    fireEvent.click(screen.getByRole('button', { name: '检查冲突' }))
    await waitFor(() => expect(preflight).toHaveBeenCalledOnce())
    const firstPolicy = screen.getByRole('combobox', { name: '冲突处理 one.jpg' })
    expect(screen.getByRole('combobox', { name: '冲突处理 two.jpg' })).toBeVisible()
    fireEvent.change(firstPolicy, {
      target: { value: 'keep_both' },
    })
    fireEvent.click(screen.getByRole('checkbox', { name: '应用到剩余冲突 one.jpg' }))
    const executeButton = screen.getByRole('button', { name: '开始复制' })
    expect(executeButton.closest('.viewer-dialog__footer')).not.toBeNull()
    expect(executeButton).toHaveAttribute('data-tone', 'primary')
    expect(screen.getAllByText('需要处理')).toHaveLength(2)
    for (const status of screen.getAllByText('需要处理')) {
      expect(status).toHaveClass('viewer-status-tag')
    }
    fireEvent.click(executeButton)
    expect(execute).toHaveBeenCalledWith(
      expect.arrayContaining([
        expect.objectContaining({
          entityId: 'one',
          action: { kind: 'copy', destinationFolderId: 'folder-b' },
        }),
      ]),
      [{ entityId: 'one', policy: 'keep_both', applyToRemaining: true }],
    )
  })

  it('shows exact blocked rows and never enables execution', async () => {
    render(
      <DestinationDialog
        mode="move"
        entityIds={['one']}
        folders={folders}
        busy={false}
        requestPreflight={vi.fn().mockResolvedValue({
          executable: false,
          rows: [
            {
              entityId: 'one',
              relativePath: 'one.jpg',
              state: 'blocked',
              code: 'invalid_target',
            },
          ],
        })}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('radio', { name: /A/ }))
    fireEvent.click(screen.getByRole('button', { name: '检查冲突' }))
    expect(await screen.findByText('invalid_target')).toBeVisible()
    expect(screen.getByRole('button', { name: '开始移动' })).toBeDisabled()
  })

  it('discards a preflight response when the destination changes in flight', async () => {
    const pending = deferred<FileCommandPreflight | null>()
    render(
      <DestinationDialog
        mode="copy"
        entityIds={['one']}
        folders={folders}
        busy={false}
        requestPreflight={() => pending.promise}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('radio', { name: /A/ }))
    fireEvent.click(screen.getByRole('button', { name: '检查冲突' }))
    fireEvent.click(screen.getByRole('radio', { name: /B/ }))
    pending.resolve({
      executable: true,
      rows: [{ entityId: 'one', relativePath: 'one.jpg', state: 'ready' }],
    })
    await waitFor(() => expect(screen.getByRole('button', { name: '检查冲突' })).toBeEnabled())
    expect(screen.queryByText('可执行')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '开始复制' })).toBeDisabled()
  })
})
