import type { ReactNode } from 'react'
import { useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type { ImageRendererViewportBinding } from '../../rendering/imageRendererTypes'
import InfoOverlay from '../InfoOverlay'
import ImagePreviewSurface, {
  type ImagePreviewSurfaceProps,
} from '../imagePreview/ImagePreviewSurface'
import type { ImagePreviewProjection } from '../imagePreview/imagePreviewProjection'
import ViewerButton, { ViewerIconButton } from '../ui/ViewerButton'
import ViewerIcon from '../ui/ViewerIcon'
import AnnotationCanvas from './AnnotationCanvas'
import AnnotationStyleBar, { shouldShowAnnotationStyleBar } from './AnnotationStyleBar'
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
  const [fileInfoOpen, setFileInfoOpen] = useState(false)
  const identity = useRef<HTMLElement | null>(null)
  const preparedImageRef = useRef(controller.preparedImage)
  preparedImageRef.current = controller.preparedImage
  const editorPhase = controller.editor.phase
  const transientAnchor = editorPhase.status === 'idle' ? null : editorPhase.draftAnchor
  const transientStyle = editorPhase.status === 'idle' ? null : editorPhase.style
  const transientText =
    editorPhase.status !== 'idle' && editorPhase.status !== 'drawing' ? editorPhase.text : null
  const editingItemId =
    editorPhase.status === 'idle' || editorPhase.status === 'drawing'
      ? null
      : editorPhase.sourceItemId
  const editingOperation =
    editorPhase.status === 'idle' || editorPhase.status === 'drawing'
      ? null
      : (editorPhase.operation ?? null)
  const annotationScene = useMemo(
    () =>
      buildAnnotationScene({
        feedback: controller.feedback,
        selectedItemId: controller.selectedItemId,
        transientAnchor,
        transientStyle,
        transientText,
        editingItemId,
        editingOperation,
      }),
    [
      controller.feedback,
      controller.selectedItemId,
      editingItemId,
      editingOperation,
      transientAnchor,
      transientStyle,
      transientText,
    ],
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

  useLayoutEffect(() => {
    const root = identity.current?.closest<HTMLElement>('.image-preview')
    if (root === null || root === undefined) return
    const update = () => {
      const width = root.clientWidth
      if (width > 0) root.dataset.compactToolbar = width <= 820 ? 'true' : ''
    }
    update()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(update)
    observer.observe(root)
    return () => observer.disconnect()
  }, [])

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

  // The native renderer has no polygon primitive yet. While the polygon tool
  // is active, render the image through the Web viewport so the existing
  // multi-click Canvas polygon gesture stays available.
  const polygonToolActive = controller.tool === 'polygon'
  const renderer = polygonToolActive ? undefined : surfaceProps.renderer
  const renderSurface = (
    nativeBinding?: ImageRendererViewportBinding,
    nativeEditor?: (projection: ImagePreviewProjection) => ReactNode,
  ) => (
    <ImagePreviewSurface
      {...surfaceProps}
      renderer={renderer}
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
      ariaLabel={`图片评审 ${surfaceProps.file.name}`}
      toolbarLabel="图片评审工具"
      slots={{
        magnifierOverlayPainter,
        toolbarLeading: (
          <>
            <ViewerIconButton
              icon="arrow-left"
              label="返回"
              title="返回网格"
              tone="quiet"
              className="preview-back-action"
              onClick={() => void controller.requestLeave({ kind: 'return_grid' })}
            />
            <div className="preview-identity">
              <strong ref={identity}>{surfaceProps.file.name}</strong>
              <span>
                {metadata === null
                  ? formatBytes(surfaceProps.file.size)
                  : `${metadata.width} × ${metadata.height} px · ${formatBytes(
                      surfaceProps.file.size,
                    )}`}
                {` · 当前意见 ${controller.feedback.length}`}
              </span>
            </div>
          </>
        ),
        toolbarActions: (
          <WorkbenchToolbarActions
            controller={controller}
            fileInfoOpen={fileInfoOpen}
            onFileInfoToggle={() => setFileInfoOpen((current) => !current)}
            onArchive={onArchive}
            onHistory={onHistory}
          />
        ),
        banner:
          controller.readOnlyReason === 'write_unavailable' ? (
            <div className="review-readonly-banner" role="status">
              <ViewerIcon name="lock" />
              <span>项目为只读：可以查看已有意见，不能新增或修改标注。</span>
            </div>
          ) : undefined,
        stageOverlay: (projection) => (
          <>
            <AnnotationCanvas
              projection={projection}
              controller={controller}
              scene={annotationScene}
            />
            {shouldShowAnnotationStyleBar(controller) && (
              <AnnotationStyleBar controller={controller} />
            )}
            {editorPhase.status !== 'idle' &&
              editorPhase.status !== 'drawing' &&
              editorPhase.sourceItemId === null &&
              editorPhase.draftAnchor.kind !== 'asset' && (
                <InlineFeedbackEditor
                  controller={controller}
                  anchor={editorPhase.draftAnchor}
                  projection={projection}
                  topInset={shouldShowAnnotationStyleBar(controller) ? 64 : 8}
                />
              )}
          </>
        ),
        nativeStageOverlay: nativeBinding === undefined ? undefined : nativeEditor,
        sidePanel: <ReviewFeedbackRail controller={controller} />,
      }}
    />
  )

  const surface =
    surfaceProps.renderer?.backend === 'native' && !polygonToolActive ? (
      <NativeReviewViewportBridge controller={controller}>
        {({ binding, editor }) => renderSurface(binding, editor)}
      </NativeReviewViewportBridge>
    ) : (
      renderSurface()
    )

  return (
    <>
      {surface}
      {fileInfoOpen && (
        <InfoOverlay
          files={[surfaceProps.file]}
          dimensions={
            metadata === null
              ? {}
              : {
                  [surfaceProps.file.entityId]: {
                    width: metadata.width,
                    height: metadata.height,
                  },
                }
          }
          onClose={() => setFileInfoOpen(false)}
        />
      )}
    </>
  )
}

function WorkbenchToolbarActions({
  controller,
  fileInfoOpen,
  onFileInfoToggle,
  onArchive,
  onHistory,
}: Pick<ImageReviewWorkspaceProps, 'controller' | 'onArchive' | 'onHistory'> & {
  fileInfoOpen: boolean
  onFileInfoToggle(): void
}) {
  return (
    <>
      <AnnotationToolbar controller={controller} />
      {controller.protocol === 'continuous' ? (
        <>
          <ViewerButton
            leadingIcon="archive"
            disabled={
              onArchive === undefined || controller.readOnlyReason !== null || controller.dirty
            }
            onClick={onArchive}
          >
            存档意见
          </ViewerButton>
          <ViewerButton disabled={onHistory === undefined} onClick={onHistory}>
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
      <ViewerButton active={fileInfoOpen} onClick={onFileInfoToggle}>
        文件信息
      </ViewerButton>
      <ViewerButton
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
