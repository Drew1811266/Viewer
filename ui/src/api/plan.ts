import type { PlanSnapshot } from '../components/plan/planGraph'

export interface PlanSummary {
  planId: string
  revision: number
  ignored: number
}

export interface ResolvedMark {
  mark: {
    id: string
    revision: number
    target: { kind: 'shot'; shotId: string } | { kind: 'group'; groupId: string } | { kind: 'plan' }
    category: string
    field?: string | null
    note?: string | null
    observedHash?: string | null
    createdAt: string
  }
  status: 'pending' | 'changed' | 'replaced' | 'orphaned'
  resolvedTarget?: string
}

export interface PlanMarksView {
  planId: string
  planAvailable: boolean
  planRevision: number | null
  marks: ResolvedMark[]
}

export interface PlanBridge {
  list(projectRoot: string): Promise<PlanSummary[]>
  snapshot(projectRoot: string, planId: string): Promise<PlanSnapshot>
  readMarks(projectRoot: string, planId: string): Promise<PlanMarksView>
  writeMarks(projectRoot: string, marks: unknown): Promise<void>
}
