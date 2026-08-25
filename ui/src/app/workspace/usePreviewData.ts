import { useCallback, useEffect, useMemo } from 'react'
import type { BrowserFile, ImageRepresentationRequest, TextEncoding } from '../../api/types'
import { createProjectThumbnailCache, type ThumbnailLoader } from '../projectThumbnailCache'
import type { PreviewDataPort } from './ports'

export interface PreviewDataCoordinator {
  requestThumbnail: ThumbnailLoader
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
  requestVideoCover(entityId: string): Promise<string>
  openExternalLink: PreviewDataPort['openExternalLink']
}

export function usePreviewData(
  projectSessionId: string,
  port: PreviewDataPort,
): PreviewDataCoordinator {
  const loadThumbnail = useCallback<ThumbnailLoader>(
    (file, maxPixels, scaleMilli) =>
      port
        .requestImage({
          entityId: file.entityId,
          representation: { kind: 'thumbnail', maxPixels, scaleMilli },
        })
        .then((image) => image.url),
    [port],
  )
  const thumbnailCache = useMemo(
    () => createProjectThumbnailCache(projectSessionId, loadThumbnail),
    [loadThumbnail, projectSessionId],
  )
  useEffect(() => () => thumbnailCache.clear(), [thumbnailCache])

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
  const requestVideoCover = useCallback(
    (entityId: string) => port.videoRequestCover(entityId),
    [port],
  )

  return {
    requestThumbnail: thumbnailCache.request,
    requestFolderImages,
    requestPreviewImage,
    requestTextPreview,
    requestVideoCover,
    openExternalLink: port.openExternalLink,
  }
}
