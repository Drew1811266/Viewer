import { expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import { DIALOG_SCENES } from './dialogScenes'

it('covers every dialog acceptance state exactly once in ledger order', () => {
  expect(Object.keys(DIALOG_SCENES)).toEqual(
    ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'dialog').map(
      ({ id }) => id,
    ),
  )
})
