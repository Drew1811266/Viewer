import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { BrowserFile, ContentFolderCard, ThumbnailDensity } from '../api/types'
import { isPreviewableImage } from '../fileKinds'
import {
  type AspectGeometry,
  anchoredScrollOffset,
  buildFilmstripGeometry,
  horizontalVisibleIndexes,
  type ImageDimensions,
  validDimensions,
} from '../layout/aspectLayout'
import { THUMBNAIL_HEIGHT } from '../settings/thumbnailDensity'
import AspectThumbnail from './AspectThumbnail'
import ReviewFeedbackCountBadge from './review/ReviewFeedbackCountBadge'
import UnsupportedFileState from './UnsupportedFileState'
import ViewerButton from './ui/ViewerButton'
import ViewerLocalFeedback from './ui/ViewerLocalFeedback'

const THUMBNAIL_GAP = 8
const FILMSTRIP_INLINE_PADDING = 12
/**
 * 预挂载窗口（2026-09-15 滚动采样后加大）：viewer-image:// 响应是
 * Cache-Control: private, max-age=60（ADR 0007，内存缓存不落盘），重挂载可
 * 复用已解码位图；更大的预挂载窗口仍让首次解码在单元格进入视口之前完成，
 * 把主线程兜底解码换成后台解码。
 */
const OVERSCAN_CELLS = 6
const OVERSCAN_PIXELS = 1_024
/**
 * 渐进挂载（2026-09-15 滚动采样）：行进入近邻时若一次性挂载整个预挂载窗口
 * （可见 ± 6 格），单次 React 提交要插入几十个 <img>，对应一次长帧（掉帧
 * 尖峰）。改为先只挂可见窗口，随后每帧扩展一步预挂载，把突发摊到
 * MOUNT_RAMP_STEPS 帧内；近邻区域内保持满档，不随微小滚动重置。
 */
const MOUNT_RAMP_STEPS = 2
/** 行进入挂载的提前量：整行提前 ~1.5 行挂载，列表 IPC 与首屏解码先于可见发生。 */
const ROW_ROOT_MARGIN = '600px 0px'
/**
 * 纵向近邻门控（迟滞）：行滚出视口 1.5 行外仍保持挂载，超过 ~3.3 行才卸载
 * 全部 `<img>`。卸载把存活图片约束在近邻几行内，限制实时渲染与请求规模；
 * 重挂载的解码位图复用由 ADR 0007 的内存缓存承担（private, max-age=60）。
 */
const ROW_NEAR_MARGIN = '1500px 0px'
const SCROLLBAR_INLINE_INSET = 4
const SCROLLBAR_MIN_THUMB_WIDTH = 36
/**
 * 2026-09-15 滚动采样：视口外行的布局（LocalFrameViewLayoutContext）与图层
 * 绘制（paintReachableBackingStoreContents）是主线程忙时大头，而所有行永久
 * 挂载（FolderOverview 不做行级虚拟化）。app.css 给行加的
 * content-visibility: auto 让 WebKit 跳过视口外行的内容布局与绘制；组件按
 * 密度内联 contain-intrinsic-size（初值 = minHeight），`auto` 关键字在首次
 * 渲染后记住真实高度，避免滚动条跳动。
 */

interface FolderFilmstripRowProps {
  folder: ContentFolderCard
  density: ThumbnailDensity
  loadImages: (entityId: string, retry?: boolean) => Promise<BrowserFile[]>
  onSelect: (entityId: string) => void
  onPreview: (file: BrowserFile, files: BrowserFile[]) => void
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  feedbackCountByEntityId?: ReadonlyMap<string, number>
}

interface FilmstripThumbnailProps {
  file: BrowserFile
  imageWidth: number
  imageHeight: number
  dimensions: ImageDimensions | null
  requestThumbnail?: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onNaturalDimensions: (
    identity: { entityId: string; modifiedNs: string },
    dimensions: ImageDimensions,
  ) => void
}

/**
 * 单个挂载缩略图的宿主。卸载即取消由 AspectThumbnail 内部的请求控制器完成
 * （StrictMode 模拟卸载安全）；宿主不再持有可能跨模拟重挂载存活的
 * AbortController——那会让重跑的 effect 拿到已 aborted 的信号，
 * 所有缩略图请求被永久拒绝。
 */
