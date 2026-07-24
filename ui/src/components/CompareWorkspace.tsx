import type { KeyboardEvent } from 'react'
import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  ReviewState,
} from '../api/types'
import type { CompareAction, CompareState, PaneMetrics } from '../state/compareModel'
import {
  compareLayout,
  createCompareState,
  reconcileComparePanes,
  reduceCompare,
} from '../state/compareModel'
import ComparePane from './ComparePane'

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
  const workspaceRef = useRef<HTMLDivElement>(null)
  const originalLane = useRef<OriginalRequestLane>({
    running: null,
    queued: null,
    disposed: false,
  })
  const presentKey = files.map((file) => file.entityId).join('\u0000')

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
        <p role="alert">请选择 2–4 张 JPG 或 PNG 图片进行对比。</p>
        <button type="button" onClick={() => onEntityIdsChange([])}>
          返回文件网格
        </button>
      </section>
    )
  }

  const layout = compareLayout(model)
  const activeEntityId = model.activeEntityId

  function update(action: CompareAction) {
    setModel((current) => (current === null ? current : reduceCompare(current, action)))
  }

  function updateMetrics(entityId: string, metrics: PaneMetrics) {
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

  return (
    <section
      ref={workspaceRef}
      className={`compare-workspace compare-layout-${layout}`}
      aria-label="图片对比"
      data-layout={layout}
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
      <div className="compare-pane-grid">
        {model.entityIds.map((entityId) => {
          const file = files.find((candidate) => candidate.entityId === entityId)
          if (file === undefined) return null
          const transform = model.transforms[entityId]
          if (transform === undefined) {
            throw new Error(`Missing compare transform for entity ${entityId}`)
          }
          return (
            <ComparePane
              key={entityId}
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
        })}
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
