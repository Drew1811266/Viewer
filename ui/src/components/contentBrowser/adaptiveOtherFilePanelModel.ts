import type { BrowserFile } from '../../api/types'

export type AdaptiveContentMode =
  | 'empty'
  | 'image_only'
  | 'mixed_collapsed'
  | 'mixed_expanded'
  | 'other_only'

export type SelectAllScope = 'images' | 'other' | 'all'

export type SelectAllRequest =
  | { kind: 'none' }
  | { kind: 'direct'; scope: Exclude<SelectAllScope, 'all'> }
  | { kind: 'choice' }

export interface SelectableContent {
  images: readonly BrowserFile[]
  otherFiles: readonly BrowserFile[]
}

export function resolveAdaptiveContentMode(
  imageCount: number,
  otherFileCount: number,
  preferredExpanded: boolean,
): AdaptiveContentMode {
  if (imageCount <= 0 && otherFileCount <= 0) return 'empty'
  if (imageCount <= 0) return 'other_only'
  if (otherFileCount <= 0) return 'image_only'
  return preferredExpanded ? 'mixed_expanded' : 'mixed_collapsed'
}

export function resolveSelectAllRequest(
  imageCount: number,
  otherFileCount: number,
): SelectAllRequest {
  if (imageCount <= 0 && otherFileCount <= 0) return { kind: 'none' }
  if (otherFileCount <= 0) return { kind: 'direct', scope: 'images' }
  if (imageCount <= 0) return { kind: 'direct', scope: 'other' }
  return { kind: 'choice' }
}

export function filesForSelectAllScope(
  content: SelectableContent,
  scope: SelectAllScope,
): readonly BrowserFile[] {
  if (scope === 'images') return content.images
  if (scope === 'other') return content.otherFiles
  return [...content.images, ...content.otherFiles]
}