function FilmstripThumbnail({
  file,
  imageWidth,
  imageHeight,
  dimensions,
  requestThumbnail,
  onNaturalDimensions,
}: FilmstripThumbnailProps) {
  return (
    <AspectThumbnail
      file={file}
      width={imageWidth}
      height={imageHeight}
      dimensionsKnown={validDimensions(dimensions)}
      loadThumbnail={requestThumbnail}
      onNaturalDimensions={onNaturalDimensions}
    />
  )
}

type RowState =
  | { status: 'idle' | 'loading' }
  | { status: 'ready'; images: BrowserFile[] }
  | { status: 'failed' }

interface HorizontalAnchor {
  key: string
  visualOffset: number
}

export default function FolderFilmstripRow({
  folder,
  density,
  loadImages,
  onSelect,
  onPreview,
  requestThumbnail,
  feedbackCountByEntityId,
}: FolderFilmstripRowProps) {
  const row = useRef<HTMLElement>(null)
  const filmstrip = useRef<HTMLDivElement>(null)
  const previousGeometry = useRef<AspectGeometry | null>(null)
  const horizontalAnchor = useRef<HorizontalAnchor | null>(null)
  const requestSequence = useRef(0)
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === 'undefined')
  const [nearViewport, setNearViewport] = useState(
    () => typeof IntersectionObserver === 'undefined',
  )
  const [state, setState] = useState<RowState>({ status: 'idle' })
  const [viewport, setViewport] = useState({ scrollLeft: 0, width: 0 })
  const [mountRamp, setMountRamp] = useState(0)
  const [focusedImageIndex, setFocusedImageIndex] = useState<number | null>(null)
  const [recoveredDimensions, setRecoveredDimensions] = useState(
    () => new Map<string, ImageDimensions>(),
  )

  useEffect(() => {
    if (typeof IntersectionObserver === 'undefined' || row.current === null) return
    const mountObserver = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return
        setVisible(true)
        mountObserver.disconnect()
      },
      { rootMargin: ROW_ROOT_MARGIN },
    )
    mountObserver.observe(row.current)
    // 双向跟踪：行滚出近邻范围即卸载图片（解码位图有界），回到近邻即恢复。
    const nearObserver = new IntersectionObserver(
      (entries) => setNearViewport(entries.some((entry) => entry.isIntersecting)),
      { rootMargin: ROW_NEAR_MARGIN },
    )
    nearObserver.observe(row.current)
    return () => {
      mountObserver.disconnect()
      nearObserver.disconnect()
    }
  }, [])

  // 渐进挂载坡道：进入近邻从 0 开始逐帧升至 MOUNT_RAMP_STEPS；滚出近邻即
  // 归零。坡道只由近邻翻转驱动，区域内滚动不重置，避免反复突发。
  useEffect(() => {
    if (!nearViewport) {
      setMountRamp(0)
      return
    }
    let frame: number | null = null
    let step = 0
    const tick = () => {
      step += 1
      setMountRamp(step)
      if (step < MOUNT_RAMP_STEPS) frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => {
      if (frame !== null) cancelAnimationFrame(frame)
    }
  }, [nearViewport])

  const requestImages = useCallback(
    (retry: boolean) => {
      requestSequence.current += 1
      const requestId = requestSequence.current
      setState({ status: 'loading' })
      void loadImages(folder.entityId, retry).then(
        (images) => {
          if (requestId === requestSequence.current) {
            setState({ status: 'ready', images })
          }
        },
        () => {
          if (requestId === requestSequence.current) setState({ status: 'failed' })
        },
      )
    },
    [folder.entityId, loadImages],
  )

  useEffect(() => {
    if (!visible || state.status !== 'idle') return
    requestImages(false)
  }, [requestImages, state.status, visible])

  useEffect(
    () => () => {
      requestSequence.current += 1
    },
    [],
  )

  const updateViewport = useCallback(() => {
    const element = filmstrip.current
    if (element === null) return
    const next = {
      scrollLeft: element.scrollLeft,
      width: element.clientWidth,
    }
    const currentGeometry = previousGeometry.current
    if (currentGeometry !== null) {
      horizontalAnchor.current = captureHorizontalAnchor(currentGeometry, next.scrollLeft)
    }
    setViewport((current) =>
      current.scrollLeft === next.scrollLeft && current.width === next.width ? current : next,
    )
  }, [])

  useLayoutEffect(() => {
    const element = filmstrip.current
    if (element === null) return
    updateViewport()
    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(() => updateViewport())
      observer.observe(element)
      return () => observer.disconnect()
    }
    window.addEventListener('resize', updateViewport)
    return () => window.removeEventListener('resize', updateViewport)
  }, [updateViewport])

  const imageHeight = THUMBNAIL_HEIGHT[density]
  const images = state.status === 'ready' ? state.images : []
  const geometry = useMemo(
    () =>
      buildFilmstripGeometry(
        images.map((file) => ({
          key: imageIdentity(file),
          dimensions: dimensionsFor(file, recoveredDimensions),
        })),
        imageHeight,
        THUMBNAIL_GAP,
        FILMSTRIP_INLINE_PADDING,
      ),
    [imageHeight, images, recoveredDimensions],
  )

  useLayoutEffect(() => {
    const element = filmstrip.current
    const previous = previousGeometry.current
    if (
      element !== null &&
      previous !== null &&
      previous !== geometry &&
      horizontalAnchor.current !== null &&
      previous.items.length > 0 &&
      geometry.items.length > 0
    ) {
      const anchor = horizontalAnchor.current
      const previousIndex = previous.indexByKey.get(anchor.key)
      const previousItem = previousIndex === undefined ? undefined : previous.items[previousIndex]
      if (previousItem !== undefined) {
        const previousScrollLeft = previousItem.left - anchor.visualOffset
        element.scrollLeft = anchoredScrollOffset(
          previous,
          geometry,
          anchor.key,
          previousScrollLeft,
          'horizontal',
        )
      }
    }
    previousGeometry.current = geometry
    updateViewport()
  }, [geometry, updateViewport])

  const rampCells = Math.round((OVERSCAN_CELLS * mountRamp) / MOUNT_RAMP_STEPS)
  const rampPixels = Math.round((OVERSCAN_PIXELS * mountRamp) / MOUNT_RAMP_STEPS)
  const imageWindow =
    state.status === 'ready' && nearViewport
      ? expandImageWindow(
          horizontalVisibleIndexes(geometry, viewport.scrollLeft, viewport.width, rampPixels),
          geometry.items.length,
          rampCells,
        )
      : null

  const rememberNaturalDimensions = useCallback(
    (identity: { entityId: string; modifiedNs: string }, dimensions: ImageDimensions) => {
      if (!validDimensions(dimensions)) return
      const key = `${identity.entityId}:${identity.modifiedNs}`
      setRecoveredDimensions((current) => {
        const existing = current.get(key)
        if (existing?.width === dimensions.width && existing.height === dimensions.height) {
          return current
        }
        const next = new Map(current)
        next.set(key, dimensions)
        return next
      })
    },
    [],
  )

  const reviewed = folder.reviewProgress.total - folder.reviewProgress.unmarked
  const reviewedPercent =
    folder.reviewProgress.total > 0 ? Math.round((reviewed / folder.reviewProgress.total) * 100) : 0
  const scrollbar = overlayScrollbarMetrics(
    viewport.width,
    geometry.totalWidth,
    viewport.scrollLeft,
  )

  return (
    <article
      ref={row}
      className="folder-filmstrip-row"
      style={{
        minHeight: `${imageHeight + 24}px`,
        // 与 CSS 的 content-visibility: auto 配套：按密度给出占位高度，
        // 首次渲染后 WebKit 用 `auto` 记住的真实高度覆盖该估计值。
        containIntrinsicSize: `auto ${imageHeight + 24}px`,
      }}
    >
      <button
        type="button"
        className="folder-filmstrip-identity"
        aria-label={`打开 ${folder.name}`}
        onClick={() => onSelect(folder.entityId)}
      >
        <span className="folder-filmstrip-title">
          <strong>{folder.name}</strong>
          <span className="folder-filmstrip-path" title={folder.relativePath}>
            {folder.relativePath}
          </span>
        </span>
        <span className="folder-filmstrip-counts">
          <strong>{folder.imageCount}</strong> 张图片
          {folder.otherFileCount > 0 && (
            <>
              {' '}
              <span className="folder-filmstrip-extra">
                {`· ${folder.otherFileCount} 个其它文件`}
              </span>
            </>
          )}
        </span>
        <span className="folder-filmstrip-divider" aria-hidden="true" />
        <span
          className="folder-filmstrip-marker"
          data-marker={folder.marker.reviewState ?? 'unmarked'}
        >
          <span className="folder-filmstrip-marker-dot" aria-hidden="true" />
          {markerLabel(folder.marker)}
          {folder.reviewProgress.reject > 0 && (
            <span
              className="folder-filmstrip-reject"
              title={`淘汰 ${folder.reviewProgress.reject} 张`}
            >
              {folder.reviewProgress.reject}
            </span>
          )}
        </span>
        <span className="folder-filmstrip-progress">
          <span className="folder-filmstrip-bar" aria-hidden="true">
            <i
              style={{ width: `${reviewedPercent}%` }}
              data-reject={folder.reviewProgress.reject > 0 || undefined}
            />
          </span>
          <span className="folder-filmstrip-reviewed">
            <span>已审阅</span>
            <span className="folder-filmstrip-reviewed-num">
              {`${reviewed} / ${folder.reviewProgress.total}`}
            </span>
          </span>
        </span>
      </button>
      <div className="folder-filmstrip-shell">
        <div
          ref={filmstrip}
          className="folder-filmstrip"
          role="region"
          aria-label={`${folder.name} 图片`}
          data-state={state.status}
          onScroll={updateViewport}
        >
          {state.status === 'idle' && (
            <span
              className="folder-filmstrip-deferred"
              aria-label="等待加载图片"
              style={{ height: `${imageHeight}px` }}
            />
          )}
          {state.status === 'loading' &&
            ['first', 'second', 'third', 'fourth'].map((key) => (
              <span
                className="folder-filmstrip-skeleton"
                aria-label="图片加载中"
                key={key}
                style={{ width: `${imageHeight}px`, height: `${imageHeight}px` }}
              />
            ))}
          {state.status === 'failed' && (
            <div className="folder-filmstrip-error">
              <ViewerLocalFeedback
                tone="danger"
                title="无法加载图片"
                action={
                  <ViewerButton
                    aria-label={`重试 ${folder.name}`}
                    onClick={() => requestImages(true)}
                  >
                    重试
                  </ViewerButton>
                }
              >
                此文件夹的缩略图暂时不可用。
              </ViewerLocalFeedback>
            </div>
          )}
          {state.status === 'ready' && state.images.length === 0 && <p>无图片</p>}
          {state.status === 'ready' && imageWindow !== null && state.images.length > 0 && (
            <div
              className="folder-filmstrip-track"
              role="list"
              style={{ width: `${geometry.totalWidth}px`, height: `${imageHeight}px` }}
            >
              {getMountedImageIndexes(imageWindow, focusedImageIndex).map((index) => {
                const file = state.images[index]
                const item = geometry.items[index]
                if (file === undefined || item === undefined) {
                  throw new Error(`Missing filmstrip image at mounted index ${index}`)
                }
                const dimensions = dimensionsFor(file, recoveredDimensions)
                return (
                  <div
                    className="folder-filmstrip-item"
                    role="listitem"
                    aria-posinset={index + 1}
                    aria-setsize={state.images.length}
                    key={item.key}
                    style={{
                      left: `${item.left}px`,
                      width: `${item.imageWidth}px`,
                      height: `${item.imageHeight}px`,
                    }}
                  >
                    <button
                      type="button"
                      className="folder-filmstrip-thumbnail"
                      aria-label={`预览 ${file.name}`}
                      title={file.name}
                      onFocus={() => setFocusedImageIndex(index)}
                      onBlur={() =>
                        setFocusedImageIndex((current) => (current === index ? null : current))
                      }
                      onDoubleClick={() => onPreview(file, state.images)}
                      onKeyDown={(event) => {
                        if (event.key !== 'Enter' && event.key !== ' ') return
                        event.preventDefault()
                        onPreview(file, state.images)
                      }}
                    >
                      {isPreviewableImage(file) ? (
                        <FilmstripThumbnail
                          file={file}
                          imageWidth={item.imageWidth}
                          imageHeight={item.imageHeight}
                          dimensions={dimensions}
                          requestThumbnail={requestThumbnail}
                          onNaturalDimensions={rememberNaturalDimensions}
                        />
                      ) : (
                        <UnsupportedFileState file={file} compact />
                      )}
                    </button>
                    <ReviewFeedbackCountBadge
                      count={feedbackCountByEntityId?.get(file.entityId)}
                      className="folder-filmstrip-feedback-count"
                    />
                  </div>
                )
              })}
            </div>
          )}
        </div>
        {scrollbar !== null && (
          <div
            aria-hidden="true"
            className="folder-filmstrip-scrollbar"
            data-testid="folder-filmstrip-scrollbar"
          >
            <span
              className="folder-filmstrip-scrollbar-thumb"
              data-testid="folder-filmstrip-scrollbar-thumb"
              style={{
                width: `${scrollbar.thumbWidth}px`,
                transform: `translateX(${scrollbar.thumbOffset}px)`,
              }}
            />
          </div>
        )}
      </div>
    </article>
  )
}

