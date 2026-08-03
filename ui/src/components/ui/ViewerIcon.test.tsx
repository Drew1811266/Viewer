import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import ViewerIcon, { VIEWER_ICON_NAMES } from './ViewerIcon'

describe('ViewerIcon', () => {
  it('renders the approved static asset without exposing duplicate speech', () => {
    render(<ViewerIcon name="search" size={16} data-testid="icon" />)
    const icon = screen.getByTestId('icon')
    expect(icon).toHaveAttribute('aria-hidden', 'true')
    expect(icon).toHaveAttribute('src', expect.stringContaining('search'))
    expect(icon).toHaveAttribute('width', '16')
    expect(icon).toHaveAttribute('height', '16')
  })

  it('publishes every icon name required by the migration', () => {
    expect(VIEWER_ICON_NAMES).toEqual([
      'alert-triangle',
      'arrow-down',
      'arrow-up',
      'check',
      'chevron-down',
      'chevron-left',
      'chevron-right',
      'chevron-up',
      'circle',
      'circle-dot',
      'columns-2',
      'copy',
      'ellipsis',
      'eye',
      'folder-input',
      'folder-output',
      'grip-vertical',
      'info',
      'layout-grid',
      'lock',
      'minus',
      'move',
      'panel-right',
      'pencil',
      'plus',
      'refresh-cw',
      'rotate-cw',
      'search',
      'settings',
      'sliders-horizontal',
      'star',
      'trash-2',
      'x',
    ])
  })
})
