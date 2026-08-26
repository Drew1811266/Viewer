import catalog from './acceptanceStateCatalog.json'

export type AcceptanceWave = 1 | 2 | 3 | 4
export type AcceptanceSceneGroup =
  | 'workspace'
  | 'viewing'
  | 'dialog'
  | 'feedback'
  | 'video'
  | 'review'

export interface AcceptanceStateDefinition {
  id: string
  wave: AcceptanceWave
  referenceState: string
  sceneGroup: AcceptanceSceneGroup
  components: readonly string[]
}

function isAcceptanceWave(value: number): value is AcceptanceWave {
  return value === 1 || value === 2 || value === 3 || value === 4
}

function isSceneGroup(value: string): value is AcceptanceSceneGroup {
  return (
    value === 'workspace' ||
    value === 'viewing' ||
    value === 'dialog' ||
    value === 'feedback' ||
    value === 'video' ||
    value === 'review'
  )
}

function parseCatalog(): readonly AcceptanceStateDefinition[] {
  return catalog.map((entry) => {
    if (!isAcceptanceWave(entry.wave) || !isSceneGroup(entry.sceneGroup)) {
      throw new Error(`Invalid Viewer acceptance state definition: ${entry.id}`)
    }
    return {
      id: entry.id,
      wave: entry.wave,
      referenceState: entry.referenceState,
      sceneGroup: entry.sceneGroup,
      components: entry.components,
    }
  })
}

export const ACCEPTANCE_STATE_DEFINITIONS = parseCatalog()

const DEFINITIONS_BY_ID = new Map(
  ACCEPTANCE_STATE_DEFINITIONS.map((definition) => [definition.id, definition]),
)

export function acceptanceDefinition(id: string): AcceptanceStateDefinition {
  const definition = DEFINITIONS_BY_ID.get(id)
  if (definition === undefined) throw new Error(`Unknown Viewer acceptance state: ${id}`)
  return definition
}
