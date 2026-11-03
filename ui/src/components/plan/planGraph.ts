import dagre from '@dagrejs/dagre'

export interface PlanSnapshotGroup {
  id: string
  label: string
  declaredCount: number
  actualCount: number
  brief: string
}

export interface PlanSnapshotShot {
  shotId: string
  groupId: string
  prompt: string
  hash: string
  references: string[]
  generation?: 'ungenerated' | 'current' | 'stale'
}

export interface PlanSnapshot {
  planId: string
  revision: number
  blake3: string
  groups: PlanSnapshotGroup[]
  shots: PlanSnapshotShot[]
  warnings: { path: string; code: string; message: string }[]
  diff: null | { affectedShots: string[] }
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
  width: number
  height: number
  x: number
  y: number
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

const PLAN_NODE = { width: 260, height: 76 }
const GROUP_NODE = { width: 240, height: 84 }
const SHOT_NODE = { width: 200, height: 60 }

export function buildPlanGraph(snapshot: PlanSnapshot): PlanGraph {
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
    nodes.push({
      id: groupId,
      kind: 'group',
      label: group.label,
      detail:
        group.declaredCount === group.actualCount
          ? `${group.actualCount} 张`
          : `声明 ${group.declaredCount} · 实际 ${group.actualCount}`,
      countMismatch: group.declaredCount !== group.actualCount,
      affected: snapshot.shots.some(
        (shot) => shot.groupId === group.id && affected.has(shot.shotId),
      ),
      ...GROUP_NODE,
      x: 0,
      y: 0,
    })
    edges.push({ id: `${planId}->${groupId}`, source: planId, target: groupId })
  }

  for (const shot of snapshot.shots) {
    const shotNodeId = `shot:${shot.shotId}`
    const parent = `group:${shot.groupId}`
    nodes.push({
      id: shotNodeId,
      kind: 'shot',
      label: shot.shotId,
      detail: shot.prompt.split('\n')[0] ?? '',
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
