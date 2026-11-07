import { describe, expect, it } from 'vitest'
import { buildPlanGraph, buildRedoList, type PlanSnapshot, redoListDocument } from './planGraph'

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
    {
      shotId: 's2',
      groupId: 'g1',
      prompt: '轻薄夏装\n室内',
      hash: 'h2',
      references: [],
      generation: 'stale',
    },
    { shotId: 's3', groupId: 'g2', prompt: '厚重冬装\n雪地', hash: 'h3', references: [] },
  ],
  warnings: [],
  diff: { requirement: [], material: [], affectedShots: ['s2'] },
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
    expect(summer?.declared).toBe(3)
    expect(summer?.actual).toBe(2)
    expect(summer?.brief).toBe('轻薄夏装')
    expect(graph.nodes.find((node) => node.id === 'group:g2')?.countMismatch).toBe(false)
  })

  it('marks affected shots and their group', () => {
    const graph = buildPlanGraph(snapshot)
    expect(graph.nodes.find((node) => node.id === 'shot:s2')?.affected).toBe(true)
    expect(graph.nodes.find((node) => node.id === 'group:g1')?.affected).toBe(true)
    expect(graph.nodes.find((node) => node.id === 'shot:s1')?.affected).toBe(false)
  })

  it('carries the persistent generation state of each shot', () => {
    const graph = buildPlanGraph(snapshot)
    expect(graph.nodes.find((node) => node.id === 'shot:s2')?.generation).toBe('stale')
    expect(graph.nodes.find((node) => node.id === 'shot:s1')?.generation).toBe('ungenerated')
  })

  it('places ranks top to bottom without overlapping', () => {
    const graph = buildPlanGraph(snapshot)
    const plan = graph.nodes.find((node) => node.kind === 'plan')
    const group = graph.nodes.find((node) => node.id === 'group:g1')
    const shot = graph.nodes.find((node) => node.id === 'shot:s1')
    if (!plan || !group || !shot) throw new Error('expected plan, group and shot nodes')
    expect(group.y).toBeGreaterThan(plan.y + plan.height)
    expect(shot.y).toBeGreaterThan(group.y + group.height)

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

describe('buildRedoList', () => {
  it('lists every affected shot with its state', () => {
    const list = buildRedoList(snapshot)
    expect(list.map((entry) => entry.shotId)).toEqual(['s2'])
    expect(list[0]?.state).toBe('stale')
  })

  it('marks shots that disappeared from the revision as removed', () => {
    const list = buildRedoList({
      ...snapshot,
      diff: { requirement: [], material: [], affectedShots: ['gone'] },
    })
    expect(list[0]).toEqual({ shotId: 'gone', state: 'removed', intent: '', hash: '' })
  })

  it('returns nothing when there is no diff', () => {
    expect(buildRedoList(null)).toEqual([])
  })

  it('serialises a document agents can consume', () => {
    const document = JSON.parse(redoListDocument(snapshot)) as {
      protocolVersion: string
      shots: { shotId: string }[]
    }
    expect(document.protocolVersion).toBe('viewer.plan-redo/1')
    expect(document.shots[0]?.shotId).toBe('s2')
  })
})
