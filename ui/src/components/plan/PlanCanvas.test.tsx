import './jsdomPolyfills'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { PlanCanvas } from './PlanCanvas'
import type { PlanSnapshot } from './planGraph'

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

  it('titles the plan node with the revision and totals', () => {
    render(<PlanCanvas snapshot={snapshot} />)
    expect(screen.getByText('r4 · 2 组 / 3 张')).toBeInTheDocument()
  })
})
