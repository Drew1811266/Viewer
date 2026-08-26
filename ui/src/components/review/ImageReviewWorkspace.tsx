import { useEffect, useRef } from 'react'
import type {
  ImageReviewWorkbenchController,
  ReviewLeaveIntent,
} from '../../app/review/useImageReviewWorkbench'
import ImagePreviewSurface, {
  type ImagePreviewSurfaceProps,
} from '../imagePreview/ImagePreviewSurface'
import ViewerButton from '../ui/ViewerButton'
import AnnotationCanvas from './AnnotationCanvas'
import AnnotationToolbar from './AnnotationToolbar'
import InlineFeedbackEditor from './InlineFeedbackEditor'
import ReviewFeedbackRail from './ReviewFeedbackRail'

export interface ImageReviewWorkspaceProps extends Omit<ImagePreviewSurfaceProps, 'slots'> {
  controller: ImageReviewWorkbenchController
  onReturnGrid(): void
  onFinishReview(): void
}

export default function ImageReviewWorkspace({
  controller,
  onReturnGrid,
  onFinishReview,
  onNavigate,
  ...surfaceProps
}: ImageReviewWorkspaceProps) {
  const compactDefaultEntity = useRef<string | null>(null)

  useEffect(() => {
    if (compactDefaultEntity.current === surfaceProps.file.entityId) return
    compactDefaultEntity.current = surfaceProps.file.entityId
    if (window.matchMedia?.('(max-width: 700px)').matches) controller.setRailOpen(false)
  }, [controller, surfaceProps.file.entityId])

  async function leave(intent: ReviewLeaveIntent, action: () => void) {
    if ((await controller.requestLeave(intent)) === 'proceeded') action()
  }

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
        void leave({ kind: 'navigate', offset }, () => onNavigate(file))
      }}
      onEscape={() => {
        if (controller.dirty) controller.cancelDraft()
        else void leave({ kind: 'return_grid' }, onReturnGrid)
      }}
      ariaLabel={`图片评审 ${surfaceProps.file.name}`}
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
            <ViewerButton
              tone="quiet"
              onClick={() => void leave({ kind: 'finish_review' }, onFinishReview)}
            >
              完成本轮评审
            </ViewerButton>
            <ViewerButton
              tone="quiet"
              className="preview-complete-action"
              onClick={() => void leave({ kind: 'return_grid' }, onReturnGrid)}
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
