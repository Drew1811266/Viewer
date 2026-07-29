import type { BrowserFile } from '../../api/types'

export type AdaptiveContentMode =
  | 'empty'
  | 'image_only'
  | 'mixed_collapsed'
  | 'mixed_expanded'
  | 'text_only'

export type SelectAllScope = 'images' | 'text' | 'all'

export type SelectAllRequest =
  | { kind: 'none' }
  | { kind: 'direct'; scope: Exclude<SelectAllScope, 'all'> }
  | { kind: 'choice' }

export interface SelectableContent {
  images: readonly BrowserFile[]
  textFiles: readonly BrowserFile[]
}

export function resolveAdaptiveContentMode(
  imageCount: number,
  textCount: number,
  preferredExpanded: boolean,
): AdaptiveContentMode {
  if (imageCount <= 0 && textCount <= 0) return 'empty'
  if (imageCount <= 0) return 'text_only'
  if (textCount <= 0) return 'image_only'
  return preferredExpanded ? 'mixed_expanded' : 'mixed_collapsed'
}

export function resolveSelectAllRequest(imageCount: number, textCount: number): SelectAllRequest {
  if (imageCount <= 0 && textCount <= 0) return { kind: 'none' }
  if (textCount <= 0) return { kind: 'direct', scope: 'images' }
  if (imageCount <= 0) return { kind: 'direct', scope: 'text' }
  return { kind: 'choice' }
}

export function filesForSelectAllScope(
  content: SelectableContent,
  scope: SelectAllScope,
): readonly BrowserFile[] {
  if (scope === 'images') return content.images
  if (scope === 'text') return content.textFiles
  return [...content.images, ...content.textFiles]
}
