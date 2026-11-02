import type {
  PreparedReviewAsset,
  ReviewAssetVersion,
  ReviewTargetEdit,
  ReviewTargetVersionKey,
} from '../../api/reviewWorkspaceTypes'
import type {
  ReviewAnchor,
  ReviewAnnotationStyle,
  ReviewScopeRequest,
  ReviewSessionSnapshot,
} from '../../api/types'
import {
  type ContinuousReviewEditorSeed,
  reviewWorkspaceError,
  sameReviewValue,
} from './continuousReviewModel'
import {
  type ImageReviewReadOnlyReason,
  imageFeedbackForEntity,
  imageReviewReadOnlyReason,
} from './reviewModel'
import type { ContinuousReviewCoordinator } from './useContinuousReviewCoordinator'
import type { ReviewSessionCoordinator } from './useReviewSessionCoordinator'

export interface ImageReviewWorkbenchFeedback {
  itemId: string
  feedbackId: string
  targetKey: ReviewTargetVersionKey | null
  assetVersionId: string
  text: string
  createdAtMs: number
  ordinal: number | null
  anchor: ReviewAnchor
  style?: ReviewAnnotationStyle | null
}

export interface ImageReviewPreparation {
  key: string
  prepared: PreparedReviewAsset | null
}

export interface ImageReviewWorkbenchView {
  feedback: ImageReviewWorkbenchFeedback[]
  readOnlyReason: ImageReviewReadOnlyReason
  restorableItemId: string | null
  statusMessage: string | null
}

export interface ImageReviewSaveRequest {
  entityId: string
  clientMutationId?: string
  item: ImageReviewWorkbenchFeedback | null
  operation: 'text' | 'geometry'
  text: string
  anchor: ReviewAnchor
  style?: ReviewAnnotationStyle | null
  preparation: ImageReviewPreparation
}

export interface ImageReviewDeleteRequest {
  entityId: string
  item: ImageReviewWorkbenchFeedback
  preparation: ImageReviewPreparation
}

export interface ImageReviewRestoreRequest {
  entityId: string
  itemId: string
  preparation: ImageReviewPreparation | null
}

export interface ImageReviewWorkbenchAdapter {
  protocol: 'legacy' | 'continuous'
  preparationKey(entityId: string): string
  viewKey(entityId: string): string
  prepareEntity(entityId: string): Promise<ImageReviewPreparation>
  view(entityId: string, preparation: ImageReviewPreparation | null): ImageReviewWorkbenchView
  saveFeedback(request: ImageReviewSaveRequest): Promise<ImageReviewWorkbenchView>
  deleteFeedback(request: ImageReviewDeleteRequest): Promise<ImageReviewWorkbenchView>
  restoreDeletedFeedback(request: ImageReviewRestoreRequest): Promise<ImageReviewWorkbenchView>
  discardPendingInput(): boolean
}

