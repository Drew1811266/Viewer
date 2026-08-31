import type { Dispatch, MutableRefObject, SetStateAction } from 'react'
import { useLayoutEffect, useRef, useState } from 'react'
import type {
  ReviewAssetVersion,
  ReviewEvidenceImage,
  ReviewHistoryRef,
  ReviewHistorySelector,
  ReviewHistoryView,
  ReviewRestoreDecision,
  ReviewRestorePlan,
  ReviewSourceBindingDecision,
  ReviewTargetVersionKey,
  ReviewWorkspaceError,
} from '../../api/reviewWorkspaceTypes'
import { reviewWorkspaceError } from '../../app/review/continuousReviewModel'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'

interface HistoryPanelState {
  sessionKey: string | null
  selector: ReviewHistorySelector
  history: ReviewHistoryView
  historyRef: ReviewHistoryRef | null
  historyRefs: ReviewHistoryRef[]
  evidence: ReviewEvidenceImage[]
  restorePlan: ReviewRestorePlan | null
}

function useHistoryLifecycle(
  sessionKey: string | null,
  selectorKey: string | null,
  entityKey: string,
  session: MutableRefObject<string | null>,
  selector: MutableRefObject<string | null>,
  entity: MutableRefObject<string>,
  history: MutableRefObject<number>,
  evidence: MutableRefObject<number>,
  restore: MutableRefObject<number>,
  source: MutableRefObject<number>,
  setPanel: Dispatch<SetStateAction<HistoryPanelState | null>>,
  setSource: Dispatch<SetStateAction<SourceConfirmationState | null>>,
  setBusy: Dispatch<SetStateAction<boolean>>,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  useLayoutEffect(() => {
    if (session.current === sessionKey) return
    session.current = sessionKey
    setPanel(null)
    setSource(null)
    setBusy(false)
    setError(null)
    setNotice(null)
    ++history.current
    ++evidence.current
    ++restore.current
    ++source.current
  }, [sessionKey])
  useLayoutEffect(() => {
    if (selector.current === selectorKey) return
    selector.current = selectorKey
    ++history.current
    ++evidence.current
    ++restore.current
    ++source.current
    setPanel(null)
    setSource(null)
    setError(null)
  }, [selectorKey])
  useLayoutEffect(() => {
    if (entity.current === entityKey) return
    entity.current = entityKey
    ++source.current
    setSource(null)
  }, [entityKey])
}

function useHistoryReadActions(
  coordinator: ContinuousReviewCoordinator | undefined,
  selector: ReviewHistorySelector | undefined,
  panel: HistoryPanelState | null,
  sessionKey: string | null,
  active: (key: string | null) => boolean,
  historySequence: MutableRefObject<number>,
  evidenceSequence: MutableRefObject<number>,
  setPanel: Dispatch<SetStateAction<HistoryPanelState | null>>,
  setBusy: Dispatch<SetStateAction<boolean>>,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  async function open() {
    if (!coordinator || !selector) return
    const key = sessionKey
    const sequence = ++historySequence.current
    setBusy(true)
    setError(null)
    setNotice(null)
    try {
      const history = await coordinator.getHistory(selector)
      if (!active(key) || sequence !== historySequence.current) return
      const refs = historyRefsFor(history, coordinator)
      setPanel({
        sessionKey: key,
        selector: structuredClone(selector),
        history,
        historyRef: refs[0] ?? null,
        historyRefs: refs,
        evidence: [],
        restorePlan: null,
      })
    } catch (cause) {
      if (active(key) && sequence === historySequence.current)
        setError(asReviewError(cause, '无法读取历史意见。'))
    } finally {
      if (active(key) && sequence === historySequence.current) setBusy(false)
    }
  }
  async function requestEvidence(assetVersionId: string, role: 'base' | 'annotated') {
    if (!coordinator || !panel || panel.sessionKey !== sessionKey) return
    const key = sessionKey
    const sequence = ++evidenceSequence.current
    setBusy(true)
    setError(null)
    try {
      const image = await coordinator.getEvidence(panel.selector, assetVersionId, role)
      if (!active(key) || sequence !== evidenceSequence.current) return
      setPanel((current) =>
        current?.sessionKey !== key
          ? current
          : {
              ...current,
              evidence: [
                ...current.evidence.filter(
                  (item) =>
                    item.assetVersionId !== image.assetVersionId || item.role !== image.role,
                ),
                image,
              ],
            },
      )
    } catch (cause) {
      if (active(key) && sequence === evidenceSequence.current)
        setError(asReviewError(cause, '历史证据无法读取。'))
    } finally {
      if (active(key) && sequence === evidenceSequence.current) setBusy(false)
    }
  }
  return { open, requestEvidence }
}

interface SourceConfirmationState {
  sessionKey: string | null
  historyRef: ReviewHistoryRef
  oldAsset: ReviewAssetVersion
  targetKey: ReviewTargetVersionKey
  originalAnchor: ReviewSourceBindingDecision['anchor']
  candidates: ReviewAssetVersion[]
}

