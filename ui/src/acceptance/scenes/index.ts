import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import { DIALOG_SCENES } from './dialogScenes'
import { FEEDBACK_SCENES } from './feedbackScenes'
import { VIEWING_SCENES } from './viewingScenes'
import { WORKSPACE_SCENES } from './workspaceScenes'

export const ACCEPTANCE_SCENES: AcceptanceSceneRegistry = {
  ...WORKSPACE_SCENES,
  ...VIEWING_SCENES,
  ...DIALOG_SCENES,
  ...FEEDBACK_SCENES,
}
