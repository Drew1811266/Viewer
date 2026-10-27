import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BatchImageExecution, BatchImagePlanRow } from '../api/types'
import BatchImageDialog, { type BatchImageDialogProps } from './BatchImageDialog'

const PLAN: BatchImagePlanRow[] = [
  {
    entityId: 'entity-1',
    relativePath: 'photo.jpg',
    disposition: { kind: 'convert', destination: 'photo-converted.webp' },
  },
]

const EXECUTION: BatchImageExecution = { batchId: 'batch-1', outcomes: [] }

type Preview = BatchImageDialogProps['previewBatchImage']
type Execute = BatchImageDialogProps['executeBatchImage']

function previewMock() {
  return vi.fn().mockResolvedValue(PLAN) as unknown as Preview & {
    mock: { calls: unknown[][] }
  }
}

function executeMock() {
  return vi.fn().mockResolvedValue(EXECUTION) as unknown as Execute & {
    mock: { calls: unknown[][] }
  }
}

function renderDialog(props: { previewBatchImage: Preview; executeBatchImage: Execute }) {
  render(
    <BatchImageDialog
      entityIds={['entity-1']}
      busy={false}
      previewBatchImage={props.previewBatchImage}
      executeBatchImage={props.executeBatchImage}
      onCancel={vi.fn()}
    />,
  )
}

function lastSettings(preview: Preview & { mock: { calls: unknown[][] } }) {
  return preview.mock.calls.at(-1)?.[1] as { resize: unknown; qualityMilli: number }
}

describe('BatchImageDialog', () => {
  it('sends the selected resize mode and dimensions in the preview request', async () => {
    const preview = previewMock()
    renderDialog({ previewBatchImage: preview, executeBatchImage: executeMock() })
    await waitFor(() => expect(preview).toHaveBeenCalled())
    expect(lastSettings(preview).resize).toEqual({ kind: 'none' })

    fireEvent.change(screen.getByLabelText('尺寸修改方式'), { target: { value: 'longest_edge' } })
    await waitFor(() =>
      expect(lastSettings(preview).resize).toEqual({ kind: 'longest_edge', edge: 2048 }),
    )

    fireEvent.change(screen.getByLabelText('尺寸修改方式'), { target: { value: 'percent' } })
    fireEvent.change(screen.getByLabelText('缩放百分比'), { target: { value: '25' } })
    await waitFor(() =>
      expect(lastSettings(preview).resize).toEqual({ kind: 'percent', percent: 25 }),
    )

    fireEvent.change(screen.getByLabelText('尺寸修改方式'), { target: { value: 'exact' } })
    fireEvent.change(screen.getByLabelText('输出宽度'), { target: { value: '800' } })
    fireEvent.change(screen.getByLabelText('输出高度'), { target: { value: '600' } })
    await waitFor(() =>
      expect(lastSettings(preview).resize).toEqual({ kind: 'exact', width: 800, height: 600 }),
    )
  })

  it('allows quality zero and submits the plan with those settings', async () => {
    const preview = previewMock()
    const execute = executeMock()
    renderDialog({ previewBatchImage: preview, executeBatchImage: execute })
    const quality = screen.getByLabelText('压缩质量') as HTMLInputElement
    expect(quality.min).toBe('0')
    fireEvent.change(quality, { target: { value: '0' } })
    await waitFor(() => expect(lastSettings(preview).qualityMilli).toBe(0))

    fireEvent.click(await screen.findByRole('button', { name: '转换 1 个文件' }))
    await waitFor(() =>
      expect(execute).toHaveBeenCalledWith(
        ['entity-1'],
        expect.objectContaining({ qualityMilli: 0 }),
      ),
    )
  })
})
