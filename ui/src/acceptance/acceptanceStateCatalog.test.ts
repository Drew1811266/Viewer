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

const REVIEW_ACCEPTANCE_STATES = [
  ['RVW-01', 'review-start-selection'],
  ['RVW-02', 'review-start-folder-aggregate'],
  ['RVW-03', 'review-active-grid'],
  ['RVW-04', 'review-active-preview'],
  ['RVW-05', 'review-inspector-multi-target'],
  ['RVW-06', 'review-save-error'],
  ['RVW-07', 'review-resume'],
  ['RVW-08', 'review-writer-busy'],
  ['RVW-09', 'review-read-only'],
  ['RVW-10', 'review-completion-summary'],
  ['RVW-11', 'review-version-conflict'],
  ['RVW-12', 'review-recovery-required'],
  ['RVW-13', 'review-completed-read-only'],
  ['RVW-14', 'review-keyboard-focus'],
  ['RVW-15', 'review-zoom-200'],
  ['RVW-16', 'review-auto-start-first-annotation'],
  ['RVW-17', 'review-image-four-annotations'],
  ['RVW-18', 'review-annotation-save-error'],
  ['RVW-19', 'review-rectangle-edit'],
  ['RVW-20', 'review-brush-redraw'],
  ['RVW-21', 'legacy-review-fixed-scope-outside-read-only'],
  ['RVW-22', 'review-unsaved-leave-guard'],
  ['RVW-23', 'review-grid-feedback-badges'],
  ['RVW-24', 'review-workbench-zoom-200'],
  ['RVW-25', 'continuous-review-current-edit-and-new-image'],
  ['RVW-26', 'continuous-review-partial-archive-preview'],
  ['RVW-27', 'continuous-review-retain-later-edit'],
  ['RVW-28', 'continuous-review-old-evidence-source-confirmation'],
  ['RVW-29', 'continuous-review-restore-conflict'],
  ['RVW-30', 'continuous-review-archive-unknown-basis'],
  ['RVW-31', 'continuous-review-legacy-migration'],
  ['RVW-32', 'continuous-review-empty-current-with-history'],
] as const

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
    const expected = ledgerStates().filter(({ id }) => !id.startsWith('RVW-'))
    const actual = ACCEPTANCE_STATE_DEFINITIONS.filter(
      ({ sceneGroup }) => sceneGroup !== 'review',
    ).map(({ id, wave, referenceState }) => ({ id, wave, referenceState }))

    expect(actual).toHaveLength(104)
    expect(new Set(actual.map(({ id }) => id)).size).toBe(104)
    expect(actual).toEqual(expected)
  })

  it('adds the exact video acceptance IDs without replacing the approved atlas catalog', () => {
    const videos = ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'video')

    expect(videos.map(({ id }) => id)).toEqual(VIDEO_ACCEPTANCE_SCENE_IDS)
    expect(videos.map(({ referenceState }) => referenceState)).toEqual(VIDEO_ACCEPTANCE_SCENE_IDS)
    expect(new Set(ACCEPTANCE_STATE_DEFINITIONS.map(({ id }) => id)).size).toBe(136)
  })

  it('adds the exact exception-driven review acceptance states as a separate catalog group', () => {
    const reviews = ACCEPTANCE_STATE_DEFINITIONS.filter(({ sceneGroup }) => sceneGroup === 'review')

    expect(reviews.map(({ id, referenceState }) => [id, referenceState])).toEqual(
      REVIEW_ACCEPTANCE_STATES,
    )
    expect(reviews.every(({ wave }) => wave === 4)).toBe(true)
    expect(ACCEPTANCE_STATE_DEFINITIONS).toHaveLength(136)
    expect(new Set(ACCEPTANCE_STATE_DEFINITIONS.map(({ id }) => id)).size).toBe(136)
  })

  it('declares browser zoom once for both review zoom states and preserves existing accessibility zoom', () => {
    expect(acceptanceDefinition('RVW-24')).toHaveProperty('browserZoom', 2)
    expect(acceptanceDefinition('RVW-15')).toHaveProperty('browserZoom', 2)
    expect(acceptanceDefinition('A11Y-05')).toHaveProperty('browserZoom', 2)
    expect(acceptanceDefinition('RVW-17')).toHaveProperty('browserZoom', 1)
  })

  it('fails closed for an unknown visual acceptance state', () => {
    expect(() => acceptanceDefinition('PRE-99')).toThrow('Unknown Viewer acceptance state: PRE-99')
  })
})
