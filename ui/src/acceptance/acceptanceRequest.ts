import { acceptanceDefinition } from './acceptanceStateCatalog'

export type AcceptanceViewport = '720x720' | '1024x720' | '1440x900'

export interface AcceptanceRequest {
  id: string
  viewport: AcceptanceViewport
  width: 720 | 1024 | 1440
  height: 720 | 900
}

export const VIDEO_FEASIBILITY_ACCEPTANCE_ID = 'VIDEO-FEASIBILITY'

const VIEWPORTS = {
  '720x720': { width: 720, height: 720 },
  '1024x720': { width: 1024, height: 720 },
  '1440x900': { width: 1440, height: 900 },
} as const

export function parseAcceptanceRequest(search: string): AcceptanceRequest {
  const parameters = new URLSearchParams(search)
  for (const key of parameters.keys()) {
    if (key !== 'id' && key !== 'viewport') {
      throw new Error(`Unknown Viewer acceptance parameter: ${key}`)
    }
  }

  const ids = parameters.getAll('id')
  if (ids.length === 0) throw new Error('Missing Viewer acceptance state ID')
  if (ids.length !== 1) throw new Error('Viewer acceptance state ID must appear exactly once')

  const viewportValues = parameters.getAll('viewport')
  if (viewportValues.length === 0) throw new Error('Missing Viewer acceptance viewport')
  if (viewportValues.length !== 1) {
    throw new Error('Viewer acceptance viewport must appear exactly once')
  }

  const id = ids[0] as string
  if (id !== VIDEO_FEASIBILITY_ACCEPTANCE_ID) acceptanceDefinition(id)
  const viewport = viewportValues[0] as string
  if (!(viewport in VIEWPORTS)) {
    throw new Error(`Unsupported Viewer acceptance viewport: ${viewport}`)
  }
  const acceptedViewport = viewport as AcceptanceViewport
  return { id, viewport: acceptedViewport, ...VIEWPORTS[acceptedViewport] }
}
