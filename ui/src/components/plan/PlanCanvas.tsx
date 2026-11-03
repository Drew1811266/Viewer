import {
  Background,
  Controls,
  type Edge,
  type Node,
  type NodeProps,
  ReactFlow,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { useMemo } from 'react'
import { buildPlanGraph, type PlanGraphNode, type PlanSnapshot } from './planGraph'

interface PlanNodeData extends Record<string, unknown> {
  label: string
  detail: string
  kind: PlanGraphNode['kind']
  countMismatch: boolean
  affected: boolean
}

function PlanNodeCard({ data }: NodeProps) {
  const node = data as PlanNodeData
  return (
    <div
      className={`plan-node plan-node--${node.kind}${node.affected ? ' plan-node--affected' : ''}${
        node.countMismatch ? ' plan-node--mismatch' : ''
      }`}
      data-kind={node.kind}
    >
      <span className="plan-node__label">{node.label}</span>
      <span className="plan-node__detail">{node.detail}</span>
      {node.countMismatch ? <span className="plan-node__badge">⚠</span> : null}
    </div>
  )
}

const nodeTypes = { plan: PlanNodeCard }

export interface PlanCanvasProps {
  snapshot: PlanSnapshot
  selectedNodeId?: string | null
  onSelectNode?: (nodeId: string) => void
}

export function PlanCanvas({ snapshot, selectedNodeId, onSelectNode }: PlanCanvasProps) {
  const graph = useMemo(() => buildPlanGraph(snapshot), [snapshot])

  const nodes: Node[] = useMemo(
    () =>
      graph.nodes.map((node) => ({
        id: node.id,
        type: 'plan',
        position: { x: node.x, y: node.y },
        style: { width: node.width, height: node.height },
        selected: node.id === selectedNodeId,
        data: {
          label: node.label,
          detail: node.detail,
          kind: node.kind,
          countMismatch: node.countMismatch,
          affected: node.affected,
        } satisfies PlanNodeData,
      })),
    [graph, selectedNodeId],
  )

  const edges: Edge[] = useMemo(
    () =>
      graph.edges.map((edge) => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
        type: 'smoothstep',
      })),
    [graph],
  )

  return (
    <div className="plan-canvas" data-testid="plan-canvas">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable
        fitView
        minZoom={0.4}
        maxZoom={1.6}
        onNodeClick={(_, node) => onSelectNode?.(node.id)}
      >
        <Background gap={24} />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  )
}
