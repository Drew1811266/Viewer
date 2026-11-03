import dagre from '@dagrejs/dagre'

export interface PlanSnapshotGroup {
  id: string
  label: string
  declaredCount: number
  actualCount: number
  brief: string
}

export interface PlanSegment {
  kind: 'group' | 'value' | 'shot'
  label: string
  path: string
  text: string
}

export interface PlanSnapshotShot {
  shotId: string
  groupId: string
  prompt: string
  hash: string
  references: string[]
  segments?: PlanSegment[]
  generation?: 'ungenerated' | 'current' | 'stale'
}

export type PlanFilter = 'all' | 'ungenerated' | 'stale'

export interface PlanChange {
  layer: 'requirement' | 'material'
  kind: 'added' | 'removed' | 'modified'
  path: string
  message: string
}

export interface PlanDiff {
  requirement: PlanChange[]
  material: PlanChange[]
  affectedShots: string[]
}

export interface PlanSnapshot {
  planId: string
  title?: string | null
  revision: number
  blake3: string
  groups: PlanSnapshotGroup[]
  shots: PlanSnapshotShot[]
  warnings: { path: string; code: string; message: string }[]
  diff: PlanDiff | null
}

export type PlanNodeKind = 'plan' | 'group' | 'shot'

export interface PlanGraphNode {
  id: string
  kind: PlanNodeKind
  label: string
  detail: string
  countMismatch: boolean
  affected: boolean
  generation?: 'ungenerated' | 'current' | 'stale'
  declared?: number
  actual?: number
  generated?: number
  brief?: string
  x: number
  y: number
  width: number
  height: number
}

export interface PlanGraphEdge {
  id: string
  source: string
  target: string
}

export interface PlanGraph {
  nodes: PlanGraphNode[]
  edges: PlanGraphEdge[]
}

const PLAN_NODE = { width: 300, height: 88 }
const GROUP_NODE = { width: 300, height: 168 }
const SHOT_NODE = { width: 124, height: 116 }

export function buildPlanGraph(snapshot: PlanSnapshot, filter: PlanFilter = 'all'): PlanGraph {
  const affected = new Set(snapshot.diff?.affectedShots ?? [])
  const planId = `plan:${snapshot.planId}`

  const nodes: PlanGraphNode[] = [
    {
      id: planId,
      kind: 'plan',
      label: snapshot.planId,
      detail: `r${snapshot.revision} · ${snapshot.groups.length} 组 / ${snapshot.shots.length} 张`,
      countMismatch: false,
      affected: false,
      ...PLAN_NODE,
      x: 0,
      y: 0,
    },
  ]
  const edges: PlanGraphEdge[] = []

  for (const group of snapshot.groups) {
    const groupId = `group:${group.id}`
    const groupShots = snapshot.shots.filter((shot) => shot.groupId === group.id)
    const generated = groupShots.filter((shot) => shot.generation === 'current').length
    nodes.push({
      id: groupId,
      kind: 'group',
      label: group.label,
      detail: group.brief,
      brief: group.brief,
      declared: group.declaredCount,
      actual: group.actualCount,
      generated,
      countMismatch: group.declaredCount !== group.actualCount,
      affected: groupShots.some((shot) => affected.has(shot.shotId)),
      ...GROUP_NODE,
      x: 0,
      y: 0,
    })
    edges.push({ id: `${planId}->${groupId}`, source: planId, target: groupId })
  }

  const visibleShots = snapshot.shots.filter((shot) => {
    const state = shot.generation ?? 'ungenerated'
    if (filter === 'all') return true
    return state === filter
  })

  for (const shot of visibleShots) {
    const shotNodeId = `shot:${shot.shotId}`
    const parent = `group:${shot.groupId}`
    nodes.push({
      id: shotNodeId,
      kind: 'shot',
      label: shot.shotId,
      detail: shot.prompt.split('\n').at(-1) ?? '',
      countMismatch: false,
      affected: affected.has(shot.shotId),
      generation: shot.generation ?? 'ungenerated',
      ...SHOT_NODE,
      x: 0,
      y: 0,
    })
    if (snapshot.groups.some((group) => group.id === shot.groupId)) {
      edges.push({ id: `${parent}->${shotNodeId}`, source: parent, target: shotNodeId })
    }
  }

  return { nodes: layout(nodes, edges), edges }
}

function layout(nodes: PlanGraphNode[], edges: PlanGraphEdge[]): PlanGraphNode[] {
  const graph = new dagre.graphlib.Graph()
  graph.setGraph({ rankdir: 'LR', nodesep: 18, ranksep: 90 })
  graph.setDefaultEdgeLabel(() => ({}))

  for (const node of nodes) {
    graph.setNode(node.id, { width: node.width, height: node.height })
  }
  for (const edge of edges) {
    graph.setEdge(edge.source, edge.target)
  }
  dagre.layout(graph)

  return nodes.map((node) => {
    const positioned = graph.node(node.id)
    return {
      ...node,
      x: Math.round(positioned.x - node.width / 2),
      y: Math.round(positioned.y - node.height / 2),
    }
  })
}

export interface RedoEntry {
  shotId: string
  state: 'stale' | 'ungenerated' | 'removed'
  intent: string
  hash: string
}

export function buildRedoList(snapshot: PlanSnapshot | null): RedoEntry[] {
  if (!snapshot?.diff) return []
  const shots = new Map(snapshot.shots.map((shot) => [shot.shotId, shot]))
  return [...snapshot.diff.affectedShots].sort().map((shotId) => {
    const shot = shots.get(shotId)
    if (!shot) {
      return { shotId, state: 'removed' as const, intent: '', hash: '' }
    }
    return {
      shotId,
      state:
        shot.generation === 'current' ? ('stale' as const) : (shot.generation ?? 'ungenerated'),
      intent: shot.prompt.split('\n').at(-1) ?? '',
      hash: shot.hash,
    }
  })
}

export function redoListDocument(snapshot: PlanSnapshot): string {
  return JSON.stringify(
    {
      protocolVersion: 'viewer.plan-redo/1',
      planId: snapshot.planId,
      revision: snapshot.revision,
      generatedAt: new Date().toISOString(),
      requirement: snapshot.diff?.requirement ?? [],
      material: snapshot.diff?.material ?? [],
      shots: buildRedoList(snapshot),
    },
    null,
    2,
  )
}
