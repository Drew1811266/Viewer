import { readFileSync } from 'node:fs'
import './jsdomPolyfills'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { PlanCanvas } from './PlanCanvas'
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
  diff: null,
}

describe('PlanCanvas', () => {
  it('renders every group and the declared count mismatch', () => {
    render(<PlanCanvas snapshot={snapshot} />)
    expect(screen.getByText('夏')).toBeInTheDocument()
    expect(screen.getByText('冬')).toBeInTheDocument()
    expect(screen.getByText('⚠ 声明 3')).toBeInTheDocument()
  })

  it('maps trackpad gestures to pan and pinch-zoom', () => {
    const source = readFileSync('src/components/plan/PlanCanvas.tsx', 'utf8')
    expect(source).toContain('panOnScroll')
    expect(source).toContain('zoomOnPinch')
    expect(source).toContain('zoomOnScroll={false}')
    expect(source).toContain('panOnDrag')
  })

  it('wires connectors as bezier curves rather than stepped lines', () => {
    const source = readFileSync('src/components/plan/PlanCanvas.tsx', 'utf8')
    expect(source).toContain("type: 'default'")
    expect(source).not.toContain("type: 'smoothstep'")
  })

  it('titles the plan node with the revision and totals', () => {
    render(<PlanCanvas snapshot={snapshot} />)
    expect(screen.getByText('r4 · 2 组 / 3 张')).toBeInTheDocument()
  })

  it('gives every node the handles React Flow needs to draw its edges', () => {
    render(<PlanCanvas snapshot={snapshot} />)
    const graph = buildPlanGraph(snapshot)
    const nodes = document.querySelectorAll('.react-flow__node')
    expect(nodes.length).toBe(graph.nodes.length)
    for (const node of nodes) {
      expect(node.querySelector('.react-flow__handle-top')).not.toBeNull()
      expect(node.querySelector('.react-flow__handle-bottom')).not.toBeNull()
    }
  })

  // jsdom 没有布局，React Flow 拿不到节点尺寸就不会生成 SVG 边，
  // 因此这里只守住"Handle 存在"这个前提条件；边的实际绘制由真机验证。
})
