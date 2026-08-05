import { describe, expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import { aggregateStateRendered, WORKSPACE_SCENES, workspaceBridge } from './workspaceScenes'

it('covers every workspace acceptance state exactly once in ledger order', () => {
  expect(Object.keys(WORKSPACE_SCENES)).toEqual(
    ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'workspace').map(
      ({ id }) => id,
    ),
  )
})

describe('structure scene grounding', () => {
  it('uses numbered content folders for the root filmstrip acceptance state', async () => {
    const workspace = await workspaceBridge('STR-01').queryFolder(null, false)

    expect(workspace.workspace).toBe('category')
    if (workspace.workspace !== 'category') throw new Error('Expected STR-01 category workspace')
    expect(workspace.folders.map(({ name }) => name)).toEqual(['A01', 'A02', 'A03', 'A04'])
  })

  it('does not mistake the aggregate menu command for the rendered aggregate state', () => {
    document.body.innerHTML = '<button>显示全部后代文件</button>'
    expect(aggregateStateRendered(document)).toBe(false)

    document.body.innerHTML = '<span class="aggregate-label">全部后代文件</span>'
    expect(aggregateStateRendered(document)).toBe(true)
  })
})
