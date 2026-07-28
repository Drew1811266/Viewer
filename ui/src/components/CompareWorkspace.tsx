import type { KeyboardEvent } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  ReviewState,
} from '../api/types'
import type { CompareAction, CompareState, PaneMetrics } from '../state/compareModel'
import { createCompareState, reconcileComparePanes, reduceCompare } from '../state/compareModel'
import { compareValidationMessage } from '../state/comparePolicy'
import ComparePane from './ComparePane'
import CompareVirtualViewport from './CompareVirtualViewport'
import {
  compareSourceRevision,
  type RecoveredCompareDimensions,
  useCompareLayout,
} from './useCompareLayout'

interface CompareWorkspaceProps {
  files: BrowserFile[]
  readOnly: boolean
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
  ) => Promise<ImageRepresentation>
  onEntityIdsChange: (entityIds: string[]) => void
  onSetReview: (entityId: string, review: ReviewState | null) => void
  onToggleFavorite: (entityId: string) => void
  onStatus: (message: string) => void
}

interface OriginalRequestJob {
  run: () => Promise<ImageRepresentation>
  resolve: (image: ImageRepresentation) => void
  reject: (reason: unknown) => void
  signal?: AbortSignal
  removeAbortListener: () => void
}

interface OriginalRequestLane {
  running: OriginalRequestJob | null
  queued: OriginalRequestJob | null
  disposed: boolean
}