export function continuousImageReviewWorkbenchAdapter(
  coordinator: ContinuousReviewCoordinator,
): ImageReviewWorkbenchAdapter {
  const operationStatus = new Map<string, 'saved' | 'deleted'>()
  function view(entityId: string, preparation: ImageReviewPreparation | null) {
    const snapshot = coordinator.getSnapshot()
    const asset = preparedAsset(entityId, preparation)
    const equivalentIds = equivalentAssetVersionIds(snapshot, asset?.asset ?? null)
    const status =
      asset === null ? undefined : operationStatus.get(statusMessageKey(entityId, asset.asset.id))
    const savedHere = snapshot.lastChangedAssetVersionIds.some((id) => equivalentIds.has(id))
    return {
      feedback: continuousFeedback(snapshot, asset?.asset ?? null),
      readOnlyReason: continuousReadOnlyReason(snapshot, entityId, preparation),
      restorableItemId: null,
      statusMessage:
        status === 'saved' || savedHere
          ? pendingTargetForAsset(snapshot, equivalentIds)
            ? '已保存，待确认'
            : publicationMessage(snapshot)
          : status === 'deleted'
            ? publicationMessage(snapshot, '已删除，可供外部读取')
            : null,
    }
  }
  return {
    protocol: 'continuous',
    preparationKey: (entityId) => `continuous:${coordinator.workbenchSessionKey}:${entityId}`,
    viewKey: (entityId) => continuousViewKey(coordinator, entityId),
    async prepareEntity(entityId) {
      const assets = await coordinator.prepareAssets([entityId])
      const prepared = assets[0]
      return {
        key: `continuous:${coordinator.workbenchSessionKey}:${entityId}`,
        // Reuse may return a historical version whose stored source identity
        // predates the current scan. The requested preview entity is the
        // verified current owner, so bind that version to it.
        prepared:
          prepared === undefined
            ? null
            : prepared.asset.sourceEntityId === entityId
              ? prepared
              : { ...prepared, asset: { ...prepared.asset, sourceEntityId: entityId } },
      }
    },
    view,
    async saveFeedback(request) {
      const asset = preparedAsset(request.entityId, request.preparation)
      if (asset === null)
        throw reviewWorkspaceError('preview_required', '请重新加载并确认当前素材版本', false)
      const blocked = continuousReadOnlyReason(
        coordinator.getSnapshot(),
        request.entityId,
        request.preparation,
      )
      if (blocked !== null) throw continuousReadOnlyError(blocked)
      if (
        request.item !== null &&
        !equivalentAssetVersionIds(coordinator.getSnapshot(), asset.asset).has(
          request.item.assetVersionId,
        )
      )
        throw reviewWorkspaceError('wrong_context', '意见不属于当前素材版本', false)
      if (
        request.item !== null &&
        !continuousItemIsCurrent(coordinator.getSnapshot(), request.item)
      )
        throw reviewWorkspaceError('needs_confirmation', '意见已由其他更新改变，请重新选择', false)
      const seed = continuousEditorSeed(request, asset.asset.id)
      beginOrReuseContinuousEditor(coordinator, seed)
      await coordinator.saveFeedback()
      operationStatus.set(statusMessageKey(request.entityId, asset.asset.id), 'saved')
      return view(request.entityId, request.preparation)
    },
    async deleteFeedback(request) {
      const asset = preparedAsset(request.entityId, request.preparation)
      if (
        asset === null ||
        !equivalentAssetVersionIds(coordinator.getSnapshot(), asset.asset).has(
          request.item.assetVersionId,
        )
      )
        throw reviewWorkspaceError('wrong_context', '意见不属于当前素材版本', false)
      const blocked = continuousReadOnlyReason(
        coordinator.getSnapshot(),
        request.entityId,
        request.preparation,
      )
      if (blocked !== null) throw continuousReadOnlyError(blocked)
      if (
        request.item.targetKey === null ||
        !continuousItemIsCurrent(coordinator.getSnapshot(), request.item)
      )
        throw reviewWorkspaceError('wrong_context', '意见缺少可删除的目标版本', false)
      await coordinator.withdrawTargets([structuredClone(request.item.targetKey)])
      operationStatus.set(statusMessageKey(request.entityId, asset.asset.id), 'deleted')
      return view(request.entityId, request.preparation)
    },
    async restoreDeletedFeedback() {
      throw reviewWorkspaceError('capability_unavailable', '持续评审恢复需要从历史记录执行', false)
    },
    discardPendingInput() {
      return (
        coordinator.getSnapshot().editorInput.contextKey === null || coordinator.discardEditor()
      )
    },
  }
}

function statusMessageKey(entityId: string, assetVersionId: string) {
  return `${entityId}\u0000${assetVersionId}`
}

function continuousItemIsCurrent(
  snapshot: ReturnType<ContinuousReviewCoordinator['getSnapshot']>,
  item: ImageReviewWorkbenchFeedback,
) {
  const asset =
    snapshot.view?.current?.authoring.state.assets.find(
      (candidate) => candidate.id === item.assetVersionId,
    ) ?? null
  return continuousFeedback(snapshot, asset).some(
    (candidate) =>
      candidate.itemId === item.itemId && sameReviewValue(candidate.targetKey, item.targetKey),
  )
}

