import type { MutableRefObject } from 'react'
import { useCallback, useState } from 'react'
import type {
  BrowserFile,
  ImageRepresentation,
  ImageRepresentationRequest,
  MagnifierPreferences,
} from '../api/types'
import ImagePreviewSurface from './imagePreview/ImagePreviewSurface'
import type { Point } from './imagePreview/imageGeometry'
import ViewerButton from './ui/ViewerButton'

interface ImagePreviewProps {
  file: BrowserFile
  files: BrowserFile[]
  magnifier: MagnifierPreferences
  pointerClientPoint: MutableRefObject<Point | null>
  unavailableEntityIds?: ReadonlySet<string>
  requestImage: (
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ) => Promise<ImageRepresentation>
  onNavigate: (file: BrowserFile) => void
  onClose: () => void
  onDimensions?: (entityId: string, width: number, height: number) => void
}

export default function ImagePreview({
  file,
  files,
  magnifier,
  pointerClientPoint,
  unavailableEntityIds,
  requestImage,
  onNavigate,
  onClose,
  onDimensions,
}: ImagePreviewProps) {
  const [resolvedDimensions, setResolvedDimensions] = useState<{
    entityId: string
    width: number
    height: number
  } | null>(null)
  const previewDimensions =
    file.imageMetadata ??
    (resolvedDimensions?.entityId === file.entityId ? resolvedDimensions : undefined)
  const rememberDimensions = useCallback(
    (entityId: string, width: number, height: number) => {
      setResolvedDimensions((current) =>
        current?.entityId === entityId && current.width === width && current.height === height
          ? current
          : { entityId, width, height },
      )
      onDimensions?.(entityId, width, height)
    },
    [onDimensions],
  )
  const previewMetadata = [
    previewDimensions ? `${previewDimensions.width} × ${previewDimensions.height} px` : null,
    formatBytes(file.size),
  ]
    .filter((value): value is string => value !== null)
    .join(' · ')

  return (
    <ImagePreviewSurface
      file={file}
      files={files}
      magnifier={magnifier}
      pointerClientPoint={pointerClientPoint}
      unavailableEntityIds={unavailableEntityIds}
      requestImage={requestImage}
      onNavigate={onNavigate}
      onDimensions={rememberDimensions}
      ariaLabel={`图片预览 ${file.name}`}
      onEscape={onClose}
      slots={{
        toolbarLeading: (
          <>
            <strong>{file.name}</strong>
            <span>{previewMetadata}</span>
          </>
        ),
        toolbarActions: (
          <ViewerButton
            tone="quiet"
            className="preview-complete-action"
            aria-label="返回网格"
            onClick={onClose}
          >
            返回网格
          </ViewerButton>
        ),
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
