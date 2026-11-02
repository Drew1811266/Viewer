import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import AnnotationStyleBar from './AnnotationStyleBar'

const setAnnotationStyle = vi.fn()

const controller = {
  protocol: 'continuous',
  readOnlyReason: null,
  styleBusy: false,
  tool: 'rectangle',
  editor: { phase: { status: 'idle' } },
  annotationStyle: {
    tool: 'rectangle',
    color: 'red',
    width: 'medium',
    opacityPercent: 100,
    dashed: false,
  },
  canUndo: false,
  canRedo: false,
  setAnnotationStyle,
} as unknown as ImageReviewWorkbenchController

describe('AnnotationStyleBar native input', () => {
  it('excludes the native canvas from clicks on color and width controls', () => {
    render(<AnnotationStyleBar controller={controller} />)

    expect(screen.getByTestId('annotation-style-bar')).toHaveAttribute(
      'data-native-input-exclusion',
      'true',
    )
  })

  it('applies a color and then a width without losing the chosen color', () => {
    render(<AnnotationStyleBar controller={controller} />)

    fireEvent.click(screen.getByRole('button', { name: '颜色蓝' }))
    expect(setAnnotationStyle).toHaveBeenLastCalledWith(
      expect.objectContaining({ color: 'blue', width: 'medium' }),
    )

    fireEvent.click(screen.getByRole('button', { name: '线条粗' }))
    expect(setAnnotationStyle).toHaveBeenLastCalledWith(
      expect.objectContaining({ color: 'blue', width: 'thick' }),
    )
  })
})