function continuousFeedback(
  snapshot: ReturnType<ContinuousReviewCoordinator['getSnapshot']>,
  asset: ReviewAssetVersion | null,
) {
  const assetVersionIds = equivalentAssetVersionIds(snapshot, asset)
  if (assetVersionIds.size === 0) return []
  let ordinal = 0
  return [...(snapshot.view?.current?.authoring.state.feedback ?? [])]
    .sort(
      (left, right) =>
        left.createdAtMs - right.createdAtMs ||
        (left.id < right.id ? -1 : left.id > right.id ? 1 : 0),
    )
    .flatMap((item): ImageReviewWorkbenchFeedback[] =>
      item.targets.flatMap((target) => {
        if (!assetVersionIds.has(target.assetVersionId)) return []
        return [
          {
            itemId: target.id,
            feedbackId: item.id,
            targetKey: {
              feedbackId: item.id,
              textRevisionId: item.textRevisionId,
              targetId: target.id,
              targetRevisionId: target.revisionId,
            },
            assetVersionId: target.assetVersionId,
            text: item.text,
            createdAtMs: item.createdAtMs,
            ordinal: target.anchor.kind === 'asset' ? null : ++ordinal,
            anchor: cloneAnchor(target.anchor),
            style: target.style ?? null,
          },
        ]
      }),
    )
}

/**
 * Source identity can converge onto one live file more than once when a prior
 * scan captured the same bytes under a different entity generation. Those
 * versions share one current source, so their feedback belongs to the same
 * preview even though the target keys keep their original versions.
 */
function equivalentAssetVersionIds(
  snapshot: ReturnType<ContinuousReviewCoordinator['getSnapshot']>,
  asset: ReviewAssetVersion | null,
): ReadonlySet<string> {
  const ids = new Set<string>()
  if (asset === null) return ids
  ids.add(asset.id)
  if (asset.evidence.blake3 === null) return ids
  for (const candidate of snapshot.view?.current?.authoring.state.assets ?? []) {
    if (candidate.relativePath !== asset.relativePath) continue
    if (candidate.sourceEntityId !== asset.sourceEntityId) continue
    if (candidate.evidence.sizeBytes !== asset.evidence.sizeBytes) continue
    if (candidate.evidence.blake3 !== asset.evidence.blake3) continue
    if (candidate.media.kind !== asset.media.kind) continue
    ids.add(candidate.id)
  }
  return ids
}

function continuousEditorSeed(
  request: ImageReviewSaveRequest,
  assetVersionId: string,
): ContinuousReviewEditorSeed {
  let targets: ReviewTargetEdit[]
  if (request.item === null) {
    targets = [
      {
        kind: 'add',
        assetVersionId,
        anchor: cloneAnchor(request.anchor),
        style: request.style ?? null,
      },
    ]
  } else if (request.operation === 'text') {
    targets = []
  } else {
    if (request.item.targetKey === null)
      throw reviewWorkspaceError('wrong_context', '意见缺少可调整的目标版本', false)
    targets = [
      {
        kind: 'redraw',
        key: request.item.targetKey,
        assetVersionId: request.item.assetVersionId,
        anchor: cloneAnchor(request.anchor),
        style: request.style ?? null,
      },
    ]
  }
  return {
    contextKey:
      request.item?.itemId ??
      (request.clientMutationId === undefined
        ? `new:${request.entityId}`
        : `new:${request.entityId}:${request.clientMutationId}`),
    feedbackId: request.item?.feedbackId ?? null,
    text: request.text,
    targets,
  }
}

function beginOrReuseContinuousEditor(
  coordinator: ContinuousReviewCoordinator,
  seed: ContinuousReviewEditorSeed,
) {
  if (coordinator.beginEditor(seed)) return
  const retained = coordinator.getSnapshot().editorInput
  if (retained.contextKey !== seed.contextKey || retained.feedbackId !== seed.feedbackId)
    throw reviewWorkspaceError('needs_confirmation', '请先处理当前未保存输入', false)
  if (retained.text !== seed.text) coordinator.setEditorText(seed.text)
  if (
    !sameReviewValue(retained.targets, seed.targets) &&
    !coordinator.setEditorTargets(seed.targets)
  )
    throw reviewWorkspaceError('needs_confirmation', '请先处理当前未保存输入', false)
}

