import type { ReactNode } from 'react'
import { useCallback, useLayoutEffect, useMemo, useRef } from 'react'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type { ImageRendererViewportBinding } from '../../rendering/imageRendererTypes'
import ImagePreviewSurface, {
  type ImagePreviewSurfaceProps,
} from '../imagePreview/ImagePreviewSurface'
import type { ImagePreviewProjection } from '../imagePreview/imagePreviewProjection'
import ViewerButton from '../ui/ViewerButton'
import AnnotationCanvas from './AnnotationCanvas'
import AnnotationToolbar from './AnnotationToolbar'
import { buildAnnotationScene, createAnnotationMagnifierPainter } from './annotationScene'
import InlineFeedbackEditor from './InlineFeedbackEditor'
import NativeReviewViewportBridge from './NativeReviewViewportBridge'
import ReviewFeedbackRail from './ReviewFeedbackRail'

export interface ImageReviewWorkspaceProps
  extends Omit<ImagePreviewSurfaceProps, 'slots' | 'onNavigate'> {
  controller: ImageReviewWorkbenchController
  onArchive?: () => void
  onHistory?: () => void
}

export default function ImageReviewWorkspace({
  controller,
  onArchive,
  onHistory,
  ...surfaceProps
}: ImageReviewWorkspaceProps) {
  const compactDefaultEntity = useRef<string | null>(null)
  const identity = useRef<HTMLElement | null>(null)
  const preparedImageRef = useRef(controller.preparedImage)
  preparedImageRef.current = controller.preparedImage
  const editorPhase = controller.editor.phase
  const transientAnchor = editorPhase.status === 'idle' ? null : editorPhase.draftAnchor
  const annotationScene = useMemo(
    () =>
      buildAnnotationScene({
        feedback: controller.feedback,
        selectedItemId: controller.selectedItemId,
        transientAnchor,
      }),
    [controller.feedback, controller.selectedItemId, transientAnchor],
  )
  const magnifierOverlayPainter = useMemo(
    () => createAnnotationMagnifierPainter(annotationScene),
    [annotationScene],
  )

  useLayoutEffect(() => {
    if (compactDefaultEntity.current === surfaceProps.file.entityId) return
    compactDefaultEntity.current = surfaceProps.file.entityId
    const width = identity.current?.closest<HTMLElement>('.image-preview')?.clientWidth ?? 0
    if ((width > 0 && width <= 700) || window.matchMedia?.('(max-width: 700px)').matches) {
      controller.setRailOpen(false)
    }
  }, [controller, surfaceProps.file.entityId])

  const metadata = surfaceProps.file.imageMetadata
  const preparedAssetVersionId = controller.preparedImage?.assetVersionId ?? null
  const requestContinuousImage = useCallback<ImagePreviewSurfaceProps['requestImage']>(
    async (file, representation, signal) => {
      signal?.throwIfAborted()
      const prepared = preparedImageRef.current
      if (
        prepared === null ||
        prepared.entityId !== file.entityId ||
        prepared.assetVersionId !== preparedAssetVersionId
      )
        throw new Error('当前评审素材版本尚未准备完成')
      return {
        cacheKey: `review:${prepared.assetVersionId}:${JSON.stringify(representation)}`,
        url: prepared.url,
        width: prepared.width,
        height: prepared.height,
        backend: 'image_io',
      }
    },
    [preparedAssetVersionId],
  )
  const requestImage =
    controller.protocol === 'continuous' ? requestContinuousImage : surfaceProps.requestImage

  const renderSurface = (
    nativeBinding?: ImageRendererViewportBinding,
    nativeEditor?: (projection: ImagePreviewProjection) => ReactNode,
  ) => (
    <ImagePreviewSurface
      {...surfaceProps}
      nativeBinding={nativeBinding}
      requestImage={requestImage}
      prefetchFit={controller.protocol !== 'continuous'}
      onNavigate={(file) => {
        const currentIndex = surfaceProps.files.findIndex(
          (candidate) => candidate.entityId === surfaceProps.file.entityId,
        )
        const nextIndex = surfaceProps.files.findIndex(
          (candidate) => candidate.entityId === file.entityId,
        )
        const offset = nextIndex < currentIndex ? -1 : 1
        void controller.requestLeave({ kind: 'navigate', offset })
      }}
      onEscape={() => {
        if (controller.dirty) controller.cancelDraft()
        else void controller.requestLeave({ kind: 'return_grid' })
      }}
      ariaLabel={`图片评审 ${surfaceProps.file.name}`}
      toolbarLabel="图片评审工具"
      slots={{
        magnifierOverlayPainter,
        toolbarLeading: (
          <>
            <strong ref={identity}>{surfaceProps.file.name}</strong>
            <span>
              {metadata === null
                ? formatBytes(surfaceProps.file.size)
                : `${metadata.width} × ${metadata.height} px · ${formatBytes(surfaceProps.file.size)}`}
            </span>
          </>
        ),
        toolbarActions: (
          <WorkbenchToolbarActions
            controller={controller}
            onArchive={onArchive}
            onHistory={onHistory}
          />
        ),
        stageOverlay: (projection) => (
          <>
            <AnnotationCanvas
              projection={projection}
              controller={controller}
              scene={annotationScene}
            />
            {editorPhase.status !== 'idle' &&
              editorPhase.status !== 'drawing' &&
              editorPhase.sourceItemId === null &&
              editorPhase.draftAnchor.kind !== 'asset' && (
                <InlineFeedbackEditor
                  controller={controller}
                  anchor={editorPhase.draftAnchor}
                  projection={projection}
                />
              )}
          </>
        ),
        nativeStageOverlay: nativeBinding === undefined ? undefined : nativeEditor,
        sidePanel: (
          <ReviewFeedbackRail
            controller={controller}
            nativeGeometryEditing={nativeBinding !== undefined}
          />
        ),
      }}
    />
  )

  return surfaceProps.renderer?.backend === 'native' ? (
    <NativeReviewViewportBridge controller={controller}>
      {({ binding, editor }) => renderSurface(binding, editor)}
    </NativeReviewViewportBridge>
  ) : (
    renderSurface()
  )
}

