import { useEffect, useRef } from 'react'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import ImagePreviewSurface, {
  type ImagePreviewSurfaceProps,
} from '../imagePreview/ImagePreviewSurface'
import ViewerButton from '../ui/ViewerButton'
import AnnotationCanvas from './AnnotationCanvas'
import AnnotationToolbar from './AnnotationToolbar'
import InlineFeedbackEditor from './InlineFeedbackEditor'
import ReviewFeedbackRail from './ReviewFeedbackRail'

export interface ImageReviewWorkspaceProps
  extends Omit<ImagePreviewSurfaceProps, 'slots' | 'onNavigate'> {
  controller: ImageReviewWorkbenchController
}

export default function ImageReviewWorkspace({
  controller,
  ...surfaceProps
}: ImageReviewWorkspaceProps) {
  const compactDefaultEntity = useRef<string | null>(null)

  useEffect(() => {
    if (compactDefaultEntity.current === surfaceProps.file.entityId) return
    compactDefaultEntity.current = surfaceProps.file.entityId
    if (window.matchMedia?.('(max-width: 700px)').matches) controller.setRailOpen(false)
  }, [controller, surfaceProps.file.entityId])

  const metadata = surfaceProps.file.imageMetadata

  return (
    <ImagePreviewSurface
      {...surfaceProps}
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
        toolbarLeading: (
          <>
            <strong>{surfaceProps.file.name}</strong>
            <span>
              {metadata === null
                ? formatBytes(surfaceProps.file.size)
                : `${metadata.width} × ${metadata.height} px · ${formatBytes(surfaceProps.file.size)}`}
            </span>
          </>
        ),
        toolbarActions: (
          <>
            <AnnotationToolbar controller={controller} />
            {controller.readOnlyReason === null && (
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
            )}
            <ViewerButton
              tone="quiet"
              className="preview-complete-action"
              onClick={() => void controller.requestLeave({ kind: 'return_grid' })}
            >
              返回网格
            </ViewerButton>
          </>
        ),
        stageOverlay: (projection) => (
          <>
            <AnnotationCanvas projection={projection} controller={controller} />
            {controller.editor.status !== 'idle' &&
              controller.editor.status !== 'drawing' &&
              controller.editor.draftAnchor.kind !== 'asset' && (
                <InlineFeedbackEditor
                  controller={controller}
                  anchor={controller.editor.draftAnchor}
                  projection={projection}
                />
              )}
          </>
        ),
        sidePanel: <ReviewFeedbackRail controller={controller} />,
      }}
    />
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
