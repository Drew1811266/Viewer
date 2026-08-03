import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import WorkspaceLoadingState from './WorkspaceLoadingState'

describe('WorkspaceLoadingState', () => {
  it('keeps final folder-band geometry visible while project content is loading', () => {
    render(<WorkspaceLoadingState bandCount={3} />)

    const status = screen.getByRole('status', { name: '项目内容加载中' })
    expect(status).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByRole('status', { name: '正在读取项目' })).toHaveClass('viewer-task-surface')
    expect(status.querySelectorAll('.workspace-loading-band')).toHaveLength(3)
    expect(status.querySelectorAll('.workspace-loading-thumbnail').length).toBeGreaterThan(3)
  })
})