export default function CompareWorkspace({
  files,
  readOnly,
  requestImage,
  onEntityIdsChange,
  onSetReview,
  onToggleFavorite,
  onStatus,
}: CompareWorkspaceProps) {
  const [model, setModel] = useState<CompareState | null>(() => initialModel(files))
  const [originalEntityId, setOriginalEntityId] = useState<string | null>(null)
  const [recoveredDimensions, setRecoveredDimensions] = useState<
    Record<string, RecoveredCompareDimensions | undefined>
  >({})
  const workspaceRef = useRef<HTMLDivElement>(null)
  const originalLane = useRef<OriginalRequestLane>({
    running: null,
    queued: null,
    disposed: false,
  })
  const presentKey = files.map((file) => file.entityId).join('\u0000')
  const filesById = useMemo(() => new Map(files.map((file) => [file.entityId, file])), [files])
  const rotations = useMemo(
    () =>
      Object.fromEntries(
        (model?.entityIds ?? []).map((entityId) => [
          entityId,
          model?.transforms[entityId]?.rotation ?? 0,
        ]),
      ),
    [model],
  )
  const comparedFiles = useMemo(
    () =>
      model === null
        ? files
        : model.entityIds.flatMap((entityId) => {
            const file = filesById.get(entityId)
            return file === undefined ? [] : [file]
          }),
    [files, filesById, model],
  )
  const { containerRef, plan } = useCompareLayout({
    files: comparedFiles,
    rotations,
    recoveredDimensions,
  })

  const requestComparedImage = useCallback(
    (file: BrowserFile, representation: ImageRepresentationRequest, signal?: AbortSignal) => {
      if (representation.kind !== 'original100_percent') {
        return requestImage(file, representation)
      }
      return enqueueOriginalRequest(
        originalLane.current,
        () => requestImage(file, representation),
        signal,
      )
    },
    [requestImage],
  )

  useEffect(() => {
    const lane = originalLane.current
    lane.disposed = false
    return () => {
      lane.disposed = true
      if (lane.queued !== null) {
        cancelQueuedOriginal(lane.queued)
        lane.queued = null
      }
    }
  }, [])

  useEffect(() => {
    workspaceRef.current?.focus()
  }, [])

  useEffect(() => {
    if (model === null) return
    const presentIds = files.map((file) => file.entityId)
    const liveIds = model.entityIds.filter((entityId) => presentIds.includes(entityId))
    const liveSourceRevisions = new Set(files.map(compareSourceRevision))
    setRecoveredDimensions((current) => {
      const retained = Object.entries(current).filter(([sourceRevision]) =>
        liveSourceRevisions.has(sourceRevision),
      )
      return retained.length === Object.keys(current).length
        ? current
        : Object.fromEntries(retained)
    })
    if (sameIds(liveIds, model.entityIds)) return
    const transition = reconcileComparePanes(model, liveIds)
    if (transition.kind === 'compare') {
      setModel(transition.state)
      if (!liveIds.includes(originalEntityId ?? '')) setOriginalEntityId(null)
    } else {
      onEntityIdsChange(liveIds)
    }
  }, [files, model, onEntityIdsChange, originalEntityId, presentKey])

  if (model === null) {
    return (
      <section className="compare-workspace compare-invalid" aria-label="图片对比">
        <p role="alert">{compareValidationMessage('invalid_cardinality')}</p>
        <button type="button" onClick={() => onEntityIdsChange([])}>
          返回文件网格
        </button>
      </section>
    )
  }

  const compareModel = model
  const activeEntityId = compareModel.activeEntityId

  function update(action: CompareAction) {
    setModel((current) => (current === null ? current : reduceCompare(current, action)))
  }

  function updateMetrics(entityId: string, metrics: PaneMetrics) {
    const file = filesById.get(entityId)
    if (file !== undefined && metrics.imageWidth > 0 && metrics.imageHeight > 0) {
      const sourceRevision = compareSourceRevision(file)
      setRecoveredDimensions((current) => {
        const previous = current[sourceRevision]
        if (
          previous?.sourceRevision === sourceRevision &&
          previous.width === metrics.imageWidth &&
          previous.height === metrics.imageHeight
        ) {
          return current
        }
        return {
          ...current,
          [sourceRevision]: {
            sourceRevision,
            width: metrics.imageWidth,
            height: metrics.imageHeight,
          },
        }
      })
    }
    setModel((current) => {
      if (current === null) return current
      const measured = reduceCompare(current, {
        type: 'metrics_changed',
        entityId,
        metrics,
      })
      return originalEntityId === entityId
        ? reduceCompare(measured, { type: 'actual_size', entityId })
        : measured
    })
  }

  function remove(entityId: string) {
    if (model === null) return
    const survivors = model.entityIds.filter((candidate) => candidate !== entityId)
    const transition = reconcileComparePanes(model, survivors)
    if (originalEntityId === entityId) setOriginalEntityId(null)
    if (transition.kind === 'compare') setModel(transition.state)
    onEntityIdsChange(survivors)
  }

  function fitView() {
    setOriginalEntityId(null)
    update({ type: 'fit', entityId: activeEntityId })
  }

  function actualSize() {
    setOriginalEntityId(activeEntityId)
    update({ type: 'actual_size', entityId: activeEntityId })
  }

  function keyboard(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      onEntityIdsChange([])
    } else if (event.key === '+' || event.key === '=') {
      event.preventDefault()
      update({ type: 'zoom', entityId: activeEntityId, factor: 1.25 })
    } else if (event.key === '-') {
      event.preventDefault()
      update({ type: 'zoom', entityId: activeEntityId, factor: 0.8 })
    } else if (event.key === '0') {
      event.preventDefault()
      fitView()
    }
  }

  function renderPane(entityId: string) {
    const file = filesById.get(entityId)
    if (file === undefined) return null
    const transform = compareModel.transforms[entityId]
    if (transform === undefined) {
      if (compareModel.entityIds.includes(entityId)) {
        throw new Error(`Missing compare transform for entity ${entityId}`)
      }
      return null
    }
    return (
      <ComparePane
        file={file}
        transform={transform}
        active={activeEntityId === entityId}
        useOriginal={originalEntityId === entityId}
        readOnly={readOnly}
        requestImage={requestComparedImage}
        onActivate={() => update({ type: 'active_changed', entityId })}
        onMetrics={updateMetrics}
        onPan={(targetId, deltaX, deltaY) =>
          update({ type: 'pan', entityId: targetId, deltaX, deltaY })
        }
        onRemove={remove}
        onSetReview={onSetReview}
        onToggleFavorite={onToggleFavorite}
        onOriginalUnavailable={(targetId, reason) => {
          if (originalEntityId === targetId) setOriginalEntityId(null)
          update({ type: 'fit', entityId: targetId })
          onStatus(
            reason === 'budget'
              ? '原图超出安全预览限制，已继续使用适窗代理。'
              : '无法加载原图，已继续使用适窗代理。',
          )
        }}
      />
    )
  }

  return (
    <section
      ref={workspaceRef}
      className="compare-workspace"
      aria-label="图片对比"
      data-layout={plan.kind}
      tabIndex={0}
      onKeyDown={keyboard}
    >
      <div className="compare-toolbar" aria-label="对比工具">
        <span>{model.entityIds.length} 张图片</span>
        <button type="button" onClick={fitView}>
          适应窗口
        </button>
        <button type="button" onClick={actualSize}>
          100%
        </button>
        <button
          type="button"
          aria-label="缩小当前对比"
          onClick={() => update({ type: 'zoom', entityId: activeEntityId, factor: 0.8 })}
        >
          −
        </button>
        <button
          type="button"
          aria-label="放大当前对比"
          onClick={() => update({ type: 'zoom', entityId: activeEntityId, factor: 1.25 })}
        >
          +
        </button>
        <button
          type="button"
          aria-label="顺时针旋转当前图片"
          onClick={() => update({ type: 'rotate_clockwise', entityId: activeEntityId })}
        >
          ↻
        </button>
        <button
          type="button"
          aria-label={model.mode === 'synchronized' ? '切换为独立变换' : '切换为同步变换'}
          aria-pressed={model.mode === 'synchronized'}
          onClick={() =>
            update({
              type: 'mode_changed',
              mode: model.mode === 'synchronized' ? 'independent' : 'synchronized',
            })
          }
        >
          {model.mode === 'synchronized' ? '同步' : '独立'}
        </button>
        <button type="button" aria-label="关闭对比" onClick={() => onEntityIdsChange([])}>
          完成
        </button>
      </div>
      <div ref={containerRef} className="compare-layout-region">
        {plan.scrollAxis === 'none' ? (
          <div
            role="list"
            aria-label="全部图片对比"
            className="compare-fit-layout"
            data-testid="compare-fit-layout"
          >
            {plan.rects.map((rect, index) => (
              <div
                key={rect.entityId}
                role="listitem"
                aria-posinset={index + 1}
                aria-setsize={plan.rects.length}
                data-compare-entity-id={rect.entityId}
                className="compare-layout-item"
                style={{
                  left: rect.left,
                  top: rect.top,
                  width: rect.width,
                  height: rect.height,
                }}
              >
                {renderPane(rect.entityId)}
              </div>
            ))}
          </div>
        ) : (
          <CompareVirtualViewport
            plan={plan}
            activeEntityId={activeEntityId}
            onActivate={(entityId) => update({ type: 'active_changed', entityId })}
            renderItem={(entityId) => renderPane(entityId)}
          />
        )}
      </div>
    </section>
  )
}

