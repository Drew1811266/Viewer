import { invoke } from '@tauri-apps/api/core'
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

export const tauriPlanBridge: PlanBridge = {
  list: (projectRoot) => invoke('plan_list', { projectRoot }),
  snapshot: (projectRoot, planId) => invoke('plan_snapshot', { projectRoot, planId }),
  readMarks: (projectRoot, planId) => invoke('plan_marks_read', { projectRoot, planId }),
  writeMarks: (projectRoot, marks) => invoke('plan_marks_write', { projectRoot, marks }),
}