function overlayScrollbarMetrics(viewportWidth: number, contentWidth: number, scrollLeft: number) {
  const trackWidth = Math.max(0, viewportWidth - SCROLLBAR_INLINE_INSET * 2)
  const maximumScroll = Math.max(0, contentWidth - viewportWidth)
  if (trackWidth === 0 || maximumScroll === 0) return null

  const thumbWidth = Math.min(
    trackWidth,
    Math.max(SCROLLBAR_MIN_THUMB_WIDTH, trackWidth * (viewportWidth / contentWidth)),
  )
  const progress = Math.max(0, Math.min(1, scrollLeft / maximumScroll))
  return {
    thumbWidth,
    thumbOffset: (trackWidth - thumbWidth) * progress,
  }
}

function imageIdentity(file: BrowserFile): string {
  return `${file.entityId}:${file.modifiedNs}`
}

function dimensionsFor(
  file: BrowserFile,
  recoveredDimensions: ReadonlyMap<string, ImageDimensions>,
): ImageDimensions | null {
  if (validDimensions(file.imageMetadata)) return file.imageMetadata
  const recovered = recoveredDimensions.get(imageIdentity(file))
  if (recovered !== undefined && validDimensions(recovered)) return recovered
  return null
}

function captureHorizontalAnchor(
  geometry: AspectGeometry,
  scrollLeft: number,
): HorizontalAnchor | null {
  const safeScrollLeft = Number.isFinite(scrollLeft) && scrollLeft > 0 ? scrollLeft : 0
  let low = 0
  let high = geometry.items.length
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    const item = geometry.items[middle]
    if (item !== undefined && item.left + item.width > safeScrollLeft) high = middle
    else low = middle + 1
  }
  const item = geometry.items[low]
  return item === undefined
    ? null
    : {
        key: item.key,
        visualOffset: item.left - safeScrollLeft,
      }
}

function expandImageWindow(
  visible: { start: number; end: number },
  imageCount: number,
  overscanCells: number,
): { start: number; end: number } {
  return {
    start: Math.max(0, visible.start - overscanCells),
    end: Math.min(imageCount, visible.end + overscanCells),
  }
}

function getMountedImageIndexes(
  imageWindow: { start: number; end: number },
  focusedImageIndex: number | null,
) {
  const indexes = Array.from(
    { length: imageWindow.end - imageWindow.start },
    (_, offset) => imageWindow.start + offset,
  )
  if (
    focusedImageIndex === null ||
    (focusedImageIndex >= imageWindow.start && focusedImageIndex < imageWindow.end)
  ) {
    return indexes
  }
  if (focusedImageIndex < imageWindow.start) indexes.unshift(focusedImageIndex)
  else indexes.push(focusedImageIndex)
  return indexes
}

function markerLabel(marker: ContentFolderCard['marker']): string {
  const review =
    marker.reviewState === 'keep'
      ? '保留'
      : marker.reviewState === 'pending'
        ? '待定'
        : marker.reviewState === 'reject'
          ? '淘汰'
          : '未标记'
  return marker.favorite ? `${review} · 收藏` : review
}
