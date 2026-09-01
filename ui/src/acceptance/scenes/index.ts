import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import { CONTINUOUS_REVIEW_SCENES } from './continuousReviewScenes'
import { DIALOG_SCENES } from './dialogScenes'
import { FEEDBACK_SCENES } from './feedbackScenes'
import { REVIEW_SCENES } from './reviewScenes'
import { VIDEO_FEASIBILITY_SCENES } from './videoFeasibilityScene'
import { VIDEO_SCENES } from './videoScenes'
import { VIEWING_SCENES } from './viewingScenes'
import { WORKSPACE_SCENES } from './workspaceScenes'

export const ACCEPTANCE_SCENES: AcceptanceSceneRegistry = {
  ...WORKSPACE_SCENES,
  ...VIEWING_SCENES,
  ...DIALOG_SCENES,
  ...FEEDBACK_SCENES,
  ...VIDEO_FEASIBILITY_SCENES,
  ...VIDEO_SCENES,
  ...REVIEW_SCENES,
  ...CONTINUOUS_REVIEW_SCENES,
}
