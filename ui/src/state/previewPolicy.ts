import type { BrowserFile } from '../api/types'
import { isPreviewableText } from '../fileKinds'

export type PreviewSelectionResult =
  | { ok: true; mode: 'single' | 'split_text' }
  | { ok: false; reason: string }

export function validatePreviewSelection(
  files: readonly Pick<BrowserFile, 'kind'>[],
): PreviewSelectionResult {
  if (files.length === 1) return { ok: true, mode: 'single' }
  if (files.length === 2 && files.every(isPreviewableText)) {
    return { ok: true, mode: 'split_text' }
  }
  if (files.length > 2 && files.every(isPreviewableText)) {
    return { ok: false, reason: '文本预览最多支持 2 个可预览文件' }
  }
  return {
    ok: false,
    reason: '仅支持单文件预览，或同时预览 2 个文本文件',
  }
}
