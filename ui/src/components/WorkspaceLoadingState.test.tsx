import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import WorkspaceLoadingState from './WorkspaceLoadingState'

describe('WorkspaceLoadingState', () => {
  it('matches the approved launch scan grid without duplicating the global task surface', () => {
    render(<WorkspaceLoadingState />)

    const status = screen.getByRole('status', { name: '项目内容加载中' })
    expect(status).toHaveAttribute('aria-busy', 'true')
    expect(status.querySelectorAll('.workspace-loading-card')).toHaveLength(8)
    expect(status.querySelector('.workspace-loading-band')).not.toBeInTheDocument()
    expect(screen.queryByRole('status', { name: '正在读取项目' })).not.toBeInTheDocument()
  })
})