/** Session-scoped history and source binding coordination kept out of the legacy workbench hook. */
export function useContinuousHistoryReview(
  coordinator: ContinuousReviewCoordinator | undefined,
  selectedEntityIds: string[],
  selector: ReviewHistorySelector | undefined,
) {
  const [panel, setPanel] = useState<HistoryPanelState | null>(null)
  const [source, setSource] = useState<SourceConfirmationState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ReviewWorkspaceError | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const sessionKey = coordinator?.workbenchSessionKey ?? null
  const sessionKeyRef = useRef(sessionKey)
  const historySequence = useRef(0)
  const evidenceSequence = useRef(0)
  const restoreSequence = useRef(0)
  const sourceSequence = useRef(0)
  const selectorKey = selector === undefined ? null : JSON.stringify(selector)
  const selectorKeyRef = useRef(selectorKey)
  const entityKey = selectedEntityIds.join('\u0000')
  const entityKeyRef = useRef(entityKey)

  useHistoryLifecycle(
    sessionKey,
    selectorKey,
    entityKey,
    sessionKeyRef,
    selectorKeyRef,
    entityKeyRef,
    historySequence,
    evidenceSequence,
    restoreSequence,
    sourceSequence,
    setPanel,
    setSource,
    setBusy,
    setError,
    setNotice,
  )

  const active = (requestSessionKey: string | null) => sessionKeyRef.current === requestSessionKey

  const { open, requestEvidence } = useHistoryReadActions(
    coordinator,
    selector,
    panel,
    sessionKey,
    active,
    historySequence,
    evidenceSequence,
    setPanel,
    setBusy,
    setError,
    setNotice,
  )
  const { close, closeSource } = useHistoryCloseActions(
    busy,
    historySequence,
    evidenceSequence,
    restoreSequence,
    sourceSequence,
    setPanel,
    setSource,
    setError,
  )

  async function restore(decisions: ReviewRestoreDecision[]) {
    if (coordinator === undefined || panel === null) return
    const requestSessionKey = sessionKey
    const sequence = ++restoreSequence.current
    setBusy(true)
    setError(null)
    try {
      const outcome = await runHistoryRestore(
        coordinator,
        panel,
        requestSessionKey,
        active,
        decisions,
      )
      if (sequence !== restoreSequence.current || outcome.kind === 'ignored') return
      if (outcome.kind === 'preview') {
        setPanel((current) =>
          current?.sessionKey === requestSessionKey
            ? { ...current, restorePlan: outcome.plan }
            : current,
        )
        return
      }
      if (outcome.kind === 'empty') {
        setPanel(null)
        setNotice('未恢复任何历史目标；当前意见保持不变。')
        return
      }
      setPanel(null)
      setNotice('已追加恢复状态；历史记录没有被改写。')
    } catch (cause) {
      if (active(requestSessionKey) && sequence === restoreSequence.current)
        setError(asReviewError(cause, '无法预览或恢复历史意见。'))
    } finally {
      if (active(requestSessionKey) && sequence === restoreSequence.current) setBusy(false)
    }
  }

  async function continueHistorical(historyRef: ReviewHistoryRef) {
    const sourceRef = historyRef.source
    if (coordinator === undefined || sourceRef.kind !== 'snapshot') return
    const currentPanel = panel
    if (currentPanel === null || currentPanel.sessionKey !== sessionKey) return
    const selectedKey = sourceRef.keys[0]
    if (selectedKey === undefined) {
      setError(reviewWorkspaceError('invalid_data', '历史目标不完整，不能继续提出。', false))
      return
    }
    const target = historicalTarget(
      currentPanel.history,
      sourceRef.snapshot.snapshotId,
      selectedKey,
    )
    if (target === null) {
      setError(reviewWorkspaceError('invalid_data', '历史目标不完整，不能继续提出。', false))
      return
    }
    const oldAsset = currentPanel.history.entries
      .find((entry) => entry.snapshot.snapshotId === sourceRef.snapshot.snapshotId)
      ?.assets.find((asset) => asset.id === target.assetVersionId)
    if (oldAsset === undefined) {
      setError(reviewWorkspaceError('asset_unavailable', '历史素材不可用，不能确认新素材。', false))
      return
    }
    if (selectedEntityIds.length === 0) {
      setError(reviewWorkspaceError('needs_confirmation', '请先选择一个当前素材作为候选。', false))
      return
    }
    const requestSessionKey = sessionKey
    const sequence = ++sourceSequence.current
    setBusy(true)
    setError(null)
    try {
      const prepared = await coordinator.prepareAssets(selectedEntityIds)
      if (!active(requestSessionKey) || sequence !== sourceSequence.current) return
      setSource({
        sessionKey: requestSessionKey,
        historyRef: structuredClone(historyRef),
        oldAsset,
        targetKey: structuredClone(selectedKey),
        originalAnchor: structuredClone(target.anchor),
        candidates: prepared.map((entry) => entry.asset),
      })
    } catch (cause) {
      if (active(requestSessionKey) && sequence === sourceSequence.current)
        setError(asReviewError(cause, '无法核验当前素材候选。'))
    } finally {
      if (active(requestSessionKey) && sequence === sourceSequence.current) setBusy(false)
    }
  }

  async function confirmSource(decision: ReviewSourceBindingDecision) {
    if (coordinator === undefined || source === null || source.sessionKey !== sessionKey) return
    const requestSessionKey = sessionKey
    const sequence = ++sourceSequence.current
    setBusy(true)
    setError(null)
    try {
      // The backend prepare envelope, not this UI, owns the new Feedback/Target identities.
      await coordinator.continueHistorical(source.historyRef, [decision])
      if (active(requestSessionKey) && sequence === sourceSequence.current) {
        setSource(null)
        setPanel(null)
        setNotice('已基于历史原文继续提出；新意见身份由协调器生成。')
      }
    } catch (cause) {
      if (active(requestSessionKey) && sequence === sourceSequence.current)
        setError(asReviewError(cause, '无法继续提出历史意见。'))
    } finally {
      if (active(requestSessionKey) && sequence === sourceSequence.current) setBusy(false)
    }
  }

  return {
    available: coordinator !== undefined && selector !== undefined,
    panel: panel?.sessionKey === sessionKey ? panel : null,
    source: source?.sessionKey === sessionKey ? source : null,
    busy,
    error,
    notice,
    open,
    close,
    closeSource,
    requestEvidence,
    restore,
    continueHistorical,
    confirmSource,
  }
}

