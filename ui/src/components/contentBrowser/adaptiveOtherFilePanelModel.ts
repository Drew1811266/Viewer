import type { BrowserFile } from '../../api/types'

export type AdaptiveContentMode =
  | 'empty'
  | 'image_only'
  | 'mixed_collapsed'
  | 'mixed_expanded'
  | 'other_only'

export type SelectAllScope = 'images' | 'videos' | 'otherFiles' | 'all'

export type SelectAllRequest =
  | { kind: 'none' }
  | { kind: 'direct'; scope: Exclude<SelectAllScope, 'all'> }
  | { kind: 'choice'; scopes: Array<Exclude<SelectAllScope, 'all'>> }

export interface SelectableContent {
  images: readonly BrowserFile[]
  videos: readonly BrowserFile[]
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
  videoCount: number,
  otherFileCount: number,
): SelectAllRequest {
  const available = [
    ['images', imageCount],
    ['videos', videoCount],
    ['otherFiles', otherFileCount],
  ].filter(([, count]) => Number(count) > 0) as Array<[Exclude<SelectAllScope, 'all'>, number]>
  if (available.length === 0) return { kind: 'none' }
  if (available.length === 1) return { kind: 'direct', scope: available[0]?.[0] ?? 'images' }
  return { kind: 'choice', scopes: available.map(([scope]) => scope) }
}

export function filesForSelectAllScope(
  content: SelectableContent,
  scope: SelectAllScope,
): readonly BrowserFile[] {
  if (scope === 'images') return content.images
  if (scope === 'videos') return content.videos
  if (scope === 'otherFiles') return content.otherFiles
  return [...content.images, ...content.videos, ...content.otherFiles]
}
