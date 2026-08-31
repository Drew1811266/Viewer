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
import { historyRefsForEntries } from './reviewHistoryModel'

interface HistoryPanelState {
  sessionKey: string | null
  selector: ReviewHistorySelector
  history: ReviewHistoryView
  historyRef: ReviewHistoryRef | null
  historyRefs: ReviewHistoryRef[]
  evidence: ReviewEvidenceImage[]
  restorePreview: { decisions: ReviewRestoreDecision[]; plan: ReviewRestorePlan } | null
}

type HistoryBusyKind = 'history' | 'evidence' | 'source' | 'restore_preview' | 'restore_commit'
interface HistoryBusyOperation {
  token: number
  kind: HistoryBusyKind
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
  invalidateBusy: () => void,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  useLayoutEffect(() => {
    if (session.current === sessionKey) return
    session.current = sessionKey
    setPanel(null)
    setSource(null)
    invalidateBusy()
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
    invalidateBusy()
    setError(null)
  }, [selectorKey])
  useLayoutEffect(() => {
    if (entity.current === entityKey) return
    entity.current = entityKey
    ++source.current
    setSource(null)
    invalidateBusy()
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
  beginBusy: (kind: HistoryBusyKind) => HistoryBusyOperation,
  finishBusy: (operation: HistoryBusyOperation) => void,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  async function open() {
    if (!coordinator || !selector) return
    const key = sessionKey
    const sequence = ++historySequence.current
    const operation = beginBusy('history')
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
        restorePreview: null,
      })
    } catch (cause) {
      if (active(key) && sequence === historySequence.current)
        setError(asReviewError(cause, '无法读取历史意见。'))
    } finally {
      finishBusy(operation)
    }
  }
  async function requestEvidence(assetVersionId: string, role: 'base' | 'annotated') {
    if (!coordinator || !panel || panel.sessionKey !== sessionKey) return
    const key = sessionKey
    const sequence = ++evidenceSequence.current
    const operation = beginBusy('evidence')
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
      finishBusy(operation)
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

function useHistoryWriteActions(
  coordinator: ContinuousReviewCoordinator | undefined,
  panel: HistoryPanelState | null,
  source: SourceConfirmationState | null,
  sessionKey: string | null,
  selectedEntityIds: string[],
  active: (key: string | null) => boolean,
  restoreSequence: MutableRefObject<number>,
  sourceSequence: MutableRefObject<number>,
  beginBusy: (kind: HistoryBusyKind) => HistoryBusyOperation,
  finishBusy: (operation: HistoryBusyOperation) => void,
  setPanel: Dispatch<SetStateAction<HistoryPanelState | null>>,
  setSource: Dispatch<SetStateAction<SourceConfirmationState | null>>,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  async function restore(decisions: ReviewRestoreDecision[]) {
    if (coordinator === undefined || panel === null) return
    const requestSessionKey = sessionKey
    const sequence = ++restoreSequence.current
    const operation = beginBusy(
      panel.restorePreview === null ? 'restore_preview' : 'restore_commit',
    )
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
            ? {
                ...current,
                restorePreview: { decisions: structuredClone(decisions), plan: outcome.plan },
              }
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
      finishBusy(operation)
    }
  }

  async function continueHistorical(historyRef: ReviewHistoryRef) {
    const sourceRef = historyRef.source
    if (coordinator === undefined) return
    if (sourceRef.kind !== 'snapshot' || sourceRef.keys.length !== 1) {
      setError(reviewWorkspaceError('invalid_data', '历史目标不完整，不能继续提出。', false))
      return
    }
    if (panel === null || panel.sessionKey !== sessionKey) return
    const selectedKey = sourceRef.keys[0]
    if (selectedKey === undefined) return
    const target = historicalTarget(panel.history, sourceRef.snapshot.snapshotId, selectedKey)
    if (target === null) {
      setError(reviewWorkspaceError('invalid_data', '历史目标不完整，不能继续提出。', false))
      return
    }
    const oldAsset = panel.history.entries
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
    const operation = beginBusy('source')
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
      finishBusy(operation)
    }
  }

  async function confirmSource(decision: ReviewSourceBindingDecision) {
    if (coordinator === undefined || source === null || source.sessionKey !== sessionKey) return
    const requestSessionKey = sessionKey
    const sequence = ++sourceSequence.current
    const operation = beginBusy('restore_commit')
    setError(null)
    try {
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
      finishBusy(operation)
    }
  }

  return { restore, continueHistorical, confirmSource }
}

