import type {
  PreparedReviewCommand,
  PrepareReviewCommandRequest,
  ReviewCommitReceipt,
  ReviewWorkspaceCommand,
  ReviewWorkspaceError,
  ReviewWorkspacePort,
  ReviewWorkspaceSessionRequest,
} from '../../api/reviewWorkspaceTypes'
import { continuousReviewActions } from './continuousReviewActions'
import {
  type ContinuousReviewEditorSeed,
  type ContinuousReviewSnapshot,
  continuousReviewState,
  deepFreezeReviewValue,
  emptyContinuousEditor,
  normalizeReviewWorkspaceError,
  requiresReviewReconciliation,
  reviewWorkspaceError,
  sameReviewValue,
} from './continuousReviewModel'

interface PendingCommand {
  request: PrepareReviewCommandRequest
  envelope: PreparedReviewCommand | null
  inputRevision: number | null
  frozenInput: ContinuousReviewEditorSeed | null
  error: ReviewWorkspaceError | null
}

/** Owns one desktop session. React owns its lifetime, never protocol or file IO. */
export class ContinuousReviewSession {
  private snapshot: ContinuousReviewSnapshot = {
    state: { kind: 'loading' },
    view: null,
    editorInput: emptyContinuousEditor(),
    pendingEnvelope: null,
    lastReceipt: null,
    error: null,
  }
  private listeners = new Set<() => void>()
  private pending: PendingCommand | null = null
  private inputRevision = 0
  private operation: Promise<void> | null = null
  private active = false
  private epoch = 0
  private loadSequence = 0
  private loading = false
  private cancelRequested = false
  private closing: Promise<unknown> | null = null
  private cancelling = false
  private cancellationCompletion: Promise<unknown> | null = null
  private readonly port: ReviewWorkspacePort
  private readonly scope: ReviewWorkspaceSessionRequest
  private readonly onError: (error: ReviewWorkspaceError) => void
  private viewVersion = 0
  private querySequences = new Map<string, number>()
  readonly actions: ReturnType<typeof continuousReviewActions>

  constructor(
    port: ReviewWorkspacePort,
    scope: ReviewWorkspaceSessionRequest,
    onError: (error: ReviewWorkspaceError) => void,
  ) {
    this.port = port
    this.scope = scope
    this.onError = onError
    this.actions = continuousReviewActions(port, {
      getSnapshot: this.getSnapshot,
      version: () => this.viewVersion,
      query: this.query,
      submit: (command, expected) => this.submit(command, expected),
      reject: (error) => this.reject(error),
    })
  }

