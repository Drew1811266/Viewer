import type { BrowserFile, VideoFile } from './api/types'

export const isImageFile = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'jpeg' || file.kind === 'png' || file.kind === 'unsupported_image'

export const isPreviewableImage = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'jpeg' || file.kind === 'png'

export const isVideoFile = (file: BrowserFile): file is VideoFile =>
  file.kind === 'video' && file.videoMetadata != null

export const isOtherFile = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'markdown' || file.kind === 'text' || file.kind === 'other'

export const isPreviewableText = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'markdown' || file.kind === 'text'

export function fileExtensionLabel(name: string): string {
  const separator = name.lastIndexOf('.')
  if (separator <= 0 || separator === name.length - 1) return '无扩展名'
  return `.${name.slice(separator + 1).toUpperCase()}`
}
