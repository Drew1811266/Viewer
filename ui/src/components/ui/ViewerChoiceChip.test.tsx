import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import ViewerChoiceChip from './ViewerChoiceChip'

describe('ViewerChoiceChip', () => {
  it('keeps checkbox as the default input type', () => {
    render(<ViewerChoiceChip>默认</ViewerChoiceChip>)

    expect(screen.getByRole('checkbox', { name: '默认' })).toBeVisible()
  })

  it('provides real radio semantics for bounded exclusive choices', () => {
    const onCheckedChange = vi.fn()
    render(
      <>
        <ViewerChoiceChip type="radio" name="shape" checked>
          圆形
        </ViewerChoiceChip>
        <ViewerChoiceChip
          type="radio"
          name="shape"
          checked={false}
          onCheckedChange={onCheckedChange}
        >
          圆角矩形
        </ViewerChoiceChip>
      </>,
    )

    const radios = screen.getAllByRole('radio')
    expect(radios).toHaveLength(2)
    expect(screen.getByText('圆形').closest('label')).toHaveAttribute('data-checked', 'true')

    fireEvent.click(screen.getByRole('radio', { name: '圆角矩形' }))

    expect(onCheckedChange).toHaveBeenCalledWith(true)
  })
})