export function legacyImageReviewWorkbenchAdapter(
  coordinator: ReviewSessionCoordinator,
  scope: ReviewScopeRequest,
): ImageReviewWorkbenchAdapter {
  let snapshot = coordinator.snapshot
  let capture: Promise<void> | null = null
  const scopeKey = JSON.stringify(scope)

  function view(entityId: string): ImageReviewWorkbenchView {
    return {
      feedback: imageFeedbackForEntity(snapshot, entityId),
      readOnlyReason: imageReviewReadOnlyReason(snapshot, entityId),
      restorableItemId: snapshot.restorableFeedbackId,
      statusMessage: null,
    }
  }

  return {
    protocol: 'legacy',
    preparationKey: (entityId) => `legacy:${scopeKey}:${entityId}`,
    viewKey: (entityId) =>
      `legacy:${entityId}:${snapshot.phase}:${snapshot.revision}:${snapshot.restorableFeedbackId ?? ''}`,
    async prepareEntity(entityId) {
      if (snapshot.phase === 'idle') {
        capture ??= coordinator.captureStart(cloneScope(scope))
        await capture
      }
      return { key: `legacy:${scopeKey}:${entityId}`, prepared: null }
    },
    view: (entityId) => view(entityId),
    async saveFeedback(request) {
      await capture
      let next: ReviewSessionSnapshot | null
      if (request.item !== null) {
        next =
          request.operation === 'geometry'
            ? await coordinator.replaceAnchoredFeedbackAnchor(
                request.item.feedbackId,
                request.entityId,
                cloneAnchor(request.anchor),
              )
            : await coordinator.updateAnchoredFeedbackText(request.item.feedbackId, request.text)
      } else if (snapshot.phase === 'active') {
        next = await coordinator.addAnchoredFeedback({
          entityId: request.entityId,
          text: request.text,
          anchor: cloneAnchor(request.anchor),
        })
      } else {
        next = await coordinator.startWithFeedback({
          entityId: request.entityId,
          text: request.text,
          anchor: cloneAnchor(request.anchor),
        })
      }
      if (next !== null) snapshot = next
      return view(request.entityId)
    },
    async deleteFeedback(request) {
      const next = await coordinator.deleteFeedback(request.item.feedbackId)
      if (next !== null) snapshot = next
      return view(request.entityId)
    },
    async restoreDeletedFeedback(request) {
      const next = await coordinator.restoreDeletedFeedback(request.itemId)
      if (next !== null) snapshot = next
      return view(request.entityId)
    },
    discardPendingInput: () => true,
  }
}

function preparedAsset(entityId: string, preparation: ImageReviewPreparation | null) {
  const candidate = preparation?.prepared ?? null
  return candidate?.asset.sourceEntityId === entityId &&
    candidate.preview?.assetVersionId === candidate.asset.id
    ? candidate
    : null
}

function continuousReadOnlyReason(
  snapshot: ReturnType<ContinuousReviewCoordinator['getSnapshot']>,
  entityId: string,
  preparation: ImageReviewPreparation | null,
): ImageReviewReadOnlyReason {
  switch (snapshot.state.kind) {
    case 'loading':
      return 'loading'
    case 'saving_authoring':
      return 'saving'
    case 'saved_pending_publication':
    case 'publication_blocked':
      break
    case 'recovery_required':
      return 'recovery_required'
    case 'migration_required':
      return 'migration_required'
    case 'unavailable':
      return sourceConfirmationError(snapshot.state.error.code)
        ? 'source_confirmation'
        : 'write_unavailable'
    case 'save_failed':
      if (sourceConfirmationError(snapshot.state.error.code)) return 'source_confirmation'
      break
    case 'ready':
      break
  }
  if (!snapshot.view?.capabilities.continuousEditing) return 'write_unavailable'
  if (preparation === null) return 'loading'
  const asset = preparedAsset(entityId, preparation)
  if (asset === null) return 'source_confirmation'
  const equivalentIds = equivalentAssetVersionIds(snapshot, asset.asset)
  const current = snapshot.view.current?.authoring.state
  const assetVersionsForEntity = new Set(
    current?.assets
      .filter((candidate) => candidate.sourceEntityId === entityId)
      .map((candidate) => candidate.id) ?? [],
  )
  const referencedAssetVersionsForEntity = new Set(
    current?.feedback.flatMap((feedback) =>
      feedback.targets
        .map((target) => target.assetVersionId)
        .filter((assetVersionId) => assetVersionsForEntity.has(assetVersionId)),
    ) ?? [],
  )
  if (
    [...referencedAssetVersionsForEntity].some(
      (assetVersionId) => !equivalentIds.has(assetVersionId),
    )
  )
    return 'source_confirmation'
  if (
    snapshot.view.sourceChecks.some(
      (check) => equivalentIds.has(check.assetVersionId) && check.status !== 'match',
    ) ||
    snapshot.view.current?.authoring.state.feedback.some((feedback) =>
      feedback.targets.some(
        (target) =>
          equivalentIds.has(target.assetVersionId) &&
          target.availability.kind === 'needs_confirmation',
      ),
    )
  )
    return 'source_confirmation'
  return null
}

