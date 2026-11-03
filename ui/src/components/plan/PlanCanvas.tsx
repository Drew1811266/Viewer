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
import { buildPlanGraph, type PlanFilter, type PlanGraphNode, type PlanSnapshot } from './planGraph'
import { STATE_LABELS } from './planLabels'

interface PlanNodeData extends Record<string, unknown> {
  label: string
  detail: string
  kind: PlanGraphNode['kind']
  countMismatch: boolean
  affected: boolean
  generation?: 'ungenerated' | 'current' | 'stale'
  declared?: number
  actual?: number
  generated?: number
  brief?: string
}

function PlanCard({ data }: NodeProps) {
  const node = data as PlanNodeData
  return (
    <div className="plan-node plan-node--plan">
      <span className="plan-node__icon" aria-hidden="true">
        ⌘
      </span>
      <div className="plan-node__body">
        <span className="plan-node__label">{node.label}</span>
        <span className="plan-node__meta">{node.detail}</span>
      </div>
    </div>
  )
}

function GroupCard({ data }: NodeProps) {
  const node = data as PlanNodeData
  const rendered = node.generated ?? 0
  return (
    <div
      className={`plan-node plan-node--group${node.countMismatch ? ' plan-node--mismatch' : ''}${
        node.affected ? ' plan-node--affected' : ''
      }`}
    >
      <div className="plan-node__row">
        <span className="plan-node__dot" aria-hidden="true" />
        <span className="plan-node__label">{node.label}</span>
      </div>
      <span className="plan-node__meta">{node.brief}</span>
      <div className="plan-node__thumbs" aria-hidden="true">
        <span className="plan-node__thumb" />
        <span className="plan-node__thumb" />
        <span className="plan-node__thumb" />
      </div>
      <div className="plan-node__row plan-node__row--footer">
        <span className="plan-node__strong">{node.actual} 张</span>
        <span className="plan-node__meta plan-node__meta--dim">
          已生成 {rendered}/{node.actual}
        </span>
        {node.countMismatch ? (
          <span className="plan-node__chip plan-node__chip--warn">⚠ 声明 {node.declared}</span>
        ) : null}
      </div>
    </div>
  )
}

function ShotCard({ data }: NodeProps) {
  const node = data as PlanNodeData
  const state = node.generation ?? 'ungenerated'
  return (
    <div className={`plan-node plan-node--shot plan-shot--${state}`} data-state={state}>
      <span className="plan-node__thumb plan-node__thumb--shot" aria-hidden="true" />
      <span className="plan-node__label">{node.label}</span>
      <span className="plan-node__meta plan-node__meta--dim plan-node__meta--clamp">
        {node.detail}
      </span>
      <span className={`plan-node__state plan-node__state--${state}`}>{STATE_LABELS[state]}</span>
    </div>
  )
}

const nodeTypes = { plan: PlanCard, group: GroupCard, shot: ShotCard }

export interface PlanCanvasProps {
  snapshot: PlanSnapshot
  filter?: PlanFilter
  selectedNodeId?: string | null
  onSelectNode?: (nodeId: string) => void
}

export function PlanCanvas({
  snapshot,
  filter = 'all',
  selectedNodeId,
  onSelectNode,
}: PlanCanvasProps) {
  const graph = useMemo(() => buildPlanGraph(snapshot, filter), [snapshot, filter])

  const nodes: Node[] = useMemo(
    () =>
      graph.nodes.map((node) => ({
        id: node.id,
        type: node.kind === 'shot' ? 'shot' : node.kind,
        position: { x: node.x, y: node.y },
        style: { width: node.width, height: node.height },
        selected: node.id === selectedNodeId,
        data: {
          label: node.label,
          detail: node.detail,
          kind: node.kind,
          countMismatch: node.countMismatch,
          affected: node.affected,
          generation: node.generation,
          declared: node.declared,
          actual: node.actual,
          generated: node.generated,
          brief: node.brief,
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
