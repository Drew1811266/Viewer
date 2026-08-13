import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { ACCEPTANCE_STATE_DEFINITIONS, acceptanceDefinition } from './acceptanceStateCatalog'

interface LedgerState {
  id: string
  wave: number
  referenceState: string
}

const VIDEO_ACCEPTANCE_SCENE_IDS = [
  'workspace-video-expanded',
  'workspace-video-unavailable',
  'video-preparing',
  'video-playing-controls',
  'video-paused-controls',
  'video-timeline-pending',
  'video-timeline-ready',
  'video-ended',
  'video-failed-retry',
  'video-fullscreen-controls',
  'settings-video-cache',
  'video-reduced-motion',
]

function ledgerStates(): LedgerState[] {
  const source = readFileSync(
    resolve(
      import.meta.dirname,
      '../../../docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md',
    ),
    'utf8',
  )
  return source
    .split('\n')
    .filter((line) => {
      const id = line.match(/^\| ([^|]+) \|/)?.[1]
      return (
        id !== undefined &&
        (/^[A-Z0-9]+-\d{2}$/.test(id) || VIDEO_ACCEPTANCE_SCENE_IDS.includes(id as never))
      )
    })
    .map((line) => {
      const cells = line
        .slice(1, -1)
        .split(' | ')
        .map((cell) => cell.trim())
      const id = cells[0]
      const waveMatch = cells[1]?.match(/^Wave ([1-4])$/)
      const referenceMatch = cells[2]?.match(/^`([^`]+)`$/)
      if (
        id === undefined ||
        waveMatch === undefined ||
        waveMatch === null ||
        referenceMatch === undefined ||
        referenceMatch === null
      ) {
        throw new Error(`Malformed Viewer acceptance ledger row: ${line}`)
      }
      return {
        id,
        wave: Number(waveMatch[1]),
        referenceState: referenceMatch[1] as string,
      }
    })
}

describe('Viewer visual acceptance state catalog', () => {
  it('matches every ledger ID, wave and atlas reference state exactly once', () => {
    const expected = ledgerStates()
    const actual = ACCEPTANCE_STATE_DEFINITIONS.map(({ id, wave, referenceState }) => ({
      id,
      wave,
      referenceState,
    }))

    expect(actual).toHaveLength(104)
    expect(new Set(actual.map(({ id }) => id)).size).toBe(104)
    expect(actual).toEqual(expected)
  })

  it('adds the exact video acceptance IDs without replacing the approved atlas catalog', () => {
    const videos = ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'video')

    expect(videos.map(({ id }) => id)).toEqual(VIDEO_ACCEPTANCE_SCENE_IDS)
    expect(videos.map(({ referenceState }) => referenceState)).toEqual(VIDEO_ACCEPTANCE_SCENE_IDS)
    expect(new Set(ACCEPTANCE_STATE_DEFINITIONS.map(({ id }) => id)).size).toBe(104)
  })

  it('fails closed for an unknown visual acceptance state', () => {
    expect(() => acceptanceDefinition('PRE-99')).toThrow('Unknown Viewer acceptance state: PRE-99')
  })
})
