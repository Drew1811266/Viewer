import { type CompareValidationResult, validateCompareCandidates } from './comparePolicy'

export const MIN_COMPARE_SCALE = 0.1
export const MAX_COMPARE_SCALE = 8

export type CompareMode = 'synchronized' | 'independent'
export type QuarterRotation = 0 | 90 | 180 | 270

export interface CompareCandidate {
  entityId: string
  kind: string
}

export interface PaneMetrics {
  imageWidth: number
  imageHeight: number
  viewportWidth: number
  viewportHeight: number
}

export interface NormalizedTransform {
  scale: number
  centerX: number
  centerY: number
}

export interface PaneTransform extends NormalizedTransform {
  rotation: QuarterRotation
}

export interface CompareState {
  entityIds: string[]
  activeEntityId: string
  mode: CompareMode
  shared: NormalizedTransform
  transforms: Record<string, PaneTransform>
  metrics: Record<string, PaneMetrics | undefined>
}

export type CompareCreationResult =
  | { ok: true; state: CompareState }
  | Extract<CompareValidationResult, { ok: false }>

export type CompareTransition =
  | { kind: 'compare'; state: CompareState }
  | { kind: 'single_preview'; entityId: string }
  | { kind: 'grid' }

export type CompareAction =
  | { type: 'active_changed'; entityId: string }
  | { type: 'metrics_changed'; entityId: string; metrics: PaneMetrics }
  | { type: 'mode_changed'; mode: CompareMode }
  | { type: 'fit'; entityId: string }
  | { type: 'actual_size'; entityId: string }
  | { type: 'zoom'; entityId: string; factor: number }
  | { type: 'pan'; entityId: string; deltaX: number; deltaY: number }
  | { type: 'rotate_clockwise'; entityId: string }

const DEFAULT_NORMALIZED: NormalizedTransform = {
  scale: 1,
  centerX: 0.5,
  centerY: 0.5,
}

export function createCompareState(candidates: readonly CompareCandidate[]): CompareCreationResult {
  const validation = validateCompareCandidates(candidates)
  if (!validation.ok) return validation
  const entityIds = candidates.map((candidate) => candidate.entityId)
  const transforms = Object.fromEntries(
    entityIds.map((entityId) => [entityId, paneTransform(DEFAULT_NORMALIZED, 0)]),
  )
  const [activeEntityId] = entityIds
  if (activeEntityId === undefined) {
    throw new Error('Valid compare candidates must include an active entity')
  }
  return {
    ok: true,
    state: {
      entityIds,
      activeEntityId,
      mode: 'synchronized',
      shared: { ...DEFAULT_NORMALIZED },
      transforms,
      metrics: {},
    },
  }
}

export function reduceCompare(state: CompareState, action: CompareAction): CompareState {
  if (action.type === 'active_changed') {
    return state.entityIds.includes(action.entityId)
      ? { ...state, activeEntityId: action.entityId }
      : state
  }

  if (action.type === 'metrics_changed') {
    if (!state.entityIds.includes(action.entityId) || !validMetrics(action.metrics)) return state
    const metrics = { ...state.metrics, [action.entityId]: action.metrics }
    const current = transformFor(state, action.entityId)
    const normalized = state.mode === 'synchronized' ? state.shared : current
    return {
      ...state,
      metrics,
      transforms: {
        ...state.transforms,
        [action.entityId]: clampPane(paneTransform(normalized, current.rotation), action.metrics),
      },
    }
  }

  if (action.type === 'mode_changed') {
    if (action.mode === state.mode) return state
    if (action.mode === 'independent') {
      return {
        ...state,
        mode: action.mode,
        transforms: applySharedToEveryPane(state, state.shared),
      }
    }
    const active = transformFor(state, state.activeEntityId)
    const shared = normalized(active)
    return {
      ...state,
      mode: action.mode,
      shared,
      transforms: applySharedToEveryPane(state, shared),
    }
  }

  if (!state.entityIds.includes(action.entityId)) return state
  if (action.type === 'rotate_clockwise') {
    const current = transformFor(state, action.entityId)
    const rotation = ((current.rotation + 90) % 360) as QuarterRotation
    const source = state.mode === 'synchronized' ? state.shared : current
    return {
      ...state,
      activeEntityId: action.entityId,
      transforms: {
        ...state.transforms,
        [action.entityId]: clampPane(
          paneTransform(source, rotation),
          state.metrics[action.entityId],
        ),
      },
    }
  }

  const current =
    state.mode === 'synchronized' ? state.shared : transformFor(state, action.entityId)
  let next: NormalizedTransform
  switch (action.type) {
    case 'fit':
      next = { ...DEFAULT_NORMALIZED }
      break
    case 'actual_size':
      next = {
        scale: actualSizeScale(
          state.metrics[action.entityId],
          transformFor(state, action.entityId).rotation,
        ),
        centerX: 0.5,
        centerY: 0.5,
      }
      break
    case 'zoom':
      if (!Number.isFinite(action.factor) || action.factor <= 0) return state
      next = {
        ...current,
        scale: clamp(current.scale * action.factor, MIN_COMPARE_SCALE, MAX_COMPARE_SCALE),
      }
      break
    case 'pan':
      if (!Number.isFinite(action.deltaX) || !Number.isFinite(action.deltaY)) return state
      next = {
        ...current,
        centerX: clamp(current.centerX + action.deltaX, 0, 1),
        centerY: clamp(current.centerY + action.deltaY, 0, 1),
      }
      break
  }

  if (state.mode === 'synchronized') {
    return {
      ...state,
      activeEntityId: action.entityId,
      shared: next,
      transforms: applySharedToEveryPane(state, next),
    }
  }
  const currentPane = transformFor(state, action.entityId)
  return {
    ...state,
    activeEntityId: action.entityId,
    transforms: {
      ...state.transforms,
      [action.entityId]: clampPane(
        paneTransform(next, currentPane.rotation),
        state.metrics[action.entityId],
      ),
    },
  }
}

