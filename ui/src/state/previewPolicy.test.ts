import { describe, expect, it } from 'vitest'
import type { BrowserFile } from '../api/types'
import { validatePreviewSelection } from './previewPolicy'

const file = (kind: BrowserFile['kind']) => ({ kind })

describe('validatePreviewSelection', () => {
  it('allows one indexed file of any kind', () => {
    expect(validatePreviewSelection([file('jpeg')])).toEqual({
      ok: true,
      mode: 'single',
    })
    expect(validatePreviewSelection([file('other')])).toEqual({
      ok: true,
      mode: 'single',
    })
  })

  it('allows exactly two previewable text files', () => {
    expect(validatePreviewSelection([file('text'), file('markdown')])).toEqual({
      ok: true,
      mode: 'split_text',
    })
  })

  it('rejects two files unless both are previewable text', () => {
    expect(validatePreviewSelection([file('text'), file('other')])).toEqual({
      ok: false,
      reason: '仅支持单文件预览，或同时预览 2 个文本文件',
    })
  })

  it('gives previewable text overflow its specific reason', () => {
    expect(validatePreviewSelection([file('text'), file('text'), file('text')])).toEqual({
      ok: false,
      reason: '文本预览最多支持 2 个可预览文件',
    })
  })

  it('uses the general reason for larger mixed selections', () => {
    expect(validatePreviewSelection([file('jpeg'), file('text'), file('other')])).toEqual({
      ok: false,
      reason: '仅支持单文件预览，或同时预览 2 个文本文件',
    })
  })
})