  getSnapshot = () => this.snapshot
  subscribe = (listener: () => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }
  private update(patch: Partial<ContinuousReviewSnapshot>) {
    this.snapshot = { ...this.snapshot, ...patch }
    for (const listener of this.listeners) listener()
  }
  start = (handover: Promise<unknown> | null = null) => {
    const epoch = ++this.epoch
    this.active = true
    const load = () => (this.isActive(epoch) ? this.refresh() : Promise.resolve())
    // StrictMode's setup/cleanup/setup must not let the old cancellation cancel the new load.
    const barrier = handover ?? this.closing
    void (barrier === null ? load() : barrier.then(load)).catch(() => {})
    return () => {
      this.active = false
      this.epoch += 1
      this.loadSequence += 1
      this.closing = Promise.allSettled([
        this.cancellationCompletion,
        this.port.cancelTask(this.scope),
      ])
      return this.closing
    }
  }
  private isActive(epoch = this.epoch) {
    return this.active && epoch === this.epoch
  }
  private stale(receipt: ReviewCommitReceipt | null = null): ReviewWorkspaceError {
    return {
      ...reviewWorkspaceError('stale_session', '评审会话已切换或关闭', false),
      committedReceipt: receipt,
    }
  }
  private notify(error: ReviewWorkspaceError) {
    try {
      this.onError(error)
    } catch {
      /* Error reporting cannot change command outcome. */
    }
  }
  private reject(error: ReviewWorkspaceError): Promise<never> {
    if (this.isActive()) {
      this.update({ error })
      this.notify(error)
    }
    return Promise.reject(error)
  }
  private recoveryError(): ReviewWorkspaceError | null {
    return this.snapshot.view?.recovery.some(
      (draft) => draft.commandId !== this.pending?.request.commandId,
    )
      ? reviewWorkspaceError('needs_confirmation', '存在待处理恢复输入', false)
      : null
  }
  private editingError(kind?: ReviewWorkspaceCommand['kind']): ReviewWorkspaceError | null {
    if (!this.isActive()) return this.stale()
    if (this.cancelling) return reviewWorkspaceError('busy', '正在取消上一个操作', true)
    if (this.loading || this.snapshot.view === null)
      return reviewWorkspaceError('busy', '正在加载评审', true)
    if (this.snapshot.state.kind === 'unavailable') return this.snapshot.state.error
    const recovery = this.recoveryError()
    if (recovery !== null) return recovery
    if (kind === 'migrate' && this.snapshot.view.migration !== null) {
      return this.snapshot.view.capabilities.migration
        ? null
        : reviewWorkspaceError('read_only', '项目不可写', false)
    }
    if (this.snapshot.view.migration !== null)
      return reviewWorkspaceError('migration_required', '需要先确认迁移', false)
    if (!this.snapshot.view.capabilities.continuousEditing)
      return reviewWorkspaceError('read_only', '项目不可写', false)
    if (kind === 'adopt_usage' && !this.snapshot.view.capabilities.usageImport)
      return reviewWorkspaceError('capability_unavailable', '项目不支持使用依据导入', false)
    return null
  }
  refresh = async () => {
    if (!this.isActive()) throw this.stale()
    if (this.operation !== null || this.cancelling)
      return this.reject(reviewWorkspaceError('busy', '操作尚未结束', true))
    const epoch = this.epoch
    const sequence = ++this.loadSequence
    this.viewVersion += 1
    this.loading = true
    if (this.pending === null) this.update({ state: { kind: 'loading' }, error: null })
    try {
      const view = await this.port.getWorkspace(this.scope)
      if (!this.isActive(epoch)) throw this.stale()
      if (sequence !== this.loadSequence)
        throw reviewWorkspaceError('cancelled', '已由后续读取替代', true)
      const error = this.pending?.error ?? null
      this.update({
        view,
        error,
        state: error === null ? continuousReviewState(view) : this.failureState(error),
      })
    } catch (cause) {
      if (!this.isActive(epoch))
        throw this.stale(normalizeReviewWorkspaceError(cause).committedReceipt)
      const error = normalizeReviewWorkspaceError(cause)
      if (sequence === this.loadSequence) {
        this.update({
          error,
          state: this.pending?.error
            ? this.failureState(this.pending.error)
            : { kind: 'unavailable', error },
        })
        this.notify(error)
      }
      throw error
    } finally {
      if (sequence === this.loadSequence && this.isActive(epoch)) this.loading = false
    }
  }
  cancel = async () => {
    if (!this.isActive()) throw this.stale()
    if (this.cancelling)
      return this.reject(reviewWorkspaceError('busy', '正在取消上一个操作', true))
    this.cancelling = true
    const epoch = this.epoch
    const wasWriting = this.operation !== null
    this.cancelRequested = true
    this.viewVersion += 1
    ++this.loadSequence
    this.loading = false
    try {
      const acknowledgement = this.port.cancelTask(this.scope)
      this.cancellationCompletion = acknowledgement.catch(() => {})
      await acknowledgement
      if (!this.isActive(epoch)) throw this.stale()
      if (!wasWriting && this.operation === null) {
        const error =
          this.pending?.error ?? reviewWorkspaceError('cancelled', '已取消，输入仍保留', true)
        this.update({
          error,
          state: this.pending?.error
            ? this.failureState(this.pending.error)
            : this.snapshot.view
              ? continuousReviewState(this.snapshot.view)
              : { kind: 'unavailable', error },
        })
      }
    } catch (cause) {
      if (!this.isActive(epoch)) throw this.stale()
      return this.reject(normalizeReviewWorkspaceError(cause))
    } finally {
      this.cancelling = false
    }
  }
  beginEditor = (input: ContinuousReviewEditorSeed) => {
    if (
      this.editingError() !== null ||
      this.hasUncommittedInput() ||
      this.operation !== null ||
      (this.pending?.error !== null && this.pending !== null)
    )
      return false
    this.update({
      editorInput: {
        ...structuredClone(input),
        baseSnapshotId: this.snapshot.view?.current?.reference.snapshotId ?? null,
      },
    })
    this.inputRevision += 1
    return true
  }
  setEditorText = (text: string) => {
    if (!this.isActive() || this.snapshot.editorInput.contextKey === null) return
    this.inputRevision += 1
    this.update({ editorInput: { ...this.snapshot.editorInput, text } })
  }
  setEditorTargets = (targets: ContinuousReviewEditorSeed['targets']) => {
    if (
      !this.isActive() ||
      this.snapshot.editorInput.contextKey === null ||
      this.operation !== null ||
      (this.pending?.error && requiresReviewReconciliation(this.pending.error))
    )
      return false
    this.inputRevision += 1
    this.update({
      editorInput: { ...this.snapshot.editorInput, targets: structuredClone(targets) },
    })
    return true
  }
  /** Explicit user confirmation only: a refresh never silently rebases a draft. */
  rebaseEditor = () => {
    if (
      this.editingError() !== null ||
      !this.hasUncommittedInput() ||
      this.operation !== null ||
      (this.pending?.error && requiresReviewReconciliation(this.pending.error))
    )
      return false
    this.inputRevision += 1
    this.update({
      editorInput: {
        ...this.snapshot.editorInput,
        baseSnapshotId: this.snapshot.view?.current?.reference.snapshotId ?? null,
      },
    })
    return true
  }
  discardEditor = () => {
    if (
      !this.isActive() ||
      this.operation !== null ||
      (this.pending?.error && requiresReviewReconciliation(this.pending.error))
    )
      return false
    this.pending = null
    this.inputRevision += 1
    this.update({
      editorInput: emptyContinuousEditor(),
      pendingEnvelope: null,
      error: null,
      state: this.snapshot.view ? continuousReviewState(this.snapshot.view) : { kind: 'loading' },
    })
    return true
  }
  hasUncommittedInput = () => this.snapshot.editorInput.contextKey !== null

