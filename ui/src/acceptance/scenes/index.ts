import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import { VIEWING_SCENES } from './viewingScenes'
import { WORKSPACE_SCENES } from './workspaceScenes'

export const ACCEPTANCE_SCENES: AcceptanceSceneRegistry = {
  ...WORKSPACE_SCENES,
  ...VIEWING_SCENES,
}
