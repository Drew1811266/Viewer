import './jsdomPolyfills'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { PlanBridge } from '../../api/plan'
import { PlanWorkspace } from './PlanWorkspace'
import { stalePanelCopy } from './usePlanWorkspace'

function bridge(): PlanBridge {
  return {
    list: vi.fn().mockResolvedValue([{ planId: 'autumn', revision: 4, ignored: 0 }]),
    snapshot: vi.fn().mockResolvedValue({
      planId: 'autumn',
      revision: 4,
      blake3: 'hash',
      groups: [{ id: 'g1', label: '夏', declaredCount: 3, actualCount: 2, brief: '轻薄夏装' }],
      shots: [
        { shotId: 's1', groupId: 'g1', prompt: '轻薄夏装\n海边', hash: 'h1', references: ['a'] },
      ],
      warnings: [],
      diff: null,
    }),
    readMarks: vi.fn().mockResolvedValue({
      planId: 'autumn',
      planAvailable: true,
      planRevision: 4,
      marks: [
        {
          mark: {
            id: 'mk_1',
            revision: 3,
            target: { kind: 'shot', shotId: 's1' },
            category: 'wrong',
            createdAt: '2026-11-03T00:00:00Z',
          },
          status: 'changed',
        },
      ],
    }),
    writeMarks: vi.fn().mockResolvedValue(undefined),
  }
}

describe('PlanWorkspace', () => {
  it('loads the plan and shows revision, counts and mark states', async () => {
    render(<PlanWorkspace bridge={bridge()} />)
    await waitFor(() => expect(screen.getAllByText('r4 · 1 组 / 1 张').length).toBeGreaterThan(0))
    expect(screen.getByText('待处理 0 · 已改 1 · 已换 0 · 已失效 0')).toBeInTheDocument()
  })

  it('opens the drawer for the selected shot', async () => {
    render(<PlanWorkspace bridge={bridge()} />)
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('s1'))
    await waitFor(() => expect(screen.getByText('参考图 1 张')).toBeInTheDocument())
  })

  it('explains that the project has no plan', async () => {
    const empty = { ...bridge(), list: vi.fn().mockResolvedValue([]) }
    render(<PlanWorkspace bridge={empty} />)
    await waitFor(() => expect(screen.getByText('这个项目还没有生成计划')).toBeInTheDocument())
  })

  it('summarises mark states without inventing an approval', () => {
    expect(stalePanelCopy([])).toBe('还没有标记')
  })
})
