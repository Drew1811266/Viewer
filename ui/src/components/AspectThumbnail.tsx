import { useEffect, useState } from 'react'
import type { BrowserFile } from '../api/types'
import type { ImageDimensions } from '../layout/aspectLayout'
import { thumbnailRequestSize } from '../settings/thumbnailDensity'

export interface AspectThumbnailProps {
  file: BrowserFile
  width: number
  height: number
  dimensionsKnown: boolean
  loadThumbnail?: (
    file: BrowserFile,
    maxPixels: number,
    scaleMilli: number,
    signal?: AbortSignal,
  ) => Promise<string>
  /** 宿主取消触发器：abort 时级联取消在途请求；请求生命周期由组件自持。 */
  signal?: AbortSignal
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
  signal,
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
    // 每次请求自持生命周期控制器：StrictMode 的模拟卸载会重跑 effect，
    // 重跑后仍能拿到全新信号（宿主持有的 controller 会被第一次清理 abort
    // 且 useMemo 跨模拟重挂载存活，导致重挂载后全部请求以 aborted 拒绝）。
    // 宿主 signal 只作外部取消触发器级联进内部控制器。
    const controller = new AbortController()
    const onHostAbort = () => controller.abort()
    if (signal?.aborted) onHostAbort()
    signal?.addEventListener('abort', onHostAbort)
    void loadThumbnail(file, requestSize.maxPixels, requestSize.scaleMilli, controller.signal).then(
      (url) => {
        if (active) setState({ status: 'ready', identityKey, url })
      },
      () => {
        if (active) setState({ status: 'failed', identityKey })
      },
    )
    return () => {
      active = false
      signal?.removeEventListener('abort', onHostAbort)
      controller.abort()
    }
  }, [file, height, identityKey, loadThumbnail, signal, width])

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
          decoding="async"
          draggable={false}
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