function initialModel(files: BrowserFile[]): CompareState | null {
  const result = createCompareState(files)
  return result.ok ? result.state : null
}

function sameIds(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((id, index) => id === right[index])
}

function enqueueOriginalRequest(
  lane: OriginalRequestLane,
  run: () => Promise<ImageRepresentation>,
  signal?: AbortSignal,
): Promise<ImageRepresentation> {
  if (lane.disposed || signal?.aborted) {
    return Promise.reject(cancelledImageRequest())
  }
  return new Promise<ImageRepresentation>((resolve, reject) => {
    let job!: OriginalRequestJob
    const abort = () => {
      if (lane.queued !== job) return
      lane.queued = null
      cancelQueuedOriginal(job)
    }
    job = {
      run,
      resolve,
      reject,
      signal,
      removeAbortListener: () => signal?.removeEventListener('abort', abort),
    }
    signal?.addEventListener('abort', abort, { once: true })
    if (lane.queued !== null) cancelQueuedOriginal(lane.queued)
    lane.queued = job
    pumpOriginalLane(lane)
  })
}

function pumpOriginalLane(lane: OriginalRequestLane): void {
  if (lane.disposed || lane.running !== null || lane.queued === null) return
  const job = lane.queued
  lane.queued = null
  lane.running = job
  void job
    .run()
    .then(
      (image) => {
        if (job.signal?.aborted) job.reject(cancelledImageRequest())
        else job.resolve(image)
      },
      (reason: unknown) => {
        if (job.signal?.aborted) job.reject(cancelledImageRequest())
        else job.reject(reason)
      },
    )
    .finally(() => {
      job.removeAbortListener()
      if (lane.running === job) lane.running = null
      pumpOriginalLane(lane)
    })
}

function cancelQueuedOriginal(job: OriginalRequestJob): void {
  job.removeAbortListener()
  job.reject(cancelledImageRequest())
}

function cancelledImageRequest() {
  return { code: 'image_request_cancelled' }
}
