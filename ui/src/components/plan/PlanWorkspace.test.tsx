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
      title: '秋季形象片',
      revision: 4,
      blake3: 'hash',
      groups: [{ id: 'g1', label: '夏', declaredCount: 3, actualCount: 2, brief: '轻薄夏装' }],
      shots: [
        {
          shotId: 's1',
          groupId: 'g1',
          prompt: '轻薄夏装\n海边',
          hash: 'h1',
          references: ['a'],
          segments: [
            { kind: 'group', label: '组', path: 'groups.brief', text: '轻薄夏装' },
            { kind: 'shot', label: '分镜', path: 'shots.delta', text: '海边' },
          ],
        },
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
    await waitFor(() => expect(screen.getByText('1 组 · 1 张')).toBeInTheDocument())
    expect(screen.getByText('待处理 0 · 已改 1 · 已换 0 · 已失效 0')).toBeInTheDocument()
    expect(screen.getAllByText('autumn').length).toBeGreaterThan(0)
    expect(screen.getByText('秋季形象片')).toBeInTheDocument()
  })

  it('filters the canvas by generation state', async () => {
    const fake = bridge()
    render(<PlanWorkspace bridge={fake} />)
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /计划已变/ }))
    await waitFor(() => expect(screen.queryByText('s1')).not.toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /全部/ }))
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())
  })

  it('opens the drawer for the selected shot', async () => {
    render(<PlanWorkspace bridge={bridge()} />)
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('s1'))
    await waitFor(() => expect(screen.getByText('参考图 · 1 张')).toBeInTheDocument())
    const drawer = screen.getByTestId('plan-drawer')
    expect(drawer).toHaveTextContent('轻薄夏装')
    expect(drawer).toHaveTextContent('海边')
  })

  it('writes a mark for the selected shot', async () => {
    const fake = bridge()
    render(<PlanWorkspace bridge={fake} />)
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('s1'))
    fireEvent.click(await screen.findByText('标记这个分镜'))
    await waitFor(() => expect(fake.writeMarks).toHaveBeenCalledOnce())
    const calls = (fake.writeMarks as unknown as { mock: { calls: unknown[][] } }).mock.calls
    const payload = calls[0]?.[0] as
      | {
          protocolVersion: string
          planId: string
          marks: { target: { kind: string; shotId: string }; revision: number }[]
        }
      | undefined
    expect(payload?.protocolVersion).toBe('viewer.plan-marks/1')
    expect(payload?.marks).toHaveLength(2)
    const added = payload?.marks.at(-1)
    expect(added?.target).toEqual({ kind: 'shot', shotId: 's1' })
    expect(added?.revision).toBe(4)
  })

  it('removes a mark by id', async () => {
    const fake = bridge()
    render(<PlanWorkspace bridge={fake} />)
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('s1'))
    fireEvent.click(await screen.findByLabelText('删除标记 mk_1'))
    await waitFor(() => expect(fake.writeMarks).toHaveBeenCalledOnce())
    const calls = (fake.writeMarks as unknown as { mock: { calls: unknown[][] } }).mock.calls
    const payload = calls[0]?.[0] as { marks: unknown[] } | undefined
    expect(payload?.marks).toHaveLength(0)
  })

  it('re-reads the plan while the surface stays open', async () => {
    vi.useFakeTimers()
    try {
      const fake = bridge()
      render(<PlanWorkspace bridge={fake} />)
      await vi.advanceTimersByTimeAsync(0)
      const initial = (fake.list as unknown as { mock: { calls: unknown[] } }).mock.calls.length
      await vi.advanceTimersByTimeAsync(2100)
      await vi.advanceTimersByTimeAsync(0)
      await Promise.resolve()
      const after = (fake.list as unknown as { mock: { calls: unknown[] } }).mock.calls.length
      expect(after).toBeGreaterThan(initial)
    } finally {
      vi.useRealTimers()
    }
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