function sourceConfirmationError(code: string) {
  return [
    'asset_unavailable',
    'source_changed',
    'unsafe_source',
    'render_failed',
    'preview_required',
  ].includes(code)
}

function continuousReadOnlyError(reason: Exclude<ImageReviewReadOnlyReason, null>) {
  switch (reason) {
    case 'loading':
    case 'saving':
      return reviewWorkspaceError('busy', '请等待当前评审操作完成', true)
    case 'source_confirmation':
      return reviewWorkspaceError('needs_confirmation', '素材来源或版本需要先确认', false)
    case 'migration_required':
      return reviewWorkspaceError('migration_required', '需要先确认迁移', false)
    case 'recovery_required':
      return reviewWorkspaceError('needs_confirmation', '需要先处理恢复输入', false)
    case 'outside_scope':
    case 'write_unavailable':
      return reviewWorkspaceError('read_only', '项目当前不可写', false)
  }
}

function continuousViewKey(coordinator: ContinuousReviewCoordinator, entityId: string) {
  const snapshot = coordinator.getSnapshot()
  return JSON.stringify({
    entityId,
    state: snapshot.state.kind,
    snapshotId: snapshot.view?.current?.authoring.head.snapshotId ?? null,
    sourceChecks: snapshot.view?.sourceChecks ?? [],
    projection: snapshot.view?.projection ?? null,
    recovery: snapshot.view?.recovery.map((draft) => draft.commandId) ?? [],
    migration: snapshot.view?.migration?.inspectionDigest ?? null,
    error: snapshot.error?.code ?? null,
  })
}

function pendingTargetForAsset(
  snapshot: ReturnType<ContinuousReviewCoordinator['getSnapshot']>,
  assetVersionIds: ReadonlySet<string>,
) {
  if (assetVersionIds.size === 0) return false
  const pending = new Set(snapshot.view?.projection.needsConfirmation ?? [])
  return (
    snapshot.view?.current?.authoring.state.feedback.some((feedback) =>
      feedback.targets.some(
        (target) => assetVersionIds.has(target.assetVersionId) && pending.has(target.id),
      ),
    ) ?? false
  )
}

function publicationMessage(
  snapshot: ReturnType<ContinuousReviewCoordinator['getSnapshot']>,
  readyMessage = '已保存，可供外部读取',
) {
  switch (snapshot.state.kind) {
    case 'saving_authoring':
      return '正在保存意见，请稍候。'
    case 'saved_pending_publication':
      return '已保存，Agent 数据生成中'
    case 'publication_blocked':
      return '评审已保存，Agent 数据生成失败'
    default:
      return readyMessage
  }
}

function cloneScope(scope: ReviewScopeRequest): ReviewScopeRequest {
  return scope.kind === 'selection'
    ? { kind: 'selection', entityIds: [...scope.entityIds] }
    : { ...scope }
}

function cloneAnchor(anchor: ReviewAnchor): ReviewAnchor {
  switch (anchor.kind) {
    case 'asset':
      return { kind: 'asset' }
    case 'image_point':
    case 'image_text':
      return { ...anchor }
    case 'image_arrow':
    case 'image_line':
      return { kind: anchor.kind, tail: { ...anchor.tail }, head: { ...anchor.head } }
    case 'image_stroke':
    case 'image_polygon':
      return { kind: anchor.kind, points: anchor.points.map((point) => ({ ...point })) }
    case 'image_rect':
    case 'image_ellipse':
    case 'video_point':
    case 'video_range':
      return { ...anchor }
    default:
      return assertNeverAnchor(anchor)
  }
}

function assertNeverAnchor(value: never): never {
  throw new Error(`Unsupported review anchor: ${JSON.stringify(value)}`)
}