function WorkbenchToolbarActions({
  controller,
  onArchive,
  onHistory,
}: Pick<ImageReviewWorkspaceProps, 'controller' | 'onArchive' | 'onHistory'>) {
  return (
    <>
      <AnnotationToolbar controller={controller} />
      {controller.protocol === 'continuous' ? (
        <>
          <ViewerButton
            tone="quiet"
            disabled={
              onArchive === undefined || controller.readOnlyReason !== null || controller.dirty
            }
            onClick={onArchive}
          >
            存档意见
          </ViewerButton>
          <ViewerButton tone="quiet" disabled={onHistory === undefined} onClick={onHistory}>
            历史
          </ViewerButton>
        </>
      ) : controller.readOnlyReason === null ? (
        <>
          <ViewerButton
            tone="quiet"
            onClick={() => void controller.requestLeave({ kind: 'finish_review' })}
          >
            完成本轮评审
          </ViewerButton>
          <ViewerButton
            tone="quiet"
            onClick={() => void controller.requestLeave({ kind: 'abandon_review' })}
          >
            放弃本轮
          </ViewerButton>
        </>
      ) : null}
      <ViewerButton
        tone="quiet"
        className="preview-complete-action"
        onClick={() => void controller.requestLeave({ kind: 'return_grid' })}
      >
        返回网格
      </ViewerButton>
    </>
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`
  if (bytes < 1_024 ** 2) return `${formatUnit(bytes / 1_024)} KiB`
  if (bytes < 1_024 ** 3) return `${formatUnit(bytes / 1_024 ** 2)} MiB`
  return `${formatUnit(bytes / 1_024 ** 3)} GiB`
}

function formatUnit(value: number): string {
  return value >= 10 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, '')
}
