import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { SelectionInfo } from '../api/types'
import MarkerControls from './MarkerControls'

const mixed: SelectionInfo = {
  relativePaths: ['id/1.jpg', 'id/2.jpg'],
  totalSize: 30,
  types: { folders: 0, images: 2, videos: 0, otherFiles: 0 },
  commonReview: { state: 'mixed' },
  commonFavorite: { state: 'common', value: false },
}

describe('MarkerControls', () => {
  it('applies marker buttons to every selected target independently', () => {
    const review = vi.fn()
    const favorite = vi.fn()
    render(
      <MarkerControls
        selectedCount={2}
        selectionInfo={mixed}
        readOnly={false}
        onSetReview={review}
        onToggleFavorite={favorite}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: '标记为保留' }))
    fireEvent.click(screen.getByRole('button', { name: '切换收藏' }))
    expect(review).toHaveBeenCalledWith('keep')
    expect(favorite).toHaveBeenCalledOnce()
    expect(screen.getByText('已选 2 项')).toBeVisible()
    expect(screen.getByText('审阅状态：混合')).toBeVisible()
    expect(screen.getByText('收藏：否')).toBeVisible()
  })

  it('disables marker buttons when read-only', () => {
    const review = vi.fn()
    const favorite = vi.fn()
    render(
      <MarkerControls
        selectedCount={1}
        selectionInfo={mixed}
        readOnly
        onSetReview={review}
        onToggleFavorite={favorite}
      />,
    )
    expect(screen.getByRole('button', { name: '标记为保留' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '切换收藏' })).toBeDisabled()
    expect(review).not.toHaveBeenCalled()
    expect(favorite).not.toHaveBeenCalled()
  })
})
