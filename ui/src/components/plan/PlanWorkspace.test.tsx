import './jsdomPolyfills'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { PlanBridge } from '../../api/plan'
import { PlanWorkspace } from './PlanWorkspace'
import type { PlanSnapshot } from './planGraph'
import { stalePanelCopy } from './usePlanWorkspace'

const SNAPSHOT: PlanSnapshot = {
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
}

function bridge(snapshot: Partial<PlanSnapshot> = {}): PlanBridge {
  return {
    list: vi.fn().mockResolvedValue([{ planId: 'autumn', revision: 4, ignored: 0 }]),
    snapshot: vi.fn().mockResolvedValue({ ...SNAPSHOT, ...snapshot }),
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

  it('summarises the revision diff and the affected shots', async () => {
    render(
      <PlanWorkspace
        bridge={bridge({
          diff: {
            requirement: [
              {
                layer: 'requirement',
                kind: 'modified',
                path: 'dimensions.series.values.summer',
                message: '共享取值 series/summer 变了',
              },
            ],
            material: [],
            affectedShots: ['s1'],
          },
        })}
      />,
    )
    await waitFor(() => expect(screen.getByTestId('plan-diff')).toBeInTheDocument())
    expect(screen.getByText('需求层 · 1')).toBeInTheDocument()
    expect(screen.getByText('共享取值 series/summer 变了')).toBeInTheDocument()
    expect(screen.getByText('受影响 1 张')).toBeInTheDocument()
  })

  it('shows the validation failure with the offending path', async () => {
    const broken = {
      ...bridge(),
      snapshot: vi.fn().mockRejectedValue(new Error('plan r3 无法解析')),
    }
    render(<PlanWorkspace bridge={broken} />)
    await waitFor(() => expect(screen.getByTestId('plan-error')).toBeInTheDocument())
    expect(screen.getByText('plan r3 无法解析')).toBeInTheDocument()
    expect(screen.getByText('重新读取')).toBeInTheDocument()
  })

  it('filters the canvas by generation state', async () => {
    render(<PlanWorkspace bridge={bridge()} />)
    await waitFor(() => expect(screen.getByTestId('plan-canvas')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /计划已变/ }))
    await waitFor(() =>
      expect(screen.queryByText('s1', { selector: '.plan-node__label' })).not.toBeInTheDocument(),
    )

    fireEvent.click(screen.getByRole('button', { name: /全部/ }))
    await waitFor(() =>
      expect(screen.getByText('s1', { selector: '.plan-node__label' })).toBeInTheDocument(),
    )
  })

  it('drills into a group and comes back', async () => {
    render(<PlanWorkspace bridge={bridge()} />)
    await waitFor(() => expect(screen.getByTestId('plan-canvas')).toBeInTheDocument())

    fireEvent.click(screen.getByText('夏', { selector: '.plan-node__label' }))
    const groupView = await screen.findByTestId('plan-group')
    expect(groupView).toHaveTextContent('秋季形象片 / 夏')
    expect(groupView).toHaveTextContent('1 张 · 已生成 0/1')

    fireEvent.click(screen.getByText('返回画布', { exact: false }))
    await waitFor(() => expect(screen.getByTestId('plan-canvas')).toBeInTheDocument())
  })

  it('opens the drawer for the selected shot', async () => {
    render(<PlanWorkspace bridge={bridge()} />)
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('s1'))

    const drawer = screen.getByTestId('plan-drawer')
    await waitFor(() => expect(drawer).toHaveTextContent('轻薄夏装'))
    expect(drawer).toHaveTextContent('海边')
  })

  it('writes a mark for the selected shot', async () => {
    const fake = bridge()
    render(<PlanWorkspace bridge={fake} />)
    await waitFor(() => expect(screen.getByText('s1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('s1'))
    fireEvent.change(screen.getByLabelText('标记备注'), { target: { value: '光太硬' } })
    fireEvent.click(await screen.findByText('保存标记'))
    await waitFor(() => expect(fake.writeMarks).toHaveBeenCalledOnce())

    const calls = (fake.writeMarks as unknown as { mock: { calls: unknown[][] } }).mock.calls
    const payload = calls[0]?.[0] as
      | {
          protocolVersion: string
          planId: string
          marks: { target: { kind: string; shotId: string }; revision: number; note?: string }[]
        }
      | undefined
    expect(payload?.protocolVersion).toBe('viewer.plan-marks/1')
    const added = payload?.marks.at(-1)
    expect(added?.target).toEqual({ kind: 'shot', shotId: 's1' })
    expect(added?.revision).toBe(4)
    expect(added?.note).toBe('光太硬')
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
    await waitFor(() => expect(screen.getByTestId('plan-empty')).toBeInTheDocument())
    expect(screen.getByText('项目里还没有生成计划')).toBeInTheDocument()
  })

  it('summarises mark states without inventing an approval', () => {
    expect(stalePanelCopy([])).toBe('还没有标记')
  })
})