async function runHistoryRestore(
  coordinator: ContinuousReviewCoordinator,
  panel: HistoryPanelState,
  sessionKey: string | null,
  active: (key: string | null) => boolean,
  decisions: ReviewRestoreDecision[],
): Promise<
  | { kind: 'ignored' }
  | { kind: 'preview'; plan: ReviewRestorePlan }
  | { kind: 'empty' }
  | { kind: 'restored' }
> {
  if (panel.sessionKey !== sessionKey || panel.selector.kind !== 'archive')
    return { kind: 'ignored' }
  const plan = await coordinator.previewRestore(panel.selector.archiveId, decisions)
  if (!active(sessionKey)) return { kind: 'ignored' }
  if (panel.restorePlan === null || plan.conflicts.length > 0) return { kind: 'preview', plan }
  if (plan.restored.length === 0) return { kind: 'empty' }
  await coordinator.restore()
  return active(sessionKey) ? { kind: 'restored' } : { kind: 'ignored' }
}

function useHistoryCloseActions(
  busy: boolean,
  history: MutableRefObject<number>,
  evidence: MutableRefObject<number>,
  restore: MutableRefObject<number>,
  source: MutableRefObject<number>,
  setPanel: Dispatch<SetStateAction<HistoryPanelState | null>>,
  setSource: Dispatch<SetStateAction<SourceConfirmationState | null>>,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
) {
  return {
    close: () => {
      if (!busy) {
        ++history.current
        ++evidence.current
        ++restore.current
        ++source.current
        setPanel(null)
        setSource(null)
        setError(null)
      }
    },
    closeSource: () => {
      if (!busy) {
        ++source.current
        setSource(null)
        setError(null)
      }
    },
  }
}

function historyRefsFor(
  history: ReviewHistoryView,
  coordinator: ContinuousReviewCoordinator,
): ReviewHistoryRef[] {
  const current = coordinator.view?.current
  if (current === undefined || current === null) return []
  return history.entries.flatMap((entry) =>
    entry.selected.map((key) => ({
      projectId: current.state.projectId,
      streamId: current.state.streamId,
      source: {
        kind: 'snapshot' as const,
        snapshot: structuredClone(entry.snapshot),
        keys: [structuredClone(key)],
      },
    })),
  )
}

function historicalTarget(
  history: ReviewHistoryView,
  snapshotId: string,
  key: ReviewTargetVersionKey | undefined,
) {
  if (key === undefined) return null
  return (
    history.entries
      .find((entry) => entry.snapshot.snapshotId === snapshotId)
      ?.feedback.find(
        (feedback) =>
          feedback.id === key.feedbackId && feedback.textRevisionId === key.textRevisionId,
      )
      ?.targets.find(
        (target) => target.id === key.targetId && target.revisionId === key.targetRevisionId,
      ) ?? null
  )
}

function asReviewError(cause: unknown, fallback: string): ReviewWorkspaceError {
  if (typeof cause === 'object' && cause !== null && 'code' in cause && 'message' in cause) {
    return cause as ReviewWorkspaceError
  }
  return reviewWorkspaceError('internal', fallback, true)
}
