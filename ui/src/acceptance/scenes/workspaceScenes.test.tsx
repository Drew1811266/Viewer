import { expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import { WORKSPACE_SCENES } from './workspaceScenes'

it('covers every workspace acceptance state exactly once in ledger order', () => {
  expect(Object.keys(WORKSPACE_SCENES)).toEqual(
    ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'workspace').map(
      ({ id }) => id,
    ),
  )
})
