import type { KeyboardEvent, ReactNode } from 'react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  ReviewState,
} from '../api/types'
import { isPreviewableImage } from '../fileKinds'
import type { CompareAction, CompareState, PaneMetrics } from '../state/compareModel'
import { createCompareState, reconcileComparePanes, reduceCompare } from '../state/compareModel'
import { compareValidationMessage } from '../state/comparePolicy'
import ComparePane from './ComparePane'
import CompareVirtualViewport from './CompareVirtualViewport'
import {
  type CompareOriginalRequestScheduler,
  createCompareOriginalRequestScheduler,
} from './compareOriginalRequestScheduler'
import ViewerButton, { ViewerIconButton } from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'
import ViewerSegmentedControl from './ui/ViewerSegmentedControl'
import ViewerToolbar from './ui/ViewerToolbar'
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
    signal?: AbortSignal,
  ) => Promise<ImageRepresentation>
  onEntityIdsChange: (entityIds: string[]) => void
  onSetReview: (entityId: string, review: ReviewState | null) => void
  onToggleFavorite: (entityId: string) => void
  onStatus: (message: string) => void
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
  const [actualSizeEntityId, setActualSizeEntityId] = useState<string | null>(null)
  const [recoveredDimensions, setRecoveredDimensions] = useState<
    Record<string, RecoveredCompareDimensions | undefined>
  >({})
  const workspaceRef = useRef<HTMLDivElement>(null)
  const originalScheduler = useRef<CompareOriginalRequestScheduler<ImageRepresentation> | null>(
    null,
  )
  const originalSchedulerLifecycle = useRef(0)
  if (originalScheduler.current === null) {
    originalScheduler.current = createCompareOriginalRequestScheduler<ImageRepresentation>(2)
  }
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
        return requestImage(file, representation, signal)
      }
      return (
        originalScheduler.current?.enqueue(
          compareSourceRevision(file),
          () => requestImage(file, representation, signal),
          signal,
        ) ?? Promise.reject({ code: 'image_request_cancelled' as const })
      )
    },
    [requestImage],
  )

  useEffect(() => {
    const scheduler = originalScheduler.current
    originalSchedulerLifecycle.current += 1
    const lifecycle = originalSchedulerLifecycle.current
    return () => {
      queueMicrotask(() => {
        if (originalSchedulerLifecycle.current === lifecycle) scheduler?.dispose()
      })
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
      if (!liveIds.includes(actualSizeEntityId ?? '')) setActualSizeEntityId(null)
    } else {
      onEntityIdsChange(liveIds)
    }
  }, [actualSizeEntityId, files, model, onEntityIdsChange, presentKey])

  if (model === null) {
    return (
      <section className="compare-workspace compare-invalid" aria-label="图片对比">
        <ViewerLocalFeedback
          tone="danger"
          title="无法开始图片对比"
          action={
            <ViewerButton tone="secondary" onClick={() => onEntityIdsChange([])}>
              返回文件网格
            </ViewerButton>
          }
        >
          {compareValidationMessage('invalid_cardinality')}
        </ViewerLocalFeedback>
      </section>
    )
  }

  const compareModel = model
  const activeEntityId = compareModel.activeEntityId
  const activeFile = filesById.get(activeEntityId)
  const transformsDisabled = activeFile === undefined || !isPreviewableImage(activeFile)

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
      return actualSizeEntityId === entityId
        ? reduceCompare(measured, { type: 'actual_size', entityId })
        : measured
    })
  }

  function remove(entityId: string) {
    if (model === null) return
    const survivors = model.entityIds.filter((candidate) => candidate !== entityId)
    const transition = reconcileComparePanes(model, survivors)
    if (actualSizeEntityId === entityId) setActualSizeEntityId(null)
    if (transition.kind === 'compare') setModel(transition.state)
    onEntityIdsChange(survivors)
  }

  function fitView() {
    if (transformsDisabled) return
    setActualSizeEntityId(null)
    update({ type: 'fit', entityId: activeEntityId })
  }

  function actualSize() {
    if (transformsDisabled) return
    setActualSizeEntityId(activeEntityId)
    update({ type: 'actual_size', entityId: activeEntityId })
  }

  function keyboard(event: KeyboardEvent<HTMLElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      onEntityIdsChange([])
    } else if (!transformsDisabled && (event.key === '+' || event.key === '=')) {
      event.preventDefault()
      update({ type: 'zoom', entityId: activeEntityId, factor: 1.25 })
    } else if (!transformsDisabled && event.key === '-') {
      event.preventDefault()
      update({ type: 'zoom', entityId: activeEntityId, factor: 0.8 })
    } else if (!transformsDisabled && event.key === '0') {
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
        onOriginalUnavailable={(_targetId, reason) => {
          onStatus(
            reason === 'budget'
              ? '原图超出安全预览限制，已继续使用适窗代理。'
              : '无法加载原图，已继续使用适窗代理。',
          )
        }}
      />
    )
  }

  const toolbarLeading: ReactNode = (
    <>
      <span>{model.entityIds.length} 张图片</span>
      <strong title={activeFile?.name}>{activeFile?.name ?? '当前图片不可用'}</strong>
    </>
  )
  const toolbarTransforms: ReactNode = (
    <ViewerSegmentedControl label="对比显示控制">
      <ViewerButton disabled={transformsDisabled} onClick={fitView}>
        适应窗口
      </ViewerButton>
      <ViewerButton disabled={transformsDisabled} onClick={actualSize}>
        100%
      </ViewerButton>
      <ViewerIconButton
        icon="minus"
        label="缩小当前对比"
        disabled={transformsDisabled}
        onClick={() => update({ type: 'zoom', entityId: activeEntityId, factor: 0.8 })}
      />
      <ViewerIconButton
        icon="plus"
        label="放大当前对比"
        disabled={transformsDisabled}
        onClick={() => update({ type: 'zoom', entityId: activeEntityId, factor: 1.25 })}
      />
      <ViewerIconButton
        icon="rotate-cw"
        label="顺时针旋转当前图片"
        disabled={transformsDisabled}
        onClick={() => update({ type: 'rotate_clockwise', entityId: activeEntityId })}
      />
    </ViewerSegmentedControl>
  )
  const toolbarActions: ReactNode = (
    <>
      <ViewerButton
        tone="quiet"
        active={model.mode === 'synchronized'}
        aria-label={model.mode === 'synchronized' ? '切换为独立变换' : '切换为同步变换'}
        onClick={() =>
          update({
            type: 'mode_changed',
            mode: model.mode === 'synchronized' ? 'independent' : 'synchronized',
          })
        }
      >
        {model.mode === 'synchronized' ? '同步' : '独立'}
      </ViewerButton>
      <ViewerButton
        tone="quiet"
        className="preview-complete-action"
        aria-label="完成对比"
        onClick={() => onEntityIdsChange([])}
      >
        完成
      </ViewerButton>
    </>
  )

  return (
    <section
      ref={workspaceRef}
      className="compare-workspace"
      aria-label="图片对比"
      data-layout={plan.kind}
      tabIndex={0}
      onKeyDown={keyboard}
    >
      <ViewerToolbar
        label="对比工具"
        leading={toolbarLeading}
        center={toolbarTransforms}
        actions={toolbarActions}
      />
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
