import { fireEvent, render, screen } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { ReviewAssetVersion, ReviewTargetVersionKey } from '../../api/reviewWorkspaceTypes'
import ReviewSourceConfirmation from './ReviewSourceConfirmation'

const oldAsset: ReviewAssetVersion = {
  id: 'asset-old',
  sourceEntityId: 'old-entity',
  relativePath: 'images/look.png',
  evidence: { sizeBytes: 1, modifiedNs: '1', blake3: 'a'.repeat(64) },
  media: { kind: 'image', width: 100, height: 100 },
  producerAssetId: null,
  parentAssetVersionId: null,
}
const samePathNewImage: ReviewAssetVersion = {
  ...oldAsset,
  id: 'asset-new',
  evidence: { sizeBytes: 2, modifiedNs: '2', blake3: 'b'.repeat(64) },
}
const anotherCandidate: ReviewAssetVersion = {
  ...samePathNewImage,
  id: 'asset-other',
  relativePath: 'other.png',
}
const targetKey: ReviewTargetVersionKey = {
  feedbackId: 'feedback-1',
  textRevisionId: 'text-1',
  targetId: 'target-1',
  targetRevisionId: 'target-1',
}

it('requires an explicit candidate and position confirmation without claiming the opinion was executed', () => {
  const confirm = vi.fn()
  render(
    <ReviewSourceConfirmation
      oldAsset={oldAsset}
      candidates={[samePathNewImage, anotherCandidate]}
      originalAnchor={{ kind: 'asset' }}
      targetKey={targetKey}
      busy={false}
      error={null}
      onCancel={vi.fn()}
      onConfirm={confirm}
    />,
  )

  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeDisabled()
  expect(screen.getByText('这不会执行或修复意见。')).toBeVisible()
  fireEvent.click(screen.getByLabelText(/images\/look.png/))
  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeDisabled()
  fireEvent.click(screen.getByLabelText('我已确认该位置适用于所选素材'))
  fireEvent.click(screen.getByRole('button', { name: '确认素材与位置' }))
  expect(confirm).toHaveBeenCalledWith({
    targetKey,
    newAssetVersionId: 'asset-new',
    anchor: { kind: 'asset' },
    confirmation: { kind: 'user_confirmed' },
  })
})

it('requires position confirmation again after the user changes candidates', () => {
  render(
    <ReviewSourceConfirmation
      oldAsset={oldAsset}
      candidates={[samePathNewImage, anotherCandidate]}
      originalAnchor={{ kind: 'asset' }}
      targetKey={targetKey}
      busy={false}
      error={null}
      onCancel={vi.fn()}
      onConfirm={vi.fn()}
    />,
  )
  fireEvent.click(screen.getByLabelText(/images\/look.png/))
  fireEvent.click(screen.getByLabelText('我已确认该位置适用于所选素材'))
  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeEnabled()
  fireEvent.click(screen.getByLabelText(/other.png/))
  expect(screen.getByLabelText('我已确认该位置适用于所选素材')).not.toBeChecked()
  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeDisabled()
})

it('does not infer a binding from a same-path replacement or from no independent candidate', () => {
  const ambiguousSamePath = { ...samePathNewImage, id: 'asset-same-path-other' }
  const rendered = render(
    <ReviewSourceConfirmation
      oldAsset={oldAsset}
      candidates={[samePathNewImage, ambiguousSamePath]}
      originalAnchor={{ kind: 'asset' }}
      targetKey={targetKey}
      busy={false}
      error={null}
      onCancel={vi.fn()}
      onConfirm={vi.fn()}
    />,
  )
  expect(screen.getAllByLabelText(/images\/look.png/)).toHaveLength(2)
  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeDisabled()

  rendered.rerender(
    <ReviewSourceConfirmation
      oldAsset={oldAsset}
      candidates={[]}
      originalAnchor={{ kind: 'asset' }}
      targetKey={targetKey}
      busy={false}
      error={null}
      onCancel={vi.fn()}
      onConfirm={vi.fn()}
    />,
  )
  expect(screen.getByText(/没有可确认的候选素材/)).toBeVisible()
  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeDisabled()
})

it('clears an unavailable selected candidate and its position confirmation when candidate props change', () => {
  const rendered = render(
    <ReviewSourceConfirmation
      oldAsset={oldAsset}
      candidates={[samePathNewImage, anotherCandidate]}
      originalAnchor={{ kind: 'asset' }}
      targetKey={targetKey}
      busy={false}
      error={null}
      onCancel={vi.fn()}
      onConfirm={vi.fn()}
    />,
  )
  fireEvent.click(screen.getByLabelText(/images\/look.png/))
  fireEvent.click(screen.getByLabelText('我已确认该位置适用于所选素材'))
  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeEnabled()

  rendered.rerender(
    <ReviewSourceConfirmation
      oldAsset={oldAsset}
      candidates={[anotherCandidate]}
      originalAnchor={{ kind: 'asset' }}
      targetKey={{ ...targetKey, targetRevisionId: 'target-2' }}
      busy={false}
      error={null}
      onCancel={vi.fn()}
      onConfirm={vi.fn()}
    />,
  )

  expect(screen.getByLabelText('我已确认该位置适用于所选素材')).not.toBeChecked()
  expect(screen.getByRole('button', { name: '确认素材与位置' })).toBeDisabled()
})