  saveFeedback = () => {
    const blocked = this.editingError()
    if (blocked !== null) return this.reject(blocked)
    const input = this.snapshot.editorInput
    if (input.contextKey === null)
      return this.reject(reviewWorkspaceError('no_changes', '没有待保存意见', false))
    const frozenInput: ContinuousReviewEditorSeed = structuredClone(input)
    const command = {
      kind: 'save_feedback' as const,
      feedbackId: input.feedbackId,
      text: input.text,
      targets: structuredClone(input.targets),
    }
    return this.submit(command, input.baseSnapshotId, frozenInput)
  }

  private submit(
    command: ReviewWorkspaceCommand,
    expectedSnapshotId: string | null,
    frozenInput: ContinuousReviewEditorSeed | null = null,
  ) {
    const blocked = this.editingError(command.kind)
    if (blocked !== null) return this.reject(blocked)
    if (this.pending !== null) {
      if (
        sameReviewValue(this.pending.request.command, command) &&
        this.pending.request.expectedSnapshotId === expectedSnapshotId
      )
        return this.retry()
      if (this.operation !== null)
        return this.reject(reviewWorkspaceError('busy', '请先等待当前保存结果', true))
      if (this.pending.error !== null && requiresReviewReconciliation(this.pending.error)) {
        return Promise.reject(this.pending.error)
      }
    }
    if (frozenInput === null && this.hasUncommittedInput())
      return this.reject(
        reviewWorkspaceError('needs_confirmation', '请先保存或放弃未提交输入', false),
      )
    this.pending = {
      request: {
        ...this.scope,
        commandId: crypto.randomUUID(),
        expectedSnapshotId,
        command: deepFreezeReviewValue(structuredClone(command)),
      },
      envelope: null,
      inputRevision: frozenInput === null ? null : this.inputRevision,
      frozenInput,
      error: null,
    }
    this.update({ pendingEnvelope: null })
    return this.runOperation()
  }
  retry = () => {
    if (!this.isActive()) return Promise.reject(this.stale())
    if (this.cancelling)
      return this.reject(reviewWorkspaceError('busy', '正在取消上一个操作', true))
    if (this.loading) return this.reject(reviewWorkspaceError('busy', '正在加载评审', true))
    const recovery = this.pending === null ? null : this.recoveryError()
    if (recovery !== null) return this.reject(recovery)
    if (
      this.pending?.frozenInput &&
      (!sameReviewValue(this.pending.frozenInput.targets, this.snapshot.editorInput.targets) ||
        this.pending.request.expectedSnapshotId !== this.snapshot.editorInput.baseSnapshotId)
    ) {
      return this.reject(
        reviewWorkspaceError(
          'needs_confirmation',
          '标记或版本依据已调整，请明确保存当前编辑',
          false,
        ),
      )
    }
    if (
      this.pending?.error &&
      !this.pending.error.retryable &&
      this.pending.error.committedReceipt === null
    ) {
      return this.reject(this.pending.error)
    }
    return this.operation ?? (this.pending === null ? this.refresh() : this.runOperation())
  }

