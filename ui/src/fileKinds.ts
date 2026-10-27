import type { BrowserFile, VideoFile } from './api/types'

export const isImageFile = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'jpeg' ||
  file.kind === 'png' ||
  file.kind === 'image' ||
  file.kind === 'unsupported_image'

export const isPreviewableImage = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'jpeg' || file.kind === 'png' || file.kind === 'image'

export const isVideoFile = (file: BrowserFile): file is VideoFile =>
  file.kind === 'video' && file.videoMetadata != null

export const isOtherFile = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'markdown' || file.kind === 'text' || file.kind === 'other'

export const isPreviewableText = (file: Pick<BrowserFile, 'kind'>) =>
  file.kind === 'markdown' || file.kind === 'text'

const VECTOR_IMAGE_EXTENSIONS = new Set(['svg', 'svgz', 'ai'])

export function isVectorImage(file: Pick<BrowserFile, 'name'>): boolean {
  const separator = file.name.lastIndexOf('.')
  if (separator <= 0 || separator === file.name.length - 1) return false
  return VECTOR_IMAGE_EXTENSIONS.has(file.name.slice(separator + 1).toLowerCase())
}

export function fileExtensionLabel(name: string): string {
  const separator = name.lastIndexOf('.')
  if (separator <= 0 || separator === name.length - 1) return '无扩展名'
  return `.${name.slice(separator + 1).toUpperCase()}`
}
