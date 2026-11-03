import { useCallback, useEffect, useMemo, useState } from 'react'
import type { PlanBridge, PlanMarksView, PlanSummary, ResolvedMark } from '../../api/plan'
import type { PlanSnapshot } from './planGraph'

export type PlanWorkspaceStatus = 'idle' | 'loading' | 'ready' | 'error'

export interface PlanWorkspaceState {
  status: PlanWorkspaceStatus
  plans: PlanSummary[]
  selectedPlanId: string | null
  snapshot: PlanSnapshot | null
  marks: ResolvedMark[]
  planAvailable: boolean
  error: string | null
}

export interface PlanMarkDraft {
  target: { kind: 'shot'; shotId: string } | { kind: 'group'; groupId: string }
  category: string
  note?: string
  observedHash?: string
}

export interface PlanWorkspaceController extends PlanWorkspaceState {
  selectPlan: (planId: string | null) => void
  reload: () => void
  addMark: (draft: PlanMarkDraft) => Promise<void>
  removeMark: (markId: string) => Promise<void>
}

const EMPTY: PlanWorkspaceState = {
  status: 'idle',
  plans: [],
  selectedPlanId: null,
  snapshot: null,
  marks: [],
  planAvailable: false,
  error: null,
}

export function usePlanWorkspace(bridge: PlanBridge | null): PlanWorkspaceController {
  const [state, setState] = useState<PlanWorkspaceState>(EMPTY)
  const [requestedPlanId, setRequestedPlanId] = useState<string | null>(null)
  const [reloadToken, setReloadToken] = useState(0)

  const selectPlan = useCallback((planId: string | null) => {
    setRequestedPlanId(planId)
  }, [])
  const reload = useCallback(() => setReloadToken((token) => token + 1), [])

  useEffect(() => {
    if (!bridge) {
      setState(EMPTY)
      return
    }
    let cancelled = false
    setState((current) => ({ ...current, status: 'loading', error: null }))

    void (async () => {
      try {
        const plans = await bridge.list()
        if (cancelled) return
        const selected =
          plans.find((plan) => plan.planId === requestedPlanId)?.planId ?? plans[0]?.planId ?? null
        if (!selected) {
          setState({ ...EMPTY, status: 'ready', plans })
          return
        }
        const [snapshot, marks] = await Promise.all([
          bridge.snapshot(selected),
          bridge.readMarks(selected),
        ])
        if (cancelled) return
        setState({
          status: 'ready',
          plans,
          selectedPlanId: selected,
          snapshot,
          marks: marks.marks,
          planAvailable: marks.planAvailable,
          error: null,
        })
      } catch (error) {
        if (cancelled) return
        setState({
          ...EMPTY,
          status: 'error',
          error: error instanceof Error ? error.message : String(error),
        })
      }
    })()

    return () => {
      cancelled = true
    }
  }, [bridge, requestedPlanId, reloadToken])

  const addMark = useCallback(
    async (draft: PlanMarkDraft) => {
      if (!bridge || !state.snapshot) return
      const mark = {
        id: `mk_${Date.now().toString(36)}`,
        revision: state.snapshot.revision,
        target: draft.target,
        category: draft.category,
        ...(draft.note ? { note: draft.note } : {}),
        ...(draft.observedHash ? { observedHash: draft.observedHash } : {}),
        createdAt: new Date().toISOString(),
      }
      await bridge.writeMarks({
        protocolVersion: 'viewer.plan-marks/1',
        planId: state.snapshot.planId,
        marks: [...state.marks.map((entry) => entry.mark), mark],
        updatedAt: new Date().toISOString(),
      })
      reload()
    },
    [bridge, state.snapshot, state.marks, reload],
  )

  const removeMark = useCallback(
    async (markId: string) => {
      if (!bridge || !state.snapshot) return
      await bridge.writeMarks({
        protocolVersion: 'viewer.plan-marks/1',
        planId: state.snapshot.planId,
        marks: state.marks.filter((entry) => entry.mark.id !== markId).map((entry) => entry.mark),
        updatedAt: new Date().toISOString(),
      })
      reload()
    },
    [bridge, state.snapshot, state.marks, reload],
  )

  return useMemo(
    () => ({ ...state, selectPlan, reload, addMark, removeMark }),
    [state, selectPlan, reload, addMark, removeMark],
  )
}

export function stalePanelCopy(marks: ResolvedMark[]): string {
  const pending = marks.filter((mark) => mark.status === 'pending').length
  const changed = marks.filter((mark) => mark.status === 'changed').length
  const replaced = marks.filter((mark) => mark.status === 'replaced').length
  const orphaned = marks.filter((mark) => mark.status === 'orphaned').length
  if (pending + changed + replaced + orphaned === 0) return '还没有标记'
  return `待处理 ${pending} · 已改 ${changed} · 已换 ${replaced} · 已失效 ${orphaned}`
}

export type { PlanMarksView }
