import { describe, expect, it } from 'vitest'
import { parseAcceptanceRequest } from './acceptanceRequest'

describe('Viewer visual acceptance request', () => {
  it('accepts one known state at one exact approved viewport', () => {
    expect(parseAcceptanceRequest('?id=video-playing-controls&viewport=720x720')).toEqual({
      id: 'video-playing-controls',
      viewport: '720x720',
      width: 720,
      height: 720,
    })
    expect(parseAcceptanceRequest('?id=PRE-01&viewport=1024x720')).toEqual({
      id: 'PRE-01',
      viewport: '1024x720',
      width: 1024,
      height: 720,
    })
    expect(parseAcceptanceRequest('?id=VIDEO-FEASIBILITY&viewport=1024x720').id).toBe(
      'VIDEO-FEASIBILITY',
    )
  })

  it('rejects unknown states, unsupported viewports and missing identifiers', () => {
    expect(() => parseAcceptanceRequest('?id=PRE-99&viewport=1024x720')).toThrow(
      'Unknown Viewer acceptance state: PRE-99',
    )
    expect(() => parseAcceptanceRequest('?id=PRE-01&viewport=1024')).toThrow(
      'Unsupported Viewer acceptance viewport: 1024',
    )
    expect(() => parseAcceptanceRequest('?viewport=1024x720')).toThrow(
      'Missing Viewer acceptance state ID',
    )
  })

  it('rejects repeated and unrecognized parameters', () => {
    expect(() => parseAcceptanceRequest('?id=PRE-01&id=PRE-02&viewport=1024x720')).toThrow(
      'Viewer acceptance state ID must appear exactly once',
    )
    expect(() => parseAcceptanceRequest('?id=PRE-01&viewport=1024x720&debug=true')).toThrow(
      'Unknown Viewer acceptance parameter: debug',
    )
  })
})
