import { describe, expect, it } from 'vitest'
import {
  DEFAULT_VIEWER_SETTINGS_UPDATE,
  MAGNIFIER_AREAS,
  MAGNIFIER_MAGNIFICATIONS,
  MAGNIFIER_SHAPES,
} from './viewerSettings'

describe('viewer settings defaults', () => {
  it('uses the approved bounded magnifier defaults and choices', () => {
    expect(DEFAULT_VIEWER_SETTINGS_UPDATE).toEqual({
      thumbnailDensity: 'standard',
      magnifier: { shape: 'circle', magnification: 2, area: 'small' },
    })
    expect(MAGNIFIER_SHAPES).toEqual(['circle', 'rounded_rectangle'])
    expect(MAGNIFIER_MAGNIFICATIONS).toEqual([2, 3, 4])
    expect(MAGNIFIER_AREAS).toEqual(['small', 'medium', 'large'])
  })
})