export function reconcileComparePanes(
  state: CompareState,
  presentEntityIds: readonly string[],
): CompareTransition {
  const present = new Set(presentEntityIds)
  const entityIds = state.entityIds.filter((entityId) => present.has(entityId))
  if (entityIds.length === 0) return { kind: 'grid' }
  const [firstEntityId] = entityIds
  if (firstEntityId === undefined) {
    throw new Error('Non-empty reconciled compare state must include an entity')
  }
  if (entityIds.length === 1) return { kind: 'single_preview', entityId: firstEntityId }
  const activeEntityId = entityIds.includes(state.activeEntityId)
    ? state.activeEntityId
    : nearestSurvivingEntityId(state, present, firstEntityId)
  const transforms = Object.fromEntries(
    entityIds.map((entityId) => [entityId, transformFor(state, entityId)]),
  )
  const metrics = Object.fromEntries(
    entityIds
      .filter((entityId) => state.metrics[entityId] !== undefined)
      .map((entityId) => [entityId, state.metrics[entityId]]),
  )
  return {
    kind: 'compare',
    state: {
      ...state,
      entityIds,
      activeEntityId,
      transforms,
      metrics,
    },
  }
}

function nearestSurvivingEntityId(
  state: CompareState,
  present: ReadonlySet<string>,
  fallback: string,
): string {
  const removedIndex = state.entityIds.indexOf(state.activeEntityId)
  if (removedIndex < 0) return fallback
  for (let distance = 1; distance < state.entityIds.length; distance += 1) {
    const following = state.entityIds[removedIndex + distance]
    if (following !== undefined && present.has(following)) return following
    const preceding = state.entityIds[removedIndex - distance]
    if (preceding !== undefined && present.has(preceding)) return preceding
  }
  return fallback
}

function applySharedToEveryPane(
  state: CompareState,
  shared: NormalizedTransform,
): Record<string, PaneTransform> {
  return Object.fromEntries(
    state.entityIds.map((entityId) => {
      const rotation = transformFor(state, entityId).rotation
      return [entityId, clampPane(paneTransform(shared, rotation), state.metrics[entityId])]
    }),
  )
}

function transformFor(state: CompareState, entityId: string): PaneTransform {
  const transform = state.transforms[entityId]
  if (transform === undefined) {
    throw new Error(`Missing compare transform for entity ${entityId}`)
  }
  return transform
}

function actualSizeScale(metrics: PaneMetrics | undefined, rotation: QuarterRotation): number {
  if (metrics === undefined) return 1
  const [width, height] = rotatedDimensions(metrics, rotation)
  const fitScale = Math.min(1, metrics.viewportWidth / width, metrics.viewportHeight / height)
  if (!Number.isFinite(fitScale) || fitScale <= 0) return 1
  return clamp(1 / fitScale, MIN_COMPARE_SCALE, MAX_COMPARE_SCALE)
}

function clampPane(transform: PaneTransform, metrics: PaneMetrics | undefined): PaneTransform {
  const scale = clamp(transform.scale, MIN_COMPARE_SCALE, MAX_COMPARE_SCALE)
  if (metrics === undefined || !validMetrics(metrics)) {
    return {
      ...transform,
      scale,
      centerX: clamp(transform.centerX, 0, 1),
      centerY: clamp(transform.centerY, 0, 1),
    }
  }
  const [width, height] = rotatedDimensions(metrics, transform.rotation)
  const fitScale = Math.min(1, metrics.viewportWidth / width, metrics.viewportHeight / height)
  const displayedWidth = width * fitScale * scale
  const displayedHeight = height * fitScale * scale
  const halfVisibleX = Math.min(0.5, metrics.viewportWidth / (2 * displayedWidth))
  const halfVisibleY = Math.min(0.5, metrics.viewportHeight / (2 * displayedHeight))
  return {
    ...transform,
    scale,
    centerX: clamp(transform.centerX, halfVisibleX, 1 - halfVisibleX),
    centerY: clamp(transform.centerY, halfVisibleY, 1 - halfVisibleY),
  }
}

function rotatedDimensions(metrics: PaneMetrics, rotation: QuarterRotation): [number, number] {
  return rotation === 90 || rotation === 270
    ? [metrics.imageHeight, metrics.imageWidth]
    : [metrics.imageWidth, metrics.imageHeight]
}

function validMetrics(metrics: PaneMetrics): boolean {
  return (
    Number.isFinite(metrics.imageWidth) &&
    Number.isFinite(metrics.imageHeight) &&
    Number.isFinite(metrics.viewportWidth) &&
    Number.isFinite(metrics.viewportHeight) &&
    metrics.imageWidth > 0 &&
    metrics.imageHeight > 0 &&
    metrics.viewportWidth > 0 &&
    metrics.viewportHeight > 0
  )
}

function normalized(transform: PaneTransform): NormalizedTransform {
  return {
    scale: transform.scale,
    centerX: transform.centerX,
    centerY: transform.centerY,
  }
}

function paneTransform(transform: NormalizedTransform, rotation: QuarterRotation): PaneTransform {
  return { ...transform, rotation }
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}