/** Session-scoped history and source binding coordination kept out of the legacy workbench hook. */
export function useContinuousHistoryReview(
  coordinator: ContinuousReviewCoordinator | undefined,
  selectedEntityIds: string[],
  selector: ReviewHistorySelector | undefined,
) {
  const [panel, setPanel] = useState<HistoryPanelState | null>(null)
  const [source, setSource] = useState<SourceConfirmationState | null>(null)
  const [busyOperation, setBusyOperation] = useState<HistoryBusyOperation | null>(null)
  const [error, setError] = useState<ReviewWorkspaceError | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const sessionKey = coordinator?.workbenchSessionKey ?? null
  const sessionKeyRef = useRef(sessionKey)
  const historySequence = useRef(0)
  const evidenceSequence = useRef(0)
  const restoreSequence = useRef(0)
  const sourceSequence = useRef(0)
  const busySequence = useRef(0)
  const selectorKey = selector === undefined ? null : JSON.stringify(selector)
  const selectorKeyRef = useRef(selectorKey)
  const entityKey = selectedEntityIds.join('\u0000')
  const entityKeyRef = useRef(entityKey)

  function beginBusy(kind: HistoryBusyKind): HistoryBusyOperation {
    const operation = { token: ++busySequence.current, kind }
    setBusyOperation(operation)
    return operation
  }

  function finishBusy(operation: HistoryBusyOperation) {
    setBusyOperation((current) => (current?.token === operation.token ? null : current))
  }

  function invalidateBusy() {
    ++busySequence.current
    setBusyOperation(null)
  }

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
    invalidateBusy,
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
    beginBusy,
    finishBusy,
    setError,
    setNotice,
  )
  const { close, closeSource } = useHistoryCloseActions(
    invalidateBusy,
    historySequence,
    evidenceSequence,
    restoreSequence,
    sourceSequence,
    setPanel,
    setSource,
    setError,
  )

  const { restore, continueHistorical, confirmSource } = useHistoryWriteActions(
    coordinator,
    panel,
    source,
    sessionKey,
    selectedEntityIds,
    active,
    restoreSequence,
    sourceSequence,
    beginBusy,
    finishBusy,
    setPanel,
    setSource,
    setError,
    setNotice,
  )

  return {
    available: coordinator !== undefined && selector !== undefined,
    panel: panel?.sessionKey === sessionKey ? panel : null,
    source: source?.sessionKey === sessionKey ? source : null,
    busy: busyOperation !== null,
    canClose: busyOperation?.kind !== 'restore_commit',
    error,
    notice,
    open,
    close,
    closeSource,
    invalidateRestorePreview: () => {
      setPanel((current) =>
        current?.sessionKey === sessionKey && current.restorePreview !== null
          ? { ...current, restorePreview: null }
          : current,
      )
    },
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
  if (
    panel.restorePreview === null ||
    !sameRestoreDecisions(panel.restorePreview.decisions, decisions)
  ) {
    const plan = await coordinator.previewRestore(panel.selector.archiveId, decisions)
    if (!active(sessionKey)) return { kind: 'ignored' }
    return { kind: 'preview', plan }
  }
  if (panel.restorePreview.plan.restored.length === 0) return { kind: 'empty' }
  await coordinator.restore()
  return active(sessionKey) ? { kind: 'restored' } : { kind: 'ignored' }
}

function useHistoryCloseActions(
  invalidateBusy: () => void,
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
      ++history.current
      ++evidence.current
      ++restore.current
      ++source.current
      invalidateBusy()
      setPanel(null)
      setSource(null)
      setError(null)
    },
    closeSource: () => {
      ++source.current
      invalidateBusy()
      setSource(null)
      setError(null)
    },
  }
}

function historyRefsFor(
  history: ReviewHistoryView,
  coordinator: ContinuousReviewCoordinator,
): ReviewHistoryRef[] {
  const current = coordinator.view?.current
  if (current === undefined || current === null) return []
  return historyRefsForEntries(history, current.state.projectId, current.state.streamId)
}

function sameRestoreDecisions(left: ReviewRestoreDecision[], right: ReviewRestoreDecision[]) {
  return JSON.stringify(left) === JSON.stringify(right)
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
