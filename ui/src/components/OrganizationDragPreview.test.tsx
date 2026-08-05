import { render, screen } from '@testing-library/react'
import { expect, it } from 'vitest'
import OrganizationDragPreview from './OrganizationDragPreview'

it('renders the formal organization move preview at its pointer coordinates', () => {
  render(<OrganizationDragPreview clientX={420} clientY={280} itemCount={3} mode="move" />)

  expect(screen.getByRole('status', { name: '正在整理文件' })).toHaveTextContent('移动 3 项')
  expect(screen.getByRole('status', { name: '正在整理文件' })).toHaveStyle({
    '--organization-drag-x': '420px',
    '--organization-drag-y': '280px',
  })
})
