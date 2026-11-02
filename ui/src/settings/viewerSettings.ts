import type {
  MagnifierArea,
  MagnifierMagnification,
  MagnifierShape,
  ViewerSettingsUpdate,
} from '../api/types'

export const MAGNIFIER_SHAPES: readonly MagnifierShape[] = ['circle', 'rounded_rectangle']
export const MAGNIFIER_MAGNIFICATIONS: readonly MagnifierMagnification[] = [2, 3, 4]
export const MAGNIFIER_AREAS: readonly MagnifierArea[] = ['small', 'medium', 'large']

export const DEFAULT_VIEWER_SETTINGS_UPDATE: ViewerSettingsUpdate = {
  thumbnailDensity: 'large',
  magnifier: {
    shape: 'circle',
    magnification: 2,
    area: 'small',
  },
  speech: {
    inputMode: 'click',
    locale: 'simplified_chinese',
    autoPunctuation: true,
  },
}
