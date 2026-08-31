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

type PresentationKind = 'history' | 'evidence' | 'source' | 'restore_preview'
interface PresentationOperation {
  token: number
  kind: PresentationKind
}
type TransactionKind = 'restore_commit' | 'continue_commit'
interface TransactionOperation {
  token: number
  kind: TransactionKind
  sessionKey: string | null
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
  restorePreview: MutableRefObject<number>,
  source: MutableRefObject<number>,
  setPanel: Dispatch<SetStateAction<HistoryPanelState | null>>,
  setSource: Dispatch<SetStateAction<SourceConfirmationState | null>>,
  invalidatePresentation: () => void,
  invalidateSourcePreparation: () => void,
  transactionPending: () => boolean,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  useLayoutEffect(() => {
    if (session.current === sessionKey) return
    session.current = sessionKey
    setPanel(null)
    setSource(null)
    invalidatePresentation()
    setError(null)
    setNotice(null)
    ++history.current
    ++evidence.current
    ++restorePreview.current
    ++source.current
  }, [sessionKey])
  useLayoutEffect(() => {
    if (selector.current === selectorKey) return
    selector.current = selectorKey
    ++history.current
    ++evidence.current
    ++restorePreview.current
    ++source.current
    setPanel(null)
    setSource(null)
    invalidatePresentation()
    setError(null)
  }, [selectorKey])
  useLayoutEffect(() => {
    if (entity.current === entityKey) return
    entity.current = entityKey
    ++source.current
    if (transactionPending()) return
    setSource(null)
    setError(null)
    invalidateSourcePreparation()
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
  transactionPending: () => boolean,
  beginPresentation: (kind: PresentationKind) => PresentationOperation,
  finishPresentation: (operation: PresentationOperation) => void,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  async function open() {
    if (!coordinator || !selector || transactionPending()) return
    const key = sessionKey
    const sequence = ++historySequence.current
    const operation = beginPresentation('history')
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
      finishPresentation(operation)
    }
  }
  async function requestEvidence(assetVersionId: string, role: 'base' | 'annotated') {
    if (!coordinator || !panel || panel.sessionKey !== sessionKey || transactionPending()) return
    const key = sessionKey
    const sequence = ++evidenceSequence.current
    const operation = beginPresentation('evidence')
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
      finishPresentation(operation)
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
  restorePreviewSequence: MutableRefObject<number>,
  sourcePreparationSequence: MutableRefObject<number>,
  transactionPending: () => boolean,
  beginPresentation: (kind: PresentationKind) => PresentationOperation,
  finishPresentation: (operation: PresentationOperation) => void,
  beginTransaction: (kind: TransactionKind) => TransactionOperation | null,
  finishTransaction: (operation: TransactionOperation) => void,
  setPanel: Dispatch<SetStateAction<HistoryPanelState | null>>,
  setSource: Dispatch<SetStateAction<SourceConfirmationState | null>>,
  setError: Dispatch<SetStateAction<ReviewWorkspaceError | null>>,
  setNotice: Dispatch<SetStateAction<string | null>>,
) {
  async function previewRestore(
    currentPanel: HistoryPanelState,
    decisions: ReviewRestoreDecision[],
    requestSessionKey: string | null,
  ) {
    if (coordinator === undefined || currentPanel.selector.kind !== 'archive') return
    const sequence = ++restorePreviewSequence.current
    const operation = beginPresentation('restore_preview')
    setError(null)
    try {
      const plan = await coordinator.previewRestore(currentPanel.selector.archiveId, decisions)
      if (!active(requestSessionKey) || sequence !== restorePreviewSequence.current) return
      setPanel((current) =>
        current?.sessionKey === requestSessionKey
          ? { ...current, restorePreview: { decisions: structuredClone(decisions), plan } }
          : current,
      )
    } catch (cause) {
      if (active(requestSessionKey) && sequence === restorePreviewSequence.current)
        setError(asReviewError(cause, '无法预览或恢复历史意见。'))
    } finally {
      finishPresentation(operation)
    }
  }

  async function commitRestore(requestSessionKey: string | null) {
    if (coordinator === undefined) return
    const operation = beginTransaction('restore_commit')
    if (operation === null) return
    setError(null)
    try {
      await coordinator.restore()
      if (!active(requestSessionKey)) return
      setPanel(null)
      setNotice('已追加恢复状态；历史记录没有被改写。')
    } catch (cause) {
      if (active(requestSessionKey)) setError(asReviewError(cause, '无法预览或恢复历史意见。'))
    } finally {
      finishTransaction(operation)
    }
  }

  async function restore(decisions: ReviewRestoreDecision[]) {
    if (coordinator === undefined || panel === null || transactionPending()) return
    const requestSessionKey = sessionKey
    if (panel.sessionKey !== requestSessionKey || panel.selector.kind !== 'archive') return
    const preview = panel.restorePreview
    if (preview === null || !sameRestoreDecisions(preview.decisions, decisions)) {
      await previewRestore(panel, decisions, requestSessionKey)
      return
    }
    if (preview.plan.restored.length === 0) {
      setPanel(null)
      setNotice('未恢复任何历史目标；当前意见保持不变。')
      return
    }
    await commitRestore(requestSessionKey)
  }

  async function continueHistorical(historyRef: ReviewHistoryRef) {
    const sourceRef = historyRef.source
    if (coordinator === undefined || transactionPending()) return
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
    const sequence = ++sourcePreparationSequence.current
    const operation = beginPresentation('source')
    setError(null)
    try {
      const prepared = await coordinator.prepareAssets(selectedEntityIds)
      if (!active(requestSessionKey) || sequence !== sourcePreparationSequence.current) return
      setSource({
        sessionKey: requestSessionKey,
        historyRef: structuredClone(historyRef),
        oldAsset,
        targetKey: structuredClone(selectedKey),
        originalAnchor: structuredClone(target.anchor),
        candidates: prepared.map((entry) => entry.asset),
      })
    } catch (cause) {
      if (active(requestSessionKey) && sequence === sourcePreparationSequence.current)
        setError(asReviewError(cause, '无法核验当前素材候选。'))
    } finally {
      finishPresentation(operation)
    }
  }

  async function confirmSource(decision: ReviewSourceBindingDecision) {
    if (
      coordinator === undefined ||
      source === null ||
      source.sessionKey !== sessionKey ||
      transactionPending()
    )
      return
    const requestSessionKey = sessionKey
    const operation = beginTransaction('continue_commit')
    if (operation === null) return
    setError(null)
    try {
      await coordinator.continueHistorical(source.historyRef, [decision])
      if (active(requestSessionKey)) {
        setSource(null)
        setPanel(null)
        setNotice('已基于历史原文继续提出；新意见身份由协调器生成。')
      }
    } catch (cause) {
      if (active(requestSessionKey)) setError(asReviewError(cause, '无法继续提出历史意见。'))
    } finally {
      finishTransaction(operation)
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
  const [presentationOperation, setPresentationOperation] = useState<PresentationOperation | null>(
    null,
  )
  const [transactionOperation, setTransactionOperation] = useState<TransactionOperation | null>(
    null,
  )
  const [error, setError] = useState<ReviewWorkspaceError | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const sessionKey = coordinator?.workbenchSessionKey ?? null
  const sessionKeyRef = useRef(sessionKey)
  const historySequence = useRef(0)
  const evidenceSequence = useRef(0)
  const restorePreviewSequence = useRef(0)
  const sourcePreparationSequence = useRef(0)
  const presentationSequence = useRef(0)
  const transactionSequence = useRef(0)
  const transactionRef = useRef<TransactionOperation | null>(null)
  const selectorKey = selector === undefined ? null : JSON.stringify(selector)
  const selectorKeyRef = useRef(selectorKey)
  const entityKey = selectedEntityIds.join('\u0000')
  const entityKeyRef = useRef(entityKey)

  function beginPresentation(kind: PresentationKind): PresentationOperation {
    const operation = { token: ++presentationSequence.current, kind }
    setPresentationOperation(operation)
    return operation
  }

  function finishPresentation(operation: PresentationOperation) {
    setPresentationOperation((current) => (current?.token === operation.token ? null : current))
  }

  function invalidatePresentation() {
    ++presentationSequence.current
    setPresentationOperation(null)
  }

  function invalidateSourcePreparation() {
    setPresentationOperation((current) => (current?.kind === 'source' ? null : current))
  }

  function transactionPending() {
    return transactionRef.current !== null
  }

  function beginTransaction(kind: TransactionKind): TransactionOperation | null {
    if (transactionRef.current !== null) return null
    const operation = { token: ++transactionSequence.current, kind, sessionKey }
    transactionRef.current = operation
    setTransactionOperation(operation)
    return operation
  }

  function finishTransaction(operation: TransactionOperation) {
    if (transactionRef.current?.token !== operation.token) return
    transactionRef.current = null
    setTransactionOperation((current) => (current?.token === operation.token ? null : current))
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
    restorePreviewSequence,
    sourcePreparationSequence,
    setPanel,
    setSource,
    invalidatePresentation,
    invalidateSourcePreparation,
    transactionPending,
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
    transactionPending,
    beginPresentation,
    finishPresentation,
    setError,
    setNotice,
  )
  const { close, closeSource } = useHistoryCloseActions(
    invalidatePresentation,
    transactionPending,
    historySequence,
    evidenceSequence,
    restorePreviewSequence,
    sourcePreparationSequence,
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
    restorePreviewSequence,
    sourcePreparationSequence,
    transactionPending,
    beginPresentation,
    finishPresentation,
    beginTransaction,
    finishTransaction,
    setPanel,
    setSource,
    setError,
    setNotice,
  )

  return {
    available: coordinator !== undefined && selector !== undefined,
    panel: panel?.sessionKey === sessionKey ? panel : null,
    source: source?.sessionKey === sessionKey ? source : null,
    busy: presentationOperation !== null || transactionOperation !== null,
    presentationBusy: presentationOperation !== null,
    transactionBusy: transactionOperation !== null,
    canClose: transactionOperation === null,
    error,
    notice,
    open,
    close,
    closeSource,
    invalidateRestorePreview: () => {
      if (transactionPending()) return
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

function useHistoryCloseActions(
  invalidatePresentation: () => void,
  transactionPending: () => boolean,
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
      if (transactionPending()) return
      ++history.current
      ++evidence.current
      ++restore.current
      ++source.current
      invalidatePresentation()
      setPanel(null)
      setSource(null)
      setError(null)
    },
    closeSource: () => {
      if (transactionPending()) return
      ++source.current
      invalidatePresentation()
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