  private runOperation() {
    this.viewVersion += 1
    this.cancelRequested = false
    const operation = this.applyPending()
    this.operation = operation
    operation.then(
      () => {
        if (this.operation === operation) this.operation = null
      },
      () => {
        if (this.operation === operation) this.operation = null
      },
    )
    return operation
  }

  private async applyPending() {
    const pending = this.pending
    if (pending === null) return
    const epoch = this.epoch
    try {
      this.update({
        state: { kind: 'saving', stage: pending.envelope !== null ? 'applying' : 'preparing' },
        error: null,
      })
      pending.envelope ??= deepFreezeReviewValue(
        structuredClone(await this.port.prepareCommand(pending.request)),
      )
      if (!this.isActive(epoch)) throw this.stale()
      this.update({
        pendingEnvelope: pending.envelope,
        state: { kind: 'saving', stage: 'applying' },
      })
      if (this.cancelRequested) throw reviewWorkspaceError('cancelled', '已取消，输入仍保留', true)
      const reply = await this.port.applyCommand({ ...this.scope, envelope: pending.envelope })
      if (!this.isActive(epoch)) throw this.stale(reply.receipt)
      const inputChanged =
        pending.inputRevision !== null && pending.inputRevision !== this.inputRevision
      const nextInput =
        inputChanged &&
        pending.frozenInput !== null &&
        this.snapshot.editorInput.contextKey === pending.frozenInput.contextKey &&
        this.snapshot.editorInput.feedbackId === pending.frozenInput.feedbackId
          ? {
              ...this.snapshot.editorInput,
              feedbackId: pending.frozenInput.feedbackId ?? pending.envelope.generated.feedbackId,
              targets: [],
              baseSnapshotId: reply.receipt.snapshot.snapshotId,
            }
          : emptyContinuousEditor()
      this.pending = null
      this.update({
        view: reply.view,
        state: continuousReviewState(reply.view),
        pendingEnvelope: null,
        lastReceipt: reply.receipt,
        editorInput: nextInput,
        error: null,
      })
    } catch (cause) {
      const error = normalizeReviewWorkspaceError(cause)
      if (!this.isActive(epoch)) throw this.stale(error.committedReceipt)
      pending.error = error
      this.update({
        error,
        lastReceipt: error.committedReceipt ?? this.snapshot.lastReceipt,
        state: this.failureState(error),
      })
      this.notify(error)
      throw error
    }
  }
  private failureState(error: ReviewWorkspaceError): ContinuousReviewSnapshot['state'] {
    return requiresReviewReconciliation(error)
      ? { kind: 'recovery_required', error }
      : { kind: 'save_failed', error }
  }

  private query = async <T>(
    key: string,
    operation: (scope: ReviewWorkspaceSessionRequest) => Promise<T>,
  ): Promise<T> => {
    if (!this.isActive()) throw this.stale()
    if (this.cancelling)
      return this.reject(reviewWorkspaceError('busy', '正在取消上一个操作', true))
    const epoch = this.epoch
    const version = this.viewVersion
    const sequence = (this.querySequences.get(key) ?? 0) + 1
    this.querySequences.set(key, sequence)
    const current = () =>
      this.isActive(epoch) &&
      version === this.viewVersion &&
      this.querySequences.get(key) === sequence
    try {
      const result = await operation(this.scope)
      if (!this.isActive(epoch)) throw this.stale()
      if (!current()) throw reviewWorkspaceError('cancelled', '读取已失效，请重新获取', true)
      if (this.pending === null) this.update({ error: null })
      return result
    } catch (cause) {
      if (!this.isActive(epoch)) throw this.stale()
      const error = normalizeReviewWorkspaceError(cause)
      if (current()) {
        this.update({ error })
        this.notify(error)
      }
      throw error
    }
  }
}
