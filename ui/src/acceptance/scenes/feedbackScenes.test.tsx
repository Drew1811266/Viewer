import { expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import { FEEDBACK_SCENES } from './feedbackScenes'

it('covers every feedback and accessibility state exactly once in ledger order', () => {
  expect(Object.keys(FEEDBACK_SCENES)).toEqual(
    ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'feedback').map(
      ({ id }) => id,
    ),
  )
})
