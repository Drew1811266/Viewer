import { useEffect, useState } from 'react'
import type { BrowserFile } from '../api/types'
import type { ImageDimensions } from '../layout/aspectLayout'
import { thumbnailRequestSize } from '../settings/thumbnailDensity'

export interface AspectThumbnailProps {
  file: BrowserFile
  width: number
  height: number
  dimensionsKnown: boolean
  loadThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onNaturalDimensions: (
    identity: { entityId: string; modifiedNs: string },
    dimensions: ImageDimensions,
  ) => void
}

type ThumbnailState =
  | { status: 'loading'; identityKey: string }
  | { status: 'ready'; identityKey: string; url: string }
  | { status: 'failed'; identityKey: string }

export default function AspectThumbnail({
  file,
  width,
  height,
  dimensionsKnown,
  loadThumbnail,
  onNaturalDimensions,
}: AspectThumbnailProps) {
  const identityKey = `${file.entityId}:${file.modifiedNs}`
  const [state, setState] = useState<ThumbnailState>({
    status: 'loading',
    identityKey,
  })

  useEffect(() => {
    if (loadThumbnail === undefined) return
    let active = true
    const requestSize = thumbnailRequestSize(width, height, window.devicePixelRatio)
    setState((current) =>
      current.identityKey === identityKey ? current : { status: 'loading', identityKey },
    )
    void loadThumbnail(file, requestSize.maxPixels, requestSize.scaleMilli).then(
      (url) => {
        if (active) setState({ status: 'ready', identityKey, url })
      },
      () => {
        if (active) setState({ status: 'failed', identityKey })
      },
    )
    return () => {
      active = false
    }
  }, [file, height, identityKey, loadThumbnail, width])

  const currentState =
    state.identityKey === identityKey ? state : ({ status: 'loading', identityKey } as const)
  const showPlaceholder = currentState.status !== 'ready' || !dimensionsKnown
  const size = { width: `${width}px`, height: `${height}px` }

  return (
    <span className="aspect-thumbnail" data-thumbnail-state={currentState.status} style={size}>
      {showPlaceholder && (
        <span
          className="aspect-thumbnail-placeholder"
          aria-label={currentState.status === 'failed' ? '缩略图不可用' : '缩略图加载中'}
          style={size}
        />
      )}
      {currentState.status === 'ready' && (
        <img
          src={currentState.url}
          alt=""
          style={{
            ...size,
            objectFit: 'contain',
            visibility: dimensionsKnown ? 'visible' : 'hidden',
          }}
          onLoad={(event) => {
            if (dimensionsKnown) return
            const dimensions = {
              width: event.currentTarget.naturalWidth,
              height: event.currentTarget.naturalHeight,
            }
            if (
              !Number.isFinite(dimensions.width) ||
              !Number.isFinite(dimensions.height) ||
              dimensions.width <= 0 ||
              dimensions.height <= 0
            ) {
              return
            }
            onNaturalDimensions(
              { entityId: file.entityId, modifiedNs: file.modifiedNs },
              dimensions,
            )
          }}
        />
      )}
    </span>
  )
}
