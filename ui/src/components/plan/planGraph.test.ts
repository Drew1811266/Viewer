import { describe, expect, it } from 'vitest'
import { buildPlanGraph, type PlanSnapshot } from './planGraph'

const snapshot: PlanSnapshot = {
  planId: 'autumn',
  revision: 4,
  blake3: 'hash',
  groups: [
    { id: 'g1', label: '夏', declaredCount: 3, actualCount: 2, brief: '轻薄夏装' },
    { id: 'g2', label: '冬', declaredCount: 1, actualCount: 1, brief: '厚重冬装' },
  ],
  shots: [
    { shotId: 's1', groupId: 'g1', prompt: '轻薄夏装\n海边', hash: 'h1', references: [] },
    { shotId: 's2', groupId: 'g1', prompt: '轻薄夏装\n室内', hash: 'h2', references: [] },
    { shotId: 's3', groupId: 'g2', prompt: '厚重冬装\n雪地', hash: 'h3', references: [] },
  ],
  warnings: [],
  diff: { affectedShots: ['s2'] },
}

describe('buildPlanGraph', () => {
  it('renders one plan node, one node per group and per shot', () => {
    const graph = buildPlanGraph(snapshot)
    expect(graph.nodes.filter((node) => node.kind === 'plan')).toHaveLength(1)
    expect(graph.nodes.filter((node) => node.kind === 'group')).toHaveLength(2)
    expect(graph.nodes.filter((node) => node.kind === 'shot')).toHaveLength(3)
  })

  it('connects every group to the plan and every shot to its group', () => {
    const graph = buildPlanGraph(snapshot)
    expect(graph.edges).toContainEqual({
      id: 'plan:autumn->group:g1',
      source: 'plan:autumn',
      target: 'group:g1',
    })
    expect(graph.edges).toContainEqual({
      id: 'group:g1->shot:s2',
      source: 'group:g1',
      target: 'shot:s2',
    })
  })

  it('marks the group whose declared count disagrees with reality', () => {
    const graph = buildPlanGraph(snapshot)
    const summer = graph.nodes.find((node) => node.id === 'group:g1')
    expect(summer?.countMismatch).toBe(true)
    expect(summer?.detail).toBe('声明 3 · 实际 2')
    expect(graph.nodes.find((node) => node.id === 'group:g2')?.countMismatch).toBe(false)
  })

  it('marks affected shots and their group', () => {
    const graph = buildPlanGraph(snapshot)
    expect(graph.nodes.find((node) => node.id === 'shot:s2')?.affected).toBe(true)
    expect(graph.nodes.find((node) => node.id === 'group:g1')?.affected).toBe(true)
    expect(graph.nodes.find((node) => node.id === 'shot:s1')?.affected).toBe(false)
  })

  it('places ranks left to right without overlapping', () => {
    const graph = buildPlanGraph(snapshot)
    const plan = graph.nodes.find((node) => node.kind === 'plan')
    const group = graph.nodes.find((node) => node.id === 'group:g1')
    const shot = graph.nodes.find((node) => node.id === 'shot:s1')
    if (!plan || !group || !shot) throw new Error('expected plan, group and shot nodes')
    expect(group.x).toBeGreaterThan(plan.x + plan.width)
    expect(shot.x).toBeGreaterThan(group.x + group.width)

    const boxes = graph.nodes.map((node) => ({
      x: node.x,
      y: node.y,
      right: node.x + node.width,
      bottom: node.y + node.height,
    }))
    for (const [left, a] of boxes.entries()) {
      for (const b of boxes.slice(left + 1)) {
        const overlaps = a.x < b.right && b.x < a.right && a.y < b.bottom && b.y < a.bottom
        expect(overlaps).toBe(false)
      }
    }
  })
})
