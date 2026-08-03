import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useToolbarPopover } from './useToolbarPopover'

function Harness() {
  const toolbar = useToolbarPopover()
  return (
    <>
      <button type="button" onClick={() => toolbar.togglePopover('filter')}>
        筛选
      </button>
      <button type="button" onClick={() => toolbar.togglePopover('view')}>
        视图
      </button>
      <button type="button" onClick={() => toolbar.setPopoverOpen('more', true)}>
        更多
      </button>
      <button type="button" onClick={toolbar.closePopover}>
        关闭
      </button>
      <output>{toolbar.openPopover ?? 'none'}</output>
    </>
  )
}

describe('useToolbarPopover', () => {
  it('keeps exactly one toolbar popover open', () => {
    render(<Harness />)
    expect(screen.getByText('none')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '筛选' }))
    expect(screen.getByText('filter')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '视图' }))
    expect(screen.getByText('view')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '更多' }))
    expect(screen.getByText('more')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))
    expect(screen.getByText('none')).toBeVisible()
  })
})
