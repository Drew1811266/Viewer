import { useCallback, useEffect, useMemo } from 'react'
import type {
  BrowserFile,
  ImageRepresentationRequest,
  SaveTextRequest,
  TextEncoding,
} from '../../api/types'
import {
  createProjectThumbnailCache,
  PROJECT_THUMBNAIL_CACHE_CAPACITY,
  type ThumbnailLoader,
} from '../projectThumbnailCache'
import type { PreviewDataPort } from './ports'

/** warn 级压力下前端缩略图缓存收缩目标（方案三 §4.2）。 */
const MEMORY_PRESSURE_SHRINK_CAPACITY = 128

export interface PreviewDataCoordinator {
  requestThumbnail: ThumbnailLoader
  /** 批量预取缩略图（低解码优先级）；每个文件独立结算，取消由 signal 控制。 */
  prefetchThumbnails(
    files: BrowserFile[],
    maxPixels: number,
    scaleMilli: number,
    signal: AbortSignal,
  ): Promise<void>
  requestFolderImages(entityId: string): Promise<BrowserFile[]>
  requestPreviewImage(
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ): ReturnType<PreviewDataPort['requestImage']>
  requestTextPreview(
    file: BrowserFile,
    encoding?: TextEncoding,
  ): ReturnType<PreviewDataPort['previewText']>
  saveText: PreviewDataPort['saveText']
  renderMarkdown: PreviewDataPort['renderMarkdown']
  requestVideoCover(entityId: string): Promise<string>
  openExternalLink: PreviewDataPort['openExternalLink']
}

export function usePreviewData(
  projectSessionId: string,
  port: PreviewDataPort,
): PreviewDataCoordinator {
  const loadThumbnail = useCallback<ThumbnailLoader>(
    (file, maxPixels, scaleMilli, signal, priority) =>
      port
        .requestImage(
          {
            entityId: file.entityId,
            representation:
              priority === 'prefetch'
                ? { kind: 'thumbnail', maxPixels, scaleMilli, priority }
                : { kind: 'thumbnail', maxPixels, scaleMilli },
          },
          signal,
        )
        .then((image) => image.url),
    [port],
  )
  const thumbnailCache = useMemo(
    () => createProjectThumbnailCache(projectSessionId, loadThumbnail),
    [loadThumbnail, projectSessionId],
  )
  useEffect(() => () => thumbnailCache.clear(), [thumbnailCache])
  /**
   * 内存压力联动（方案三）：系统 warn 收缩前端缩略图 LRU、critical 全清；
   * 恢复 normal 后回到默认容量（已淘汰条目随使用自然回填）。
   * WKWebView 解码位图缓存无法直接清除，依赖虚拟化卸载 <img> 自然回落，
   * 该限制见 docs/architecture/performance-optimization-plan.md §4.2。
   */
  useEffect(() => {
    let disposed = false
    const unlisten = port.listenMemoryPressure(({ level }) => {
      if (disposed) return
      if (level === 'critical') thumbnailCache.clear()
      else if (level === 'warn') thumbnailCache.shrink(MEMORY_PRESSURE_SHRINK_CAPACITY)
      else thumbnailCache.shrink(PROJECT_THUMBNAIL_CACHE_CAPACITY)
    })
    return () => {
      disposed = true
      void unlisten.then((dispose) => dispose())
    }
  }, [port, thumbnailCache])
  const prefetchThumbnails = useCallback<PreviewDataCoordinator['prefetchThumbnails']>(
    (files, maxPixels, scaleMilli, signal) =>
      Promise.allSettled(
        files.map((file) => thumbnailCache.prefetch(file, maxPixels, scaleMilli, signal)),
      ).then(() => undefined),
    [thumbnailCache],
  )

  const requestFolderImages = useCallback(
    async (entityId: string) => {
      const workspace = await port.queryFolder(entityId, false)
      return workspace.workspace === 'content' ? workspace.images : []
    },
    [port],
  )
  const requestPreviewImage = useCallback(
    (file: BrowserFile, representation: ImageRepresentationRequest, signal?: AbortSignal) =>
      port.requestImage({ entityId: file.entityId, representation }, signal),
    [port],
  )
  const requestTextPreview = useCallback(
    (file: BrowserFile, encoding?: TextEncoding) =>
      port.previewText({ entityId: file.entityId, encoding }),
    [port],
  )
  const saveText = useCallback((request: SaveTextRequest) => port.saveText(request), [port])
  const renderMarkdown = useCallback((content: string) => port.renderMarkdown(content), [port])
  const requestVideoCover = useCallback(
    (entityId: string) => port.videoRequestCover(entityId),
    [port],
  )

  return {
    requestThumbnail: thumbnailCache.request,
    prefetchThumbnails,
    requestFolderImages,
    requestPreviewImage,
    requestTextPreview,
    saveText,
    renderMarkdown,
    requestVideoCover,
    openExternalLink: port.openExternalLink,
  }
}
